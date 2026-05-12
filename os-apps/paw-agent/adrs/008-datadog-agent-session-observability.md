# ADR-008: Datadog Agent Session Observability Contract

- Status: Accepted
- Date: 2026-05-11

## Context

TemperPaw sessions already emit state-transition metrics, staged turn metrics,
provider usage logs, OTS trajectories, and some GenAI span-hint headers. That is
not enough for the operator goal: a human or agent must be able to open one
session in Datadog and follow the chronological execution story from ingress,
through Session state transitions, WASM integrations, LLM calls, tool calls,
database work, approvals, recovery, and terminal state.

ADR-0037 defines end-to-end traceparent propagation. This ADR narrows the
TemperPaw app contract for Datadog so the session trace, dashboards, monitors,
facets, and runbooks remain consistent while Temper supplies the platform-level
span plumbing.

## Decision

TemperPaw treats an agent session as the primary observability unit.

- A complete session trace has a stable root named `temperpaw.agent.session`.
- The root span is an LLM Observability `agent` span. For OTLP conversion this
  means `gen_ai.operation.name=invoke_agent` or `create_agent`, because Datadog
  maps those operations to `span.kind=agent`.
- Session, turn, agent, entity, action, state, tool, model, provider, workflow,
  and error fields are searchable as Datadog facets where they appear in logs.
- LLM spans and logs use OpenTelemetry `gen_ai.*` semantic attributes and are
  routed to Datadog LLM Observability without duplicating ordinary APM spans.
- LLM calls use `gen_ai.operation.name=chat`, `gen_ai.provider.name`,
  `gen_ai.request.model`, `gen_ai.response.model`, token usage attributes, and
  captured input/output messages so Datadog can classify them as `llm` spans.
- Tool calls use `gen_ai.operation.name=execute_tool` or an equivalent Datadog
  LLMObs tool-span ingestion path. The span name must be stable enough for
  dashboards while `tool.name` and `tool.call_id` carry the specific call.
- Parent-child links are part of the contract. Datadog maps OTLP
  `parent_span_id` into LLMObs `parent_id`, so any LLM/tool span with
  `parent_id=undefined` is treated as a broken session trace unless it is a
  deliberately standalone trace root.
- Database views include Postgres DBM/APM correlation instructions and dashboard
  links so slow queries can be followed back to `service:temperpaw` traces.
- Postgres DBM must be verified from query samples or plans by showing that DBM
  activity is attributable to upstream APM service `temperpaw` and can pivot
  back to the relevant trace/session. A monitor without DBM sample evidence is
  not sufficient proof.
- Dashboards and monitors are organized around operator questions: system
  health, session health, LLM health, database health, trigger/transport health,
  WASM health, deployment health, and profiling.
- Fine-grained repeated activity is represented as span events, structured
  logs, metrics, or OTS trajectory rows unless it is important enough to be an
  expandable child span.

## Consequences

- Datadog assets are part of the app contract and have tests.
- Any future provider, tool, trigger, or transport path must preserve the same
  session-facing fields and query vocabulary.
- Temper changes are still required for host-level traceparent/span parenting;
  this ADR does not add a second orchestration layer in TemperPaw.
- Proof of completion must include live Datadog evidence from a working
  TemperPaw session, not only synthetic telemetry or static config checks.

## References

- Datadog LLMObs OTLP mapping:
  https://docs.datadoghq.com/llm_observability/instrumentation/otel_instrumentation/
- Datadog LLMObs trace querying:
  https://docs.datadoghq.com/llm_observability/monitoring/querying/
- Datadog DBM/APM correlation:
  https://docs.datadoghq.com/database_monitoring/connect_dbm_and_apm/
