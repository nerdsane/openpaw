# ADR-030: Provider LLMObs Output Messages

- Status: Accepted
- Date: 2026-06-04

## Context

TemperPaw provider spans already set `gen_ai.input.messages` and
`gen_ai.system_instructions`, but successful provider calls only added the
assistant response through a legacy `gen_ai.completion` string. Datadog's
OpenTelemetry LLM Observability mapping treats direct `gen_ai.output.messages`
attributes, or `gen_ai.client.inference.operation.details` span events carrying
that same attribute, as the structured output source for LLM spans.

When `gen_ai.output.messages` is absent, the provider span can appear in the
LLMObs UI with useful metadata but no rendered response content.

## Decision

`provider_caller` must attach structured output messages on successful LLM
guest spans:

- Keep `gen_ai.completion` for legacy compatibility.
- Set `gen_ai.output.messages` using the same normalized message format already
  used by session-turn artifacts.
- Add a `gen_ai.client.inference.operation.details` span event containing
  `gen_ai.output.messages` as a second Datadog-supported extraction path.

This stays inside the provider WASM integration because the provider response
is known there, and because no separate orchestration layer should own LLMObs
content.

## Consequences

- Datadog LLM Observability should render provider responses as content instead
  of `No content` for successful LLM calls.
- Tool-only responses still get a compact textual summary plus tool-call parts,
  preserving readability without inventing a separate response shape.
- The observability contract test now requires `gen_ai.output.messages` and the
  Datadog semantic event name on provider spans.

## References

- Datadog OpenTelemetry LLMObs mapping:
  https://docs.datadoghq.com/llm_observability/instrumentation/otel_instrumentation/
