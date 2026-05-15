# ADR-009: WASM Guest Observability API Adoption

- Status: Accepted
- Date: 2026-05-14

## Context

TemperPaw provider and tool execution currently relies on `X-Temper-Span-*`
headers to ask the Temper host to rename outbound HTTP spans. That preserves
Datadog visibility for external calls, but it does not represent pure
in-WASM work and cannot attach post-response attributes without extra callback
paths.

## Decision

TemperPaw adopts Temper ADR-0087 for key guest work:

- `provider_caller` wraps provider execution in a `tool.llm_call` guest span and
  records provider/model/session/prompt/response attributes directly.
- `monty_repl` wraps tool execution in `tool.<name>` guest spans with
  `tool.name` and `tool.call_id` attributes, so downstream host calls appear as
  children.
- Managed-agent common helpers use guest spans for managed session dispatch
  boundaries while leaving legacy span hints available for older modules.

## Consequences

Datadog traces should show one continuous chain from the TemperPaw session and
WASM invocation into guest provider/tool spans, host-boundary spans, logs, and
metrics. Existing span hints remain a compatibility path, not the preferred API
for new guest-owned work.
