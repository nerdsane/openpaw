# ADR-001: Bounded Reply Delivery Timeout

## Status

Accepted - 2026-05-18

## Context

Parent agent sessions deliver user-visible output by dispatching
`Channel.SendReply`. The `send_reply` WASM integration then posts to the
transport webhook and records `ReplyDelivered` or `ReplyFailed`.

Production showed the reply path could surface low-level host timeout text when
provider/runtime pressure caused an upstream operation to exceed the host HTTP
deadline. The Channel entity already owned delivery state, but the trigger had
no explicit per-delivery budget documented in the app spec.

## Decision

Keep reply delivery Temper-native: `Channel.SendReply` remains the entity action
that owns reply delivery, and `send_reply` remains the WASM integration that
owns the webhook call and delivery audit state.

The `send_reply` trigger now has an explicit 30 second timeout budget. If
delivery cannot complete inside that budget, the Channel stays `Connected` and
records `ReplyFailed` through the existing failure action.

## Consequences

Reply delivery failures are bounded and visible in Channel state history. This
does not bypass the Channel entity or add an imperative retry loop; it only
turns an implicit host boundary into an app-level contract.

## Verification

Regression coverage is in
`crates/temperpaw/tests/session_turn_architecture.rs`:

- `channel_send_reply_trigger_is_bounded_and_reports_delivery_failure`
