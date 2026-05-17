# ADR-015: Direct Session Terminal Delivery Bypass

- Status: Proposed
- Date: 2026-05-17

## Context

PERF-012 deployed the first staged Session hot-path reduction. It improved the
measured bootstrap substep, but the warm direct proof remained broadly flat at
roughly two seconds from the client polling loop.

The current production trace for
`perf-012-hot-bootstrap-warm-5-20260517172726` on version
`166538703356d488dbfad0b65e8c09f20839dfc9` shows:

- `Session.workflow`: `3442 ms`
- `wasm:workspace_provisioner`: `453 ms`
- `wasm:context_preparer`: `312 ms`
- `wasm:provider_caller`: `263 ms`
- `wasm:provider_response_applier`: `377 ms`
- `wasm:agent_reply`: `193 ms`
- `wasm:emit_ots_trajectory`: `229 ms`
- `Session.RecordResult.integrations`: `422 ms`

The `agent_reply` span is already correct after ADR-013: for direct API/mock
Sessions with no channel route, it performs no child HTTP and logs
`no reply route for direct session ...; skipping`. The remaining cost is the
framework envelope for invoking a WASM integration that has nothing useful to
do for this class of Session.

This is distinct from channel-bound production chat. Channel-created Sessions
carry `reply_channel_id`, `reply_thread_id`, and, after PERF-012,
`reply_channel_entity_id`; those Sessions must continue through `agent_reply`
so Discord/transport delivery occurs and failures are recorded through
`DeliveryFailed`.

## Decision

Add an explicit direct-terminal Session action for successful no-route
provider-only completions:

1. Add `RecordResultNoReply` from `ApplyingProviderResponse` to `Completed`.
2. Give it the same state-update params as `RecordResult` for result,
   conversation/session leaf, token accounting, system prompt metadata,
   provider-response cleanup, and pending tool/approval cleanup.
3. Trigger only `emit_ots_trajectory`; do not trigger `deliver_reply`.
4. Add the Cedar callback permit for `RecordResultNoReply` alongside the other
   system-driven Session pipeline callbacks.
5. Have `provider_response_applier` choose `RecordResultNoReply` only when
   the Session is clearly direct:
   - no `reply_channel_id`;
   - no `reply_thread_id`;
   - no `reply_route_source`;
   - no `parent_session_id`;
   - `agent_id` is empty or equal to the Session entity id.
6. Keep `RecordResult` for channel-bound Sessions, child/parent Sessions, tool
   completions, failure/cancel/timeout paths, and any ambiguous route shape.

This moves the existing "no reply route; skipping" decision from a no-op
terminal integration into the state-machine action chosen by the applier. It
does not remove trajectory emission, event recording, SessionEntry persistence,
token accounting, Cedar governance, or the append-only Session tree.

## Semantics

For direct successful completions, the terminal flow changes from:

`ProviderResponseReady -> RecordResult -> deliver_reply(skip) + emit_ots_trajectory`

to:

`ProviderResponseReady -> RecordResultNoReply -> emit_ots_trajectory`

The terminal Session state remains `Completed`, and `CompletedHasResult`
continues to hold. The same result text and accounting fields are persisted.
The trajectory is still emitted so evolution/observability retains the run.

For channel-bound Sessions, the flow remains:

`ProviderResponseReady -> RecordResult -> deliver_reply + emit_ots_trajectory`

If route data is missing or ambiguous, correctness wins: the applier returns
`RecordResult` so `agent_reply` can perform the older ChannelSession lookup and
either deliver or record a skip/failure as it does today.

## Consequences

Positive:

- Direct API/mock Sessions should avoid roughly the current no-op
  `wasm:agent_reply` envelope, about `190 ms` in the PERF-012 warm trace.
- `Session.RecordResult.integrations` should shrink for direct Sessions while
  retaining `emit_ots_trajectory`.
- The change is spec-visible and auditable instead of hiding the optimization
  inside the delivery module.
- Channel delivery and `DeliveryFailed` behavior are left untouched.

Tradeoffs:

- There is now a second successful terminal action, so tests and CSDL must keep
  the two result actions aligned.
- Tool-driven direct completions still use `RecordResult` in this slice. That
  keeps the change small; a later ADR can extend the bypass once tool-terminal
  evidence justifies it.
- This does not solve the larger architectural cost of multiple same-process
  OData/WASM state boundaries.

## Follow-Up Architecture Options

If this measured slice behaves cleanly, the next larger options remain:

- same-process Temper host APIs for entity create/action/read without OData
  loopback, preserving Cedar and event recording;
- a provider-only composite turn executor for no-tools Sessions;
- batched assistant/tool SessionEntry append verification;
- context/system-prompt template caching;
- async-but-correct trajectory emission with retry proof and drift monitoring.

## Verification

- Add unit coverage that the direct-route predicate only matches Sessions with
  no reply route, no parent, and no distinct agent id.
- Add architecture tests that `RecordResultNoReply` mirrors `RecordResult`
  cleanup params and is exposed in CSDL.
- Run affected checks:
  - `cargo fmt --all -- --check`
  - focused `cargo test` for Session turn architecture and
    `provider_response_applier`
  - spec/CSDL validation if available in this worktree
- Live proof:
  - direct mock Session completes with a valid SessionEntry chain and a
    retained trajectory trace;
  - channel-route Session still replies correctly;
  - Datadog current-version traces show no `wasm:agent_reply` span for direct
    no-route completion and preserve `wasm:agent_reply` for channel completion.

## Rollback

Revert the applier to always return `RecordResult` and leave the
`RecordResultNoReply` action unused, or remove the action entirely if CSDL/spec
deployment shows unexpected compatibility risk. The existing `agent_reply`
direct no-route skip remains the safe fallback.
