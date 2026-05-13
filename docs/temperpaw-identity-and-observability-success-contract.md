# TemperPaw Identity and Observability Success Contract

Date: 2026-05-11

## Goal

Across `temperpaw` and `temper`, make TemperPaw the canonical active identity and build complete, useful Datadog observability for the working TemperPaw system.

This goal has two workstreams:

1. Remove active OpenPAW/OpenPaw/openpaw residue from runtime identity, deployment, Modal bridge configuration, data/document services, dashboards, monitors, pipelines, agent surfaces, and user-facing docs.
2. Instrument TemperPaw and Temper comprehensively so humans and agents can debug, improve, and operate the system from Datadog using traces, metrics, logs, profiling, Postgres database monitoring, LLM observability, dashboards, monitors, facets, pipelines, and runbooks.

## Scope Clarification

OpenPAW consolidation is naming, identity, configuration, documentation, and runtime-service cleanup. It does not imply moving business flow or orchestration into a different architecture.

Observability may require adding or improving shared tracing, metric, logging, profiling, or context-propagation primitives in Temper and TemperPaw. Those changes are allowed when they make telemetry accurate and complete. They must preserve the Temper-native rule: state lives in entities, logic reacting to state changes lives in WASM integrations, and authorization lives in Cedar policies.

No ad hoc orchestration layer should be introduced just to make telemetry easier.

## Success Criteria

### OpenPAW Consolidation

- Active runtime surfaces use `TemperPaw` or `temperpaw`, not `OpenPAW`, `OpenPaw`, or `openpaw`.
- Modal bridge deployment/configuration, Railway/Vercel deployment configuration, data/document service references, dashboard settings, agent instructions, skill/tool surfaces, secrets, and environment examples are renamed or migrated consistently.
- Datadog dashboards, monitors, log metrics, pipelines, facets, service tags, team tags, alert messages, and Slack routing no longer use OpenPAW identity unless explicitly retained as historical context.
- Repository scans produce either zero active OpenPAW matches or a reviewed allowlist of historical ADR/migration references.
- Any historical allowlist explains why the old name remains and proves it cannot affect runtime behavior, deployment, user experience, or observability.

### No Architecture Regression

- OpenPAW cleanup does not restructure business behavior unless required to remove an actual active dependency on the old identity.
- Observability support primitives may be added where needed, but they must be reusable platform/app instrumentation, not hidden imperative business flow.
- Material platform, app, trigger, entity spec, WASM, Cedar, deployment, storage, or agent-capability changes have an ADR in the appropriate repo.
- The final proof or PR notes explicitly call out any change judged too small for an ADR.

### Complete Datadog Coverage

- Unified tagging is consistent across telemetry: `env`, `service`, `version`, plus stable TemperPaw tags such as tenant, entity type, entity id, action, state, session id, turn id, agent id, model provider, model name, tool name, sandbox provider, deployment id, and workflow/cycle id where applicable.
- Traces, metrics, and structured logs correlate through trace id and span id.
- Runtime entity actions, state transitions, trigger ingress, WASM integrations, agent sessions, LLM calls, tool calls, sandbox operations, external API calls, deployment operations, and database operations are observable.
- WASM observability is explicit host/runtime instrumentation, not an assumption that APM can see inside guest code. Host-boundary spans cover HTTP/text, binary and streaming calls, Connect RPC, secret lookup, spec evaluation, stream/cache/blob field reads and hashes; guest logs and `wasm_guest.progress` events carry `wasm_module`, `workflow_step`, `progress.kind`, trace/span ids, session id, entity id, action, and workflow root/run context.
- Continuous profiling is configured for Rust/compiled services where the deployment environment supports it.
- Postgres Database Monitoring is configured and correlated with APM so database load and slow queries can be tied back to TemperPaw services and traces.
- LLM Observability captures provider, model, latency, errors, token usage, cost-relevant fields when available, session id, prompts/completions where safe and allowed, and agent/tool/workflow structure.
- Sensitive-data handling is explicit. Secrets, credentials, and unsafe payload fields are redacted or excluded while preserving enough metadata for diagnosis.

### Agent Session Trace Quality

A completed TemperPaw agent session must have a useful end-to-end trace, not merely scattered spans.

- The root span represents the session, for example `temperpaw.agent.session`.
- The trace expands into meaningful child spans for turns, context preparation, provider/auth gate, LLM call, response application, tool batch, individual important tool calls, sandbox exec/file operations, entity actions, WASM integration execution, database calls, approvals, recovery, compaction, and external API calls.
- WASM integrations expand through useful host-boundary spans such as `wasm.host.get_secret`, `wasm.host.evaluate_spec`, `wasm.host.connect_call`, `wasm.host.cache_contains`, `wasm.host.cache_to_stream`, `wasm.host.cache_from_stream`, `wasm.host.read_field`, and `wasm.host.hash_stream`, plus correlated `wasm_guest.progress` and guest-log events.
- Claims of inside-WASM APM spans are prohibited unless Temper exposes and tests an explicit guest-to-host span API. ADR-0086 records the current design.
- High-frequency or tiny details become span events or structured logs, not a flood of tiny child spans.
- Asynchronous or event-sourced work uses correct trace-context propagation or span links instead of misleading parent-child stitching.
- Chronology is clear: spans and wide events include timestamps plus stable sequence context such as state version, action index, turn id, tool id, entity id, and session id.
- The trace is long and expandable enough to show what happened and where time went, without repetitive duplicate spans.
- The trace can be queried by `session_id`, `trace_id`, `entity_id`, `turn_id`, `agent_id`, `model_provider`, `model_name`, `tool_name`, `action`, `state`, and error metadata.
- A human can open one session trace and identify bottlenecks, failures, slow external calls, slow DB operations, LLM latency, tool latency, and recovery behavior.
- An agent can query the same telemetry and produce a useful diagnosis without hidden local context.

### Human Usability

- Datadog has organized dashboards for TemperPaw health, Temper platform health, agent/session health, LLM behavior, database behavior, trigger/transport health, WASM integration health, deployment health, and cost/usage-relevant signals.
- Monitors have clear names, ownership tags, service/env scoping, actionable messages, and routing.
- Facets and log pipelines make the important fields searchable without requiring memorized JSON paths.
- A human can answer from Datadog:
  - Is TemperPaw healthy right now?
  - Which service, agent, session, entity, action, or WASM integration is failing?
  - What changed during the latest deployment?
  - Is the database slow, blocked, overloaded, or being hit by a specific service/action?
  - Are LLM calls slow, expensive, retrying, failing, or producing poor outputs?
  - Are tools, sandboxes, triggers, transports, and external APIs healthy?

### Agent Usability

- Agent-facing documentation and tool surfaces explain how to query Datadog for metrics, logs, monitors, traces, session telemetry, LLM telemetry, and database symptoms.
- Names, tags, and query patterns are predictable and stable enough for agents to use programmatically.
- An agent can inspect a production symptom, map it to session/entity/action state, and decide whether to heal, tune, escalate, or continue investigating.
- The final proof includes at least one agent-oriented diagnostic path.

### Verification

- Red-green TDD is followed for code, spec, policy, and integration changes.
- Full relevant builds and test suites pass in both repositories.
- The local or deployed system boots cleanly.
- End-to-end flows are exercised against a working TemperPaw system, including trigger/event ingress, entity transitions, agent/session work, LLM call path, database write/read path, and Datadog telemetry emission.
- Datadog is verified directly: traces, logs, metrics, dashboards, monitors, profiling, DBM, and LLM observability are checked in Datadog or through Datadog APIs.
- `.proofs/` evidence records commands, outputs, Datadog query results or links, screenshots when useful, remaining limitations, and any allowlisted historical OpenPAW references.
- Synthetic telemetry may be used to test pipelines, but final success requires observing telemetry from the actual working TemperPaw system.

## Final Deliverable

When implementation and verification are complete, produce a human-facing observability guide that teaches what was set up and how to use it.

The guide must cover:

- What a human can observe in Datadog from the actual working TemperPaw system.
- The main dashboards, monitors, facets, pipelines, traces, DBM views, profiler views, and LLM Observability views.
- How to inspect one agent session end to end.
- How to follow a symptom from alert to trace to logs to entity/action state to database or LLM root cause.
- How agents should query and interpret observability.
- Known gaps, vendor/deployment limitations, and follow-up work if anything cannot be fully observed.

The guide is not complete unless it is backed by proof from a running TemperPaw system.

## Datadog Baselines

Implementation should be checked against current Datadog guidance for:

- [Unified Service Tagging](https://docs.datadoghq.com/getting_started/tagging/unified_service_tagging/)
- [Trace Explorer](https://docs.datadoghq.com/tracing/trace_explorer/)
- [Trace View](https://docs.datadoghq.com/tracing/trace_explorer/trace_view/)
- [Span Links](https://docs.datadoghq.com/tracing/trace_collection/span_links/)
- [Logs and Traces Correlation](https://docs.datadoghq.com/tracing/other_telemetry/connect_logs_and_traces/)
- [LLM Observability](https://docs.datadoghq.com/llm_observability/)
- [LLM Observability Querying](https://docs.datadoghq.com/llm_observability/monitoring/querying/)
- [Database Monitoring and APM Correlation](https://docs.datadoghq.com/database_monitoring/connect_dbm_and_apm/)
- [Native Profiler for Compiled Languages](https://docs.datadoghq.com/profiler/enabling/ddprof/)
