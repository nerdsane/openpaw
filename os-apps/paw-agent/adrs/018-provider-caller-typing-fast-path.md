# ADR-018: Provider Caller Route-Aware Typing Fast Path

- Status: Proposed
- Date: 2026-05-18

## Context

PERF-018 repaired production WASM phase tracing and proved the deployed
`sha-7bfbe112843e0828dfecc2b0e503561a13830a5e` path with direct and routed live
Sessions. The new `wasm.invoke.*` spans show the Temper WASM envelope is not the
primary remaining bottleneck: context serialization, store creation,
instantiation, binding, result read, and parse are all small compared with guest
`run`.

The direct proof trace
`47b820db2bce293279f5f38dd32aaa95` exposed one low-risk guest-run issue in
`provider_caller`: before calling the mock provider, it tries to send a Discord
typing indicator for a direct no-route Session. That helper performs
ChannelSession discovery even when there is no channel route. In the proof, the
direct Session paid two `GET /tdata/ChannelSessions` calls inside
`provider_caller`, about `144 ms` and `15 ms`, and both returned no useful
typing target.

The routed CLI proof trace
`2674ba4a40a08cb4f1b385dfaa3de5cb` already carries explicit route metadata:
`reply_channel_type=cli`, `reply_channel_id`, `reply_thread_id`, and
`reply_channel_entity_id`. Inline `cli`/`tui` channels do not support external
typing indicators, while Discord/webhook-style channels may still benefit from
the existing typing behavior.

## Decision

Add a route-aware gate before `provider_caller` invokes
`send_typing_indicator`:

1. Skip typing for direct no-route Sessions that have no reply channel, no
   reply thread, and no parent Session.
2. Skip typing for inline channel routes where `reply_channel_type` is `cli` or
   `tui`.
3. Preserve typing for explicit non-inline route types such as `discord` or
   `slack`.
4. Preserve typing for legacy route snapshots with channel/thread but no
   `reply_channel_type`, because the older Session may still be webhook-backed.
5. Preserve typing for child/parented Sessions so inherited parent-channel
   lookup behavior remains compatible.

This keeps the optimization in the Temper-native WASM integration layer. It
does not change Session specs, Channel specs, Cedar policies, state-machine
transitions, reply delivery, OTS trajectory emission, or SessionEntry
persistence.

## Semantics

Direct no-route provider calls change from:

`provider_caller -> find ChannelSession(active) -> find ChannelSession(any) -> no session -> provider`

to:

`provider_caller -> route gate says no typing target -> provider`

Inline CLI/TUI routed calls similarly avoid typing work. Webhook-backed routed
calls and parented child Sessions keep the existing behavior:

`provider_caller -> send_typing_indicator -> ChannelSession/Channel lookup -> /typing POST when configured`

The typing indicator remains best-effort user-experience signaling only. It is
not a correctness boundary and does not affect provider output, Session
completion, reply audit, or trajectory records.

## Consequences

Positive:

- Direct no-route Sessions should remove the observed `provider_caller`
  ChannelSession lookup pair, worth about `160 ms` in the PERF-018 direct proof
  trace.
- Inline CLI/TUI Sessions avoid a class of future route lookup work that cannot
  produce a typing indicator.
- Discord/webhook routes keep the existing visible typing behavior.
- The change is small, reversible, and covered by direct unit tests.

Tradeoffs:

- Legacy route snapshots without `reply_channel_type` stay conservative and may
  still perform typing lookup work. That is intentional to avoid breaking old
  webhook routes.
- This slice does not solve the larger Session stage orchestration cost,
  repeated OData loopbacks, SessionEntry verification overhead, or OTS tail
  latency. Those remain PERF-019 follow-ups after this low-risk cut.

## Verification

- Red tests first in `provider_caller`:
  - direct no-route fields skip typing;
  - inline `cli` and `tui` route fields skip typing;
  - explicit `discord`/`slack` route fields send typing;
  - legacy channel/thread fields with no type still send typing;
  - parented Sessions still send typing for inherited route compatibility.
- Green implementation adds the gate and calls `send_typing_indicator` only
  when it returns true.
- Run focused provider-caller tests, Session architecture tests, Datadog
  observability contract tests, affected WASM build, package check/clippy,
  rustfmt, and diff whitespace check.
- Live proof after merge/deploy:
  - direct mock Session completes and Datadog shows no provider-caller
    `GET /tdata/ChannelSessions` spans;
  - routed CLI Session still completes and records `ReplyDelivered`;
  - a Discord/webhook route is either exercised or explicitly checked in traces
    to confirm typing behavior is preserved.

## Rollback

Remove the gate and return to unconditional `send_typing_indicator` from
`provider_caller`. Because no entity spec, state variable, or policy changes are
introduced, rollback is limited to the provider-caller WASM module and its
tests.
