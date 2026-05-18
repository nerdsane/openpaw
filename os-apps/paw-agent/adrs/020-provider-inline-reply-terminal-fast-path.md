# ADR-020: Provider Inline Reply Terminal Fast Path

- Status: Proposed
- Date: 2026-05-18

## Context

PERF-021B removed same-service public `/tdata` HTTP calls from the Temper host.
The accepted production traces for version
`cb17bb462de2defb2d1f6d392b3471115ee5be1f` showed no retained public TData
HTTP spans, but the clean routed mock-provider proof still spent user-visible
time in the terminal reply stage:

- trace `e30544a490079cb5c72097945ed14399`:
  `wasm:agent_reply` about `487 ms` and
  `wasm:emit_ots_trajectory` about `510 ms`;
- trace `bac8b65ef45a79ed84d7a797f5fab653`:
  `wasm:agent_reply` about `445 ms` and
  `wasm:emit_ots_trajectory` about `628 ms`;
- both traces still include a `workflow.drain_grace_ms=2000` observability tail,
  which is not the next user-visible reply target.

ADR-017 made `agent_reply` cheaper for inline `cli` and `tui` routes by sending
`Channel.ReplyDelivered` directly instead of calling `Channel.SendReply` and
the `send_reply` WASM module. That kept the Channel audit point and skipped the
zero-I/O transport hop. The remaining cost is now the extra `agent_reply` WASM
invocation itself. For inline `cli` and `tui` routes, `provider_response_applier`
already has the terminal result, route snapshot, Channel entity id, and tenant
context needed to record `ReplyDelivered`.

Webhook-backed and child-agent routes are different. They may require external
delivery, embeds, parent lookup, or compatibility fallback. Those routes should
continue through `agent_reply`.

## Decision

Add an explicit terminal action for inline channel routes whose reply has
already been delivered:

1. Add `RecordResultInlineReply` from `ApplyingProviderResponse` to `Completed`.
   It mirrors `RecordResult` accounting and cleanup params, sets `has_result`,
   and triggers `emit_ots_trajectory`, but does not trigger `deliver_reply`.
2. Teach `provider_response_applier` to detect a complete inline route snapshot:
   `reply_channel_type` is `cli` or `tui`,
   `reply_channel_entity_id` is present, and
   `reply_thread_id` is present.
3. Before selecting the terminal Session action, have `provider_response_applier`
   POST `Paw.Channel.ReplyDelivered` directly with the same reply body shape
   used by `agent_reply`.
4. If that direct inline dispatch succeeds, choose `RecordResultInlineReply`.
   If it fails or the route is incomplete/non-inline, choose the existing
   `RecordResult` path so `agent_reply` remains the durable compatibility path.
5. Keep `RecordResultNoReply` unchanged for explicit direct no-reply Sessions.

This makes the fast path spec-visible instead of hiding reply skipping in
module-local inference.

## Semantics

Inline routed terminal completion becomes:

`ProviderResponseReady -> Channel.ReplyDelivered -> RecordResultInlineReply -> emit_ots_trajectory`

Existing direct no-reply completion remains:

`ProviderResponseReady -> RecordResultNoReply -> emit_ots_trajectory`

Webhook, unknown, child-agent, parent-routed, and legacy channel completion
remain:

`ProviderResponseReady -> RecordResult -> agent_reply -> Channel delivery/fallback -> emit_ots_trajectory`

The Channel `ReplyDelivered` audit event, Session result, token accounting,
pending tool cleanup, SessionEntry persistence, OTS trajectory emission, Cedar
authorization, tenant isolation, and replay semantics are preserved. The
optimization removes only the redundant inline reply WASM envelope for routes
where there is no external transport work left to do.

## Consequences

Positive:

- CLI/TUI routed replies should avoid the measured `wasm:agent_reply` terminal
  invocation, saving roughly `445-487 ms` in the current production proof.
- The reply audit remains on the Channel entity before Session terminalization.
- Non-inline transports keep the existing, more conservative delivery path.
- Fail-open fallback to `RecordResult` prevents a transient inline dispatch
  failure from losing the reply delivery attempt.

Tradeoffs:

- The provider-response applier now performs one Channel action for the inline
  success path, so it must keep the route-body contract aligned with
  `agent_reply`.
- The Session spec, CSDL, policy, and architecture tests gain another terminal
  success action.
- This slice does not address OTS trajectory cost, workspace/context setup, or
  the larger per-step WASM dispatch overhead.

## Verification

- Red tests before implementation:
  - Session spec/CSDL/policy expose `RecordResultInlineReply` with the same
    cleanup/accounting params as `RecordResult` and without `deliver_reply`;
  - `provider_response_applier` has helpers that identify only `cli`/`tui`
    complete inline routes, build the `ReplyDelivered` URL/body, and fall back
    to `RecordResult` for non-inline or incomplete routes.
- Green implementation in `provider_response_applier`,
  `session.ioa.toml`, `model.csdl.xml`, policy, and architecture tests.
- Run focused provider-response applier tests, Session architecture tests,
  formatting/diff checks, and affected WASM builds.
- After PR merge and deploy:
  - run a routed CLI/TUI mock-provider proof and assert `ReplyDelivered`,
    `RecordResultInlineReply`, and trajectory emission all occur;
  - query Datadog for the proof trace and confirm no retained
    `wasm:agent_reply` span while `Channel.ReplyDelivered` remains present;
  - run or inspect a non-inline route to confirm it still uses `RecordResult`
    and `agent_reply`.

## Rollback

Make `provider_response_applier` always choose `RecordResult` for routed
Sessions and remove `RecordResultInlineReply` from Session spec/CSDL/policy.
Existing stored Sessions remain compatible because the new action is terminal
callback-only and adds no required fields.
