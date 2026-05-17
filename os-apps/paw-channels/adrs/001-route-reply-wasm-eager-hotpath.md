# ADR-001: Eager Load Channel Route and Reply WASM

- Status: Proposed
- Date: 2026-05-17

## Context

PERF-015 deployed Temper's local WASM TData host path and proved both direct and
routed Session correctness on production version
`d230d3c22463a7e1c15cfae8d25f9619013526eb`.

The direct proof passed through `RecordResultNoReply` and trajectory emission.
The routed CLI proof passed through `RecordResult`, `agent_reply`,
`Paw.Channel.SendReply`, `send_reply`, `ReplyDelivered`, and trajectory
emission. Datadog also exposed the next route/reply bottleneck:

- routed trace `e2fa0d6f6b6282d14bf561151b1dc308` showed
  `agent_reply` posting `Paw.Channel.SendReply` at about `439 ms`;
- production logs showed first-use lazy compile for `route_message` and
  `send_reply` after deployment;
- a separate harness rerun hit a transient `502` during `Channel.Connect`, and
  logs from the PERF-014/PERF-015 window showed `channel_connect` also
  lazy-compiling on first use.

`paw-channels` is already a `startup_install = "core"` app, but its manifest
only declares `route_message` and `transport_reconcile`. The build script also
produces `channel_connect` and `send_reply`, and the Channel spec invokes them,
but without manifest entries they use Temper's default module policy. That means
the first route or reply after deploy can pay WASM compile work on the user path.

## Decision

Declare the Channel route/reply modules in `os-apps/paw-channels/app.toml` and
mark the user-facing route/reply/connect modules eager:

- `channel_connect`
- `route_message`
- `send_reply`

Keep `transport_reconcile` lazy because it is background transport maintenance,
not the normal route/reply critical path.

This is a manifest/startup placement change only. We are not changing Channel
or Session state machines in this slice, and we are not changing reply delivery
semantics yet.

## Semantics

The routed flow remains visible in entity transitions:

`Channel.ReceiveMessage -> route_message -> Session.Configure -> ... -> Session.RecordResult -> agent_reply -> Channel.SendReply -> send_reply -> Channel.ReplyDelivered -> Session.MarkTrajectoryEmitted`

The change only moves compile/cache work to app install/startup for declared
modules. It does not remove:

- Cedar checks;
- Channel events;
- Session events;
- route fields;
- delivery audit;
- `ReplyDelivered`;
- Session trajectory emission.

## Consequences

Positive:

- The first routed message after deploy should no longer pay lazy compile for
  `route_message` or `send_reply`.
- `Channel.Connect` should no longer pay first-use compile for
  `channel_connect`.
- `send_reply` becomes an explicit app-required module instead of an implicit
  discovered artifact, so missing bundled artifacts fail startup reconciliation
  clearly.

Tradeoffs:

- Startup/app reconcile does more CPU work up front.
- This does not yet solve all steady-state `Paw.Channel.SendReply` time. If
  post-change Datadog still shows `agent_reply` waiting on delivery around the
  same range after warmup, the next ADR should address asynchronous reply
  confirmation or a verified local direct reply path.

## Verification

- Add a red-green manifest test proving:
  - `channel_connect`, `route_message`, and `send_reply` are present,
    app-required, and eager;
  - `transport_reconcile` stays lazy.
- Run focused tests for the manifest guard and Session/Channel route contracts.
- Run package check/clippy/format gates appropriate for manifest/test changes.
- Live proof after deployment:
  - direct no-route Session still completes through `RecordResultNoReply`;
  - routed Session still reaches `ReplyDelivered`;
  - Datadog current-version logs no longer show first-use lazy compile for
    `route_message`, `send_reply`, or `channel_connect`;
  - Datadog shows lower first-route `agent_reply`/`Paw.Channel.SendReply`
    duration, or else the residual is promoted to the next steady-state
    route/reply ADR.

## Rollback

Set `channel_connect`, `route_message`, and `send_reply` back to
`startup_loading = "lazy"` or remove the new manifest declarations for
`channel_connect` and `send_reply`. That restores prior lazy first-use behavior
without changing Channel or Session data.
