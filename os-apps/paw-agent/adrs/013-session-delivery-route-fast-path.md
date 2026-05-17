# ADR-013: Session Delivery Route Fast Path

- Status: Proposed
- Date: 2026-05-17

## Context

PERF-010 made the Session background WASM subtree visible in Datadog by keeping
`dispatch.background_wasm_integrations`, child `wasm:<module>` spans, and
`dispatch.dispatch_wasm_callback` spans together. The first warm production
sample after that change, `perf-011-selection-warm-20260517150441`, completed
five mock-provider Sessions with client p50 `2.804 s`.

The retained trace `9a037861f2acdc41c1213dc9ef69ef0f` showed
`Session.workflow = 4.062 s` on deployed version
`22ac2cb290e0a2252460b953afd2d25a0125dab9`. The largest terminal owner was the
`RecordResult` background integration group:

- `ProvisionWorkspace`: `621 ms`
- `WorkspaceReady`: `458 ms`
- `ContextReadyAuthSkipped`: `414 ms`
- `ProviderResponseReady`: `544 ms`
- `RecordResult`: `1115 ms`

Most `persist_wasm_invocation` spans were only `7-13 ms`; the exception was
`emit_ots_trajectory` at `95 ms`. The trace therefore points less at scheduler
wakeup and more at the terminal integrations attached to `RecordResult`.

Today `agent_reply` has no direct delivery route on the Session. It derives
`session_id`, `agent_id`, and `parent_session_id`, then looks up a
`ChannelSession` by:

1. current Session entity id;
2. parent Session entity id;
3. active Agent entity id;
4. any Agent entity id.

That fallback is necessary for older Sessions and for channel-routed flows
whose binding lives in `ChannelSession`, but it is wasteful for direct API or
mock Sessions that have no channel delivery route. It also makes channel reply
delivery pay discovery cost even though `route_message` already knows
`channel_id` and `thread_id` at Session creation time.

## Decision

Carry an explicit delivery route snapshot on Session configuration for
channel-created Sessions, and make `agent_reply` prefer that route before
falling back to `ChannelSession` lookup.

Add optional Session fields/Configure params:

- `reply_channel_id`
- `reply_thread_id`
- `reply_channel_entity_id`
- `reply_route_source`

`route_message` will set these fields when it creates a new Session or
continuation from an inbound channel message. `reply_channel_id` and
`reply_thread_id` mirror the external channel/thread route. When the Channel
entity id is already available, `reply_channel_entity_id` can avoid a later
`Channels` lookup too; otherwise `agent_reply` may still resolve the Channel by
external id.

`agent_reply` will use this order:

1. If a complete route snapshot exists, deliver with it immediately.
2. If the Session is an obvious direct API/mock Session with no route
   (`agent_id == session_id`, no parent, no reply route fields), skip delivery
   lookup and mark reply delivery as skipped.
3. Otherwise use the existing `ChannelSession` fallback lookup, preserving
   compatibility for old/in-flight Sessions and partially configured routes.

The OTS trajectory integration remains attached to terminal result actions. It
can be optimized separately if Datadog still shows it as material after the
reply-route lookup work is reduced.

## Semantics

The route snapshot does not introduce a new orchestration layer. It is ordinary
Session state derived from the trigger-side channel route at the same moment the
Session is configured. State transitions still explain the flow:

`Channel.ReceiveMessage -> route_message -> Session.Configure -> RecordResult -> agent_reply`

Reply delivery remains a WASM integration on the verified Session transition.
The snapshot only replaces rediscovery of the same route.

Direct API and mock Sessions remain valid Sessions with no reply transport.
Skipping `ChannelSession` lookup for the obvious no-route shape makes that fact
explicit and observable rather than discovering it through repeated empty OData
queries.

## Consequences

Positive:

- Direct API/mock terminal Sessions avoid unnecessary `ChannelSession` OData
  lookups after `RecordResult`.
- Channel-created Sessions can reply from the route captured at creation time,
  reducing terminal delivery latency and lowering OData pressure.
- Datadog should show shorter `wasm:agent_reply` and
  `dispatch.background_wasm_integrations` duration for `RecordResult`.
- Delivery behavior becomes easier to diagnose because the Session carries its
  intended reply route.

Tradeoffs:

- Session specs gain four optional fields that must be kept compatible with
  old Sessions and external callers that do not provide them.
- If a channel route changes after Session creation, the snapshot may be stale.
  Fallback lookup still exists for missing snapshots, but a complete stale
  snapshot will be used. This is acceptable for reply delivery because it
  reflects the channel/thread that created the conversation turn.
- The route snapshot does not by itself remove `emit_ots_trajectory` work.

## Verification

- Red tests before implementation:
  - direct no-route Sessions produce no `ChannelSession` lookup candidates;
  - complete reply-route snapshots are extracted and preferred;
  - channel Configure bodies carry reply route fields from `route_message`;
  - continuation Configure bodies preserve reply route fields.
- Green implementation in `agent_reply`, `route_message`, and
  `session.ioa.toml`.
- Run affected WASM unit tests and Session architecture/spec tests.
- Build affected WASM modules.
- Run Datadog observability contract and CI-equivalent package checks.
- After PR merge and deploy, prove:
  - direct mock Session still reaches `Completed` and records skipped delivery;
  - channel-routed Session still delivers a reply;
  - Datadog retained trace shows lower `agent_reply`/`RecordResult` work;
  - OTS trajectory emission still runs or records a failure for sweep.

## Rollback

Remove the route snapshot fields from `route_message` Configure bodies and make
`agent_reply` always use the existing `ChannelSession` lookup. Existing fallback
logic remains the compatibility path throughout this change.
