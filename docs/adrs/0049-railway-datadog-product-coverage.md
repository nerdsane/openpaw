# ADR-0049: Railway Datadog Product Coverage

## Status

Accepted

## Context

TemperPaw production stays on Railway and keeps the Railway Postgres database.
The application instrumentation stays OpenTelemetry-native where possible, while
Railway deployments may add Datadog-specific runtime services when that is the
honest way to turn Datadog product setup indicators green. This ADR deliberately
does not introduce Linux Compose, Kubernetes, or database migration work.

- No Linux Compose
- No Kubernetes
- No database migration

Datadog product surfaces are not all the same product boundary. APM and OTLP
ingest can run through a Railway-hosted Datadog Agent. LLM Observability needs
the `dd-otlp-source: llmobs` route, which Temper can send directly when the
collector is bypassed. USM depends on the Agent `system-probe`, host kernel
visibility, mounts, and Linux capabilities that Railway may not expose.
Continuous `ddprof` depends on OS profiling APIs and must be canary-proven on
Railway before it is treated as supported.

References:

- Datadog Agent OTLP ingest: https://docs.datadoghq.com/opentelemetry/setup/otlp_ingest_in_the_agent/
- Datadog OpenTelemetry ingestion options: https://docs.datadoghq.com/opentelemetry/ingestion_sampling/
- Datadog backend Error Tracking for logs: https://docs.datadoghq.com/logs/error_tracking/backend/
- Datadog Error Tracking for backend service spans: https://docs.datadoghq.com/tracing/error_tracking/
- Datadog USM setup: https://docs.datadoghq.com/universal_service_monitoring/setup/
- Datadog native profiler: https://docs.datadoghq.com/profiler/enabling/ddprof/

## Decision

Add a dedicated Railway `datadog-runtime-agent` service separate from the
existing `datadog-postgres-agent` DBM service. In Datadog-enhanced Railway mode,
the TemperPaw service sends runtime OTLP to:

```text
OTEL_EXPORTER_OTLP_ENDPOINT=http://datadog-runtime-agent.railway.internal:4318
DD_TRACE_AGENT_URL=http://datadog-runtime-agent.railway.internal:8126
DD_AGENT_HOST=datadog-runtime-agent.railway.internal
DD_SERVICE=temperpaw
DD_ENV=prod
DD_VERSION=<build sha>
TEMPER_DATADOG_RAILWAY_PROFILE=datadog-enhanced-railway
DD_LLMOBS_API_ENABLED=true
OTEL_RESOURCE_ATTRIBUTES=service.name=temperpaw,service.version=<build version>,deployment.environment=prod,dd_llmobs_enabled=false
```

The Runtime Agent enables APM intake, OTLP HTTP and gRPC ingest, log intake, the
process Agent, unified service tags, and Datadog trace intake. The existing
`otel-collector` remains deployed as the `portable-otel` fallback path and as
the explicit non-Datadog mode.

LLMObs in Datadog-enhanced Railway mode is sourced from Temper's direct LLMObs
API exporter. OTLP spans still go to the Runtime Agent for APM, but the app sets
`dd_llmobs_enabled=false` alongside the OTLP service identity so Datadog does
not also convert the APM trace into duplicate LLMObs rows with missing content.

For already-running Railway deployments, TemperPaw exposes a governed setup API
action at `/paw/infra/railway/datadog-runtime-agent/ensure`. It uses the stored
Railway deployment token server-side to create or reuse the Runtime Agent
service, apply the required Agent and app variables, persist the Runtime Agent
service id, and redeploy both services without returning infrastructure secrets
to callers.

The same setup surface exposes proof helpers for environments where the local
Railway CLI cannot access the production project. `GET
/paw/infra/railway/datadog-capability-check` reports the USM system-probe and
continuous `ddprof` host capabilities from inside the Railway container. `POST
/paw/infra/railway/datadog-continuous-profiler-canary` toggles only
`TEMPER_DDPROF_ENABLED` and `DD_PROFILING_ENABLED` on the TemperPaw service and
redeploys it, so continuous profiling can be canary-proven and then disabled
again.

For Error Tracking proof, `POST
/paw/infra/datadog/error-tracking-synthetic` emits a
`DatadogSyntheticBackendError` as both an errored OpenTelemetry span named
`datadog.error_tracking.synthetic` and a backend error log. The event includes
Datadog span fields `error.type`, `error.message`, and `error.stack`, plus log
fields `error.kind`, `error.message`, `error.stack`, and the OTel
`exception.*` mirror fields so the live proof can distinguish "Error Tracking
is supported" from "there was a plain error log but no Error Tracking issue."

## Product Classification

| Product | Classification | Railway proof bar |
|---|---|---|
| APM | supported | Runtime Agent APM setup indicator is green, or proof names an exact Agent/Railway blocker. |
| Logs correlation | supported | In-span logs carry decimal `dd.trace_id`/`dd.span_id` and link to APM traces. |
| Error Tracking | supported | Synthetic backend error appears with exception and Datadog-compatible error fields. |
| LLM Observability | supported | Agent session, turn/workflow, `llm.chat`, and tool spans arrive through direct LLMObs export or collector LLMObs routing. |
| On-demand Profiling | supported | Authenticated `/_admin/profile/cpu` captures and uploads a profile to Datadog. |
| Continuous Profiling | best-effort | A Railway canary with `TEMPER_DDPROF_ENABLED=true` must prove continuous profile intake and no OS perf permission failures. |
| USM | blocked-on-Railway-capability | USM is only supported if Railway can expose the required system-probe host mounts and capabilities. |

If continuous profiling fails because Railway denies perf APIs, record
`blocked-on-Railway-perf-permissions` and keep on-demand profiling supported. If
USM cannot run because Railway cannot provide system-probe requirements, record
`blocked-on-Railway-system-probe` instead of calling it a TemperPaw
misconfiguration.

## Consequences

- Datadog-enhanced production remains Railway-native and does not require
  operator-managed Linux Compose, Kubernetes, or database migration work.
- The deployment can honestly show which Datadog products are green and which
  are blocked by Railway capability boundaries.
- LLMObs remains readable when bypassing the collector because Temper can submit
  direct LLMObs spans with Datadog routing enabled.
- The `portable-otel` path remains available for local, future non-Datadog, and
  fallback deployments.
