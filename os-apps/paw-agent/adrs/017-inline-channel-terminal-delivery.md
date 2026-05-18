# ADR-017: Direct Inline Channel Terminal Delivery

- Status: Proposed
- Date: 2026-05-18

## Context

PERF-016 warmed the channel route/reply WASM modules successfully. The accepted
production proof on version `35f55a93b511f21c887c43e01f0fe3bc47f83f49`
showed no lazy compile for `channel_connect`, `route_message`, or `send_reply`.
The remaining routed trace still carried terminal delivery cost:

- `Session.workflow`: about `3069 ms`
- `Session.RecordResult.integrations`: about `528 ms`
- `Channel.SendReply`: about `165 ms`
- `wasm:send_reply`: about `156 ms`

For webhook-backed channels, that nested `SendReply` integration is the
correctness boundary that performs an external platform POST before the Channel
records `ReplyDelivered`. For inline channels such as `cli` and `tui`, there is
no external transport hop. `send_reply` only validates the inline channel shape
and emits `ReplyDelivered` with the same reply fields.

The route snapshot already lets `agent_reply` avoid rediscovering the Channel
entity. It does not yet carry the Channel type, so `agent_reply` cannot safely
distinguish zero-I/O inline delivery from webhook delivery without adding a
lookup back into the terminal hot path.

## Decision

Extend the Session reply route snapshot with an optional `reply_channel_type`
field captured by `route_message` from the current Channel entity.

When `agent_reply` has a complete route snapshot:

1. If `reply_channel_type` is `cli` or `tui`, call the bound Channel
   `ReplyDelivered` action directly with the reply fields.
2. If `reply_channel_type` is missing or any non-inline type, preserve the
   existing `SendReply?await_integration=true` path.
3. If the Session is old or fallback-routed through `ChannelSession`, preserve
   existing lookup and awaited `SendReply` behavior because the channel type is
   not known on the Session route snapshot.

The corresponding Channel policy will only permit agent-originated direct
`ReplyDelivered` for inline Channel resources. Webhook channels continue to
require the `send_reply` module to perform external delivery first.

## Semantics

Inline terminal delivery becomes:

`Session.RecordResult -> agent_reply -> Channel.ReplyDelivered`

Webhook terminal delivery remains:

`Session.RecordResult -> agent_reply -> Channel.SendReply -> send_reply -> Channel.ReplyDelivered`

This does not remove Session completion, Channel audit, OTS trajectory
emission, Cedar authorization, tenant isolation, or the verified Channel
state-machine action. It removes only the redundant zero-I/O WASM hop for
inline transports.

`reply_channel_type` is route metadata, not behavioral source of truth. Cedar
checks the Channel resource before accepting direct `ReplyDelivered`; if the
Session snapshot is wrong or missing, the safe path is the existing awaited
`SendReply` integration.

## Consequences

Positive:

- CLI/TUI routed replies should no longer pay `wasm:send_reply` and nested
  `Channel.SendReply` latency on the terminal hot path.
- User-visible inline reply proof can still wait for the `ReplyDelivered`
  Channel event, preserving the audit point that clients already understand.
- Discord/webhook delivery correctness is unchanged.
- Datadog should show inline traces with direct `Channel.ReplyDelivered` and no
  `wasm:send_reply` span for that route.

Tradeoffs:

- Session specs gain one optional route field.
- Channel policy gains a narrow direct-delivery permission for inline channels.
- Existing Sessions without `reply_channel_type` do not benefit, which is
  intentional compatibility.

## Verification

- Red tests before implementation:
  - Session spec and CSDL expose `reply_channel_type`.
  - `route_message` carries Channel type into the Session Configure body.
  - `agent_reply` chooses direct `ReplyDelivered` for `cli`/`tui` route
    snapshots and awaited `SendReply` for webhook or unknown routes.
  - Channel Cedar policy gates direct agent `ReplyDelivered` to inline Channel
    resources.
- Green implementation in `route_message`, `agent_reply`,
  `session.ioa.toml`, Session CSDL, and channels policy.
- Run affected WASM unit tests, Session architecture tests, Datadog
  observability contract tests, formatting, check, and clippy.
- After PR merge and deploy:
  - run a CLI/TUI routed proof and assert the Channel records
    `ReplyDelivered`;
  - query Datadog for the proof trace and confirm inline delivery avoids
    `wasm:send_reply`;
  - run or inspect a non-inline route proof to confirm awaited `SendReply`
    remains the external-delivery path.

## Rollback

Remove `reply_channel_type` from Session Configure bodies and make
`agent_reply` always dispatch through the existing
`Paw.Channel.SendReply?await_integration=true` URL. Remove the inline
`ReplyDelivered` policy permit. Existing stored Sessions remain compatible
because the field is optional.
