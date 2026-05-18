# ADR-002: Inline Reply Delivery Gate

- Status: Proposed
- Date: 2026-05-18

## Context

Channel `SendReply` currently triggers the `send_reply` WASM module for every
reply. That is required for webhook-backed channels because the module performs
the external POST before recording `ReplyDelivered`.

For inline channel types (`cli`, `tui`), `send_reply` performs no external I/O.
It prepares the same reply parameters and records `ReplyDelivered`. PERF-016
showed this zero-I/O hop still costs about `156 ms` in the production routed
trace after the module is already warm.

## Decision

Permit direct agent-originated `ReplyDelivered` only when the Channel resource
is configured as an inline channel (`cli` or `tui`).

The regular public `SendReply` action remains available to agents and remains
the default path for webhook-backed channels. The `send_reply` module remains
the only path that can record delivery after external webhook I/O.

## Semantics

Direct `ReplyDelivered` is not a generic bypass. It is the verified completion
action for transports whose delivery work is entirely represented by recording
the reply on the Channel entity.

Webhook-backed channels continue to require:

`SendReply -> send_reply -> ReplyDelivered`

Inline channels may use:

`ReplyDelivered`

The Channel audit still records the reply content, thread, and agent entity id.

## Consequences

Positive:

- Inline replies avoid a redundant background WASM dispatch.
- Existing webhook correctness remains unchanged.
- The permission is expressed in Cedar, so an incorrect Session snapshot cannot
  force direct delivery on a webhook Channel.

Tradeoffs:

- Channel policy now depends on Channel type fields being present in the Cedar
  resource. The policy checks both generated casing variants for compatibility.
- Tests must keep this permission narrow so future channel types do not
  accidentally bypass transport delivery.

## Verification

- Add a policy/architecture test that direct agent `ReplyDelivered` is gated by
  inline channel type and does not replace `SendReply` for all channels.
- Run channel and session architecture tests.
- Live proof after deploy must show inline routed replies still produce
  `ReplyDelivered`, while Datadog no longer shows `wasm:send_reply` for the
  inline proof.

## Rollback

Remove the direct inline `ReplyDelivered` permit and route all terminal replies
through `SendReply?await_integration=true` again.
