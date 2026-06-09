# ADR-0064: LLMObs Direct API Export Boundary

## Status

Accepted

## Context

TemperPaw production sends runtime OpenTelemetry spans to the Railway-hosted
Datadog Runtime Agent for APM. Temper also submits LLMObs spans directly to the
Datadog LLMObs API so agent, workflow, tool, and LLM content can use the exact
Datadog payload shape.

Datadog can automatically convert generative AI OpenTelemetry spans into
LLMObs rows. In TemperPaw this creates a second LLMObs source for the same
provider calls. The converted rows can be missing input or output content even
when the direct LLMObs API row has content, which makes the LLMObs UI show
`No content` for fresh runs.

## Decision

Datadog-enhanced Railway deployments keep APM OTLP export enabled, keep direct
LLMObs API export enabled, and explicitly disable Datadog's automatic
OTel-to-LLMObs conversion by setting:

```text
OTEL_RESOURCE_ATTRIBUTES=service.name=temperpaw,service.version=<build version>,deployment.environment=prod,dd_llmobs_enabled=false
DD_LLMOBS_API_ENABLED=true
```

The deployment CLI and governed Railway setup API both write this boundary so
new deployments and hot setup repairs converge on the same behavior.

## Consequences

- APM remains populated from OTLP spans through the Datadog Runtime Agent.
- LLMObs rows come from the direct Temper LLMObs API exporter only.
- Duplicate converted LLMObs rows no longer appear with empty `input` or
  `output` content.
- Older Datadog LLMObs rows are not rewritten; proof must query a time window
  after the variable is deployed.
