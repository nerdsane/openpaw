# ADR-032: Provider LLMObs Export Boundary

- Status: Accepted
- Date: 2026-06-08

## Context

ADR-030 added structured `gen_ai.output.messages` to provider guest spans, but
production Datadog traces still rendered `No content`. Live traces showed that
short GenAI metadata arrived, while content fields and content span events did
not. The deployed provider also ignored every `WasmSpan` result when adding
events, setting attributes, and ending the span.

The provider response can contain large tool-call or text payloads. Sending
uncapped legacy `gen_ai.completion` through the WASM host boundary makes the
content export fragile even when structured output messages are compacted.

## Decision

`provider_caller` bounds the legacy `gen_ai.completion` attribute before it is
sent to the guest-span host API. The cap is below Temper's host span-attribute
truncation budget so caller payloads stay small enough to cross the host
boundary.

`provider_caller` also logs warn-level messages when a guest-span content export
operation fails instead of discarding the result.

The platform-side fix lives in Temper ADR-0136: Temper now declares GenAI
content fields as Datadog-visible static tracing fields.

## Consequences

- Datadog should receive bounded `gen_ai.output.messages` and
  `gen_ai.completion` content attributes for successful provider calls.
- If a span event, attribute update, or span end call fails, the failure becomes
  visible in production logs.
- Very large completions are truncated in observability only; provider response
  artifacts remain unchanged.

## Verification

- Provider unit tests cover bounded legacy completion payloads.
- Temper unit tests cover the GenAI content fields in the Datadog-visible span
  field allowlist.
- Production verification must query Datadog for `@gen_ai.output.messages:*`
  after the new Temper rev is pinned and deployed.
