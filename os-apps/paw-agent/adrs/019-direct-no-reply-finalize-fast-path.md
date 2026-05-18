# ADR-019: Direct No-Reply Finalize Fast Path

- Status: Proposed
- Date: 2026-05-18

## Context

PERF-019 deployed provider typing route awareness on version
`b44ee88385ca8ab50eecd7b41a3e44ad099ab2c2`. The direct and CLI-route live
proofs showed the targeted `provider_caller` `ChannelSessions` lookup was gone,
but the retained traces still showed a terminal no-op reply envelope:

- direct trace `799e1d737838b5bb1dd96199103ee201` invoked
  `wasm:agent_reply` for about `605 ms` and logged
  `no channel session linked to agent ...; skipping`;
- CLI-route trace `35f0372054a40dbbb0b1ac4df8f8d6d2` invoked
  `wasm:agent_reply` for about `507 ms` with the same skip shape.

ADR-015 already added `RecordResultNoReply` for the safest direct no-route
shape: no route, no parent, and `agent_id` empty or equal to the Session id.
The new proof used a persistent Agent id. That is a normal direct API shape,
but the current guard treats any distinct `agent_id` as ambiguous because old
channel-created Sessions could rely on `ChannelSession` lookup by agent id.
Correctness won, so the direct proof paid the legacy discovery path and skipped
only after the `agent_reply` WASM invocation had already happened.

The steering finalizer has a second copy of the issue. When
`provider_response_applier` dispatches `CheckSteering`, `steering_checker`
currently has only `FinalizeResult`, which always triggers `deliver_reply`.
Even if the Session is explicitly known to be direct and no-reply, the final
transition still invokes `agent_reply`.

## Decision

Introduce an explicit no-reply route-source contract and carry it through both
terminal success paths:

1. Treat `reply_route_source = "direct_no_reply"` as an explicit marker that
   the Session has no reply transport.
2. Add `FinalizeResultNoReply` from `Steering` to `Completed`. It mirrors
   `FinalizeResult` result and cleanup params, sets `has_result`, and still
   triggers `emit_ots_trajectory`, but does not trigger `deliver_reply`.
3. Have `provider_response_applier` choose `RecordResultNoReply` for a direct
   no-reply Session when either:
   - the old obvious shape holds (`agent_id` empty or equal to Session id,
     no parent, no route fields); or
   - `reply_route_source` is exactly `direct_no_reply`, with no channel/thread
     route and no parent.
4. Have `steering_checker` choose `FinalizeResultNoReply` with the same direct
   no-reply predicate.
5. Keep all non-empty route sources except `direct_no_reply` on the reply
   delivery path. Channel sources such as `channel_message`, `channel_session`,
   manual proof routes, and old/ambiguous Sessions continue through
   `agent_reply` and `ChannelSession` fallback.

This is intentionally stricter than "no channel fields means no reply." A
distinct `agent_id` without an explicit direct marker remains compatible with
legacy channel bindings.

## Semantics

Direct API callers that know they do not want transport delivery can Configure
the Session with:

```json
{
  "agent_id": "aj-...",
  "reply_route_source": "direct_no_reply"
}
```

The successful terminal flow becomes either:

`ProviderResponseReady -> RecordResultNoReply -> emit_ots_trajectory`

or, when queued steering is checked first:

`CheckSteering -> FinalizeResultNoReply -> emit_ots_trajectory`

Channel-created Sessions continue to use:

`RecordResult/FinalizeResult -> agent_reply -> Channel delivery or fallback`

Trajectory emission, SessionEntry persistence, token accounting, pending tool
cleanup, Cedar authorization, tenant isolation, and event replay semantics are
unchanged.

## Consequences

Positive:

- Direct no-reply Sessions with persistent Agent ids can avoid the measured
  terminal `agent_reply` no-op envelope.
- The optimization is explicit in Session state and spec-visible terminal
  actions rather than hidden behind broader inference.
- Legacy channel compatibility remains preserved for ambiguous no-route
  Sessions.
- The steering finalizer no longer reintroduces the no-op reply stage for
  explicit direct no-reply Sessions.

Tradeoffs:

- Direct API callers must set `reply_route_source = "direct_no_reply"` to get
  the new fast path when they use a distinct Agent id.
- There is another successful terminal action, so Session spec, CSDL, policy,
  and architecture tests must keep `FinalizeResult` and `FinalizeResultNoReply`
  aligned.
- The slice does not remove OTS trajectory work, provider-response append work,
  workspace/context setup, or the larger in-process OData/WASM stage overhead.

## Verification

- Red tests before implementation:
  - `provider_response_applier` treats explicit `direct_no_reply` as eligible
    for `RecordResultNoReply` even with a distinct Agent id;
  - other route-source values remain on `RecordResult`;
  - Session spec/CSDL/policy expose `FinalizeResultNoReply`;
  - `steering_checker` dispatches `FinalizeResultNoReply` for explicit direct
    no-reply Sessions and preserves `FinalizeResult` for channel/ambiguous
    Sessions.
- Green implementation in `provider_response_applier`, `steering_checker`,
  `session.ioa.toml`, `model.csdl.xml`, policy, and architecture tests.
- Run focused WASM tests, Session architecture tests, Datadog observability
  contract tests, package check/clippy, rustfmt, diff whitespace, and affected
  WASM builds.
- After merge and deploy, prove:
  - direct no-reply Session with a persistent Agent id completes without a
    retained `wasm:agent_reply` span;
  - queued-steering/direct no-reply path also avoids `wasm:agent_reply`;
  - a routed CLI proof still records `ReplyDelivered`;
  - Datadog trace searches confirm `service.version` and the absence/presence
    of reply spans on the appropriate traces.

## Rollback

Stop setting or honoring `reply_route_source = "direct_no_reply"` and make
`steering_checker` always dispatch `FinalizeResult`. `RecordResultNoReply` from
ADR-015 remains the fallback for the old obvious direct shape.
