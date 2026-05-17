# ADR-010: Queue-Gated Steering Fast Finalize

- Status: Proposed
- Date: 2026-05-17

## Context

The latency program live proof on production runtime
`1c6c9cf79a8b5c7a0fd4e01ed86eac0ae6a82278` created a mock-provider Session
`ss-019e352b-7ed3-7751-84d2-e99a88f33b81`. The Session completed correctly,
but Datadog showed `Session.workflow` at about 4.55 seconds even though the
provider was the deterministic mock and no tools were enabled.

The ordinary entity actions were small, usually 18-30 ms. The remaining time was
the staged WASM turn pipeline:

- `Session.ProvisionWorkspace.integrations`: about 456 ms
- `Session.WorkspaceReady.integrations`: about 297 ms
- `Session.ProviderAuthReady.integrations`: about 305 ms
- `wasm:provider_response_applier`: about 611 ms
- `Session.CheckSteering.integrations`: about 702 ms
- `Session.FinalizeResult` background integrations: about 395 ms

The largest avoidable stage is `CheckSteering`. Today an `end_turn` provider
response routes to `CheckSteering` whenever `max_follow_ups > 0`, even when
`steering_messages` is empty. The `steering_checker` module then re-reads the
session tree to recover the result text that `provider_response_applier` already
has in memory.

This preserves a broad late-steering window, but it taxes the common path where
no steering is queued.

## Decision

For `end_turn` / `stop` provider responses, `provider_response_applier` will
route directly to `RecordResult` when there are no queued steering messages.

It will continue to route to `CheckSteering` when all are true:

- `max_follow_ups > 0`
- `steering_messages` parses to a non-empty queue
- `follow_up_count < max_follow_ups`

Tool-use responses still route to `ProcessToolCalls`. Existing explicit
`max_follow_ups = 0` behavior remains a fast finalization path.

## Semantics

This changes the steering contract from "always open a post-provider steering
check when follow-ups are allowed" to "run the steering loop only when steering
is already queued at response-apply time."

That preserves mid-run steering that arrives while the Session is preparing,
authenticating, calling the provider, applying the provider response, executing
tools, compacting, waiting for approval, recovering, or already in Steering. It
does not keep a separate extra no-op `Steering` state solely to wait for a
possible late message after an LLM has already produced an end-turn answer.

If product behavior later needs a deliberate late-steering grace period, it
should be explicit as a bounded field/config such as
`steering_finalize_grace_ms`, not an unconditional extra WASM stage.

## Consequences

Positive:

- Normal no-steering answers avoid one Session action, one WASM invocation, one
  session-tree read, and one callback dispatch.
- The final result comes from the already parsed provider response instead of a
  second read of the session tree, reducing drift risk.
- The fast path keeps Temper-native orchestration: specs, state transitions,
  callbacks, Cedar, OTS emission, and reply delivery remain authoritative.

Tradeoffs:

- A steering message that arrives after response application but before the old
  `steering_checker` would have read the queue may become a new follow-up turn
  instead of being injected into the just-finished turn.
- Sessions that rely on a deliberate late-steering pause should set an explicit
  future grace-period contract rather than depending on no-op `CheckSteering`.

## Verification

- Add unit coverage for queue parsing and route selection:
  - empty/missing/invalid `steering_messages` routes to `RecordResult`
  - non-empty `steering_messages` with `max_follow_ups > 0` routes to
    `CheckSteering`
  - `max_follow_ups = 0` routes to `RecordResult`
- Run the affected WASM crate tests and the paw-agent session architecture tests.
- Run a local or live mock-provider Session proof and compare Datadog:
  - `Session.CheckSteering.integrations` should disappear on the no-steering
    proof path.
  - `Session.workflow` should drop by roughly the removed steering stage.

## Rollback

Revert the provider-response route selection change. The system returns to
always dispatching `CheckSteering` for end-turn responses when
`max_follow_ups > 0`.
