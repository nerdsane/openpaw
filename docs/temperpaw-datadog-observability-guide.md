# TemperPaw Datadog Observability Guide

Status: Draft. This guide becomes the final operator deliverable only after live
Datadog verification shows real `service:temperpaw` traffic, session-root traces,
LLMObs agent/tool/LLM hierarchy, Postgres DBM/APM correlation, profiling data,
and reconciled dashboards/monitors/log metrics.

## Primary Questions

TemperPaw observability is organized around the questions a human or agent asks
during operations:

- Is TemperPaw receiving traffic and producing telemetry under `service:temperpaw`?
- Which agent session is slow, failed, blocked, or waiting for approval?
- What happened chronologically inside one session?
- Which LLM provider/model/tool/database call consumed time or failed?
- Are logs, metrics, traces, LLMObs, DBM, and profiling all correlated by the
  same session/trace vocabulary?
- Are any Datadog assets still using legacy product identity?

## Core Query Vocabulary

Use these fields first. They are intended to be shared across traces, logs,
dashboards, monitors, and agent diagnostics.

| Concept | Datadog field |
| --- | --- |
| Service | `service:temperpaw` |
| Session | `@session_id:<session id>` |
| Managed session bridge | `@observability_event:temperpaw.agent.session`, `@managed_session_id:<id>`, `@inner_session_id:<id>`, `@parent_session_id:<id>` |
| Agent | `@agent_id:<agent id>` |
| Managed agent environment | `@inner_agent_id:<id>`, `@managed_agent_id:<id>`, `@environment_id:<id>` |
| Turn | `@turn_id:<turn id>` |
| Entity | `@entity_type:<type>` and `@entity_id:<id>` |
| Action | `@action_name:<action>` |
| State transition | `@from_status:<state>` and `@to_status:<state>` |
| Trace/log join | `@dd.trace_id:<trace id>` and `@dd.span_id:<span id>` |
| LLM provider/model | `@gen_ai.provider.name:<provider>`, `@gen_ai.request.model:<model>` |
| Tool call | `@tool.name:<tool>` and `@tool.call_id:<call id>` |
| Channel transport | `@observability_event:temperpaw.transport`, `@transport.name:<slack|discord>`, `@transport.operation:<operation>`, `@transport.outcome:<outcome>` |
| Webhook trigger | `@observability_event:temperpaw.webhook`, `@webhook.route_key:<route key>`, `@webhook.event_id:<event id>`, `@webhook.outcome:<outcome>` |
| Governance approval | `@observability_event:temperpaw.approval`, `@decision_id:<decision id>`, `@approval.operation:<operation>`, `@approval.outcome:<outcome>` |
| TemperFS/blob | `@workspace_id:<id>`, `@file_id:<id>`, `@content_hash:<sha256>`, `@blob.operation:<put|get>` |
| Workflow/deploy | `@workflow.cycle_id:<id>`, `@deployment.id:<id>` |
| Errors | `status:error`, `@error.kind:*`, `@error.message:*` |

## Agent Session Trace

The expected trace shape is one coherent tree per agent session:

1. Root span: `temperpaw.agent.session`.
2. Root span LLMObs kind: `agent`, represented for OTLP by
   `gen_ai.operation.name=invoke_agent` or `create_agent`.
3. Child spans for session turns, context preparation, WASM integrations, tool
   calls, LLM calls, approvals, recovery paths, external I/O, Postgres calls,
   and terminal state.
4. LLM spans use `gen_ai.operation.name=chat`.
5. Tool spans use `gen_ai.operation.name=execute_tool` with `tool.name` and
   `tool.call_id`.
6. Repeated low-value inner details are span events, logs, metrics, or OTS rows
   instead of thousands of tiny child spans.

Useful searches:

```text
service:temperpaw operation_name:temperpaw.agent.session @session_id:<session id>
@dd.trace_id:<trace id>
@session_id:<session id> status:error
```

TemperPaw's managed-agent bridge now emits app-side span hints on the meaningful
session boundaries: `ManagedAgents.StartSession` / `ManagedAgents.ResumeSession`
when they configure or steer the inner `TemperPaw.Session`. Those hints use
`temperpaw.agent.session`, `gen_ai.operation.name=invoke_agent`,
`session_id`, `managed_session_id`, `inner_session_id`, `agent_id`,
`parent_session_id`, `environment_id`, `entity_type=ManagedSession`,
`entity_id`, and `action_name`. The polling monitor path intentionally does not
emit session root hints so traces do not fill with repetitive check spans.

The managed `SessionEvent` chronology carries the same bridge context on the
high-signal rows: `session.status_running`, derived `agent.message`,
`agent.thinking`, `agent.tool_use`, `agent.tool_result`, `session.status_idle`,
and `session.status_terminated`. Each row has top-level
`observability_event=temperpaw.agent.session`, `managed_session_id`,
`inner_session_id`, `inner_agent_id`, `managed_agent_id`, `parent_session_id`,
`environment_id`, and `action_name` fields. Use that entity timeline as the
Temper-native chronology when live trace parenting is missing or incomplete.

Broken-trace signs:

- LLMObs spans for TemperPaw have `parent_id: undefined` when they are not
  deliberate roots.
- The only visible LLMObs span is a single `llm` span such as
  `wasm:provider_caller`.
- APM has useful platform spans, but LLMObs shows a separate root instead of the
  same chronological session.
- Root trace duration hides long background children or appears much shorter
  than the session wall-clock time.

## Agent Query Surface

Agents use `temper.datadog_query({...})` for credential-gated Datadog reads. The
tool returns compact JSON summaries rather than raw Datadog payloads.

Useful query kinds:

```python
temper.datadog_query({"query_kind": "logs_query", "query": "service:temperpaw @session_id:<session id>", "from": "now-30m", "to": "now"})
temper.datadog_query({"query_kind": "trace_query", "query": "service:temperpaw operation_name:temperpaw.agent.session", "from": "now-30m", "to": "now"})
temper.datadog_query({"query_kind": "llmobs_query", "ml_app": "temperpaw", "span_kind": "agent", "from": "now-30m", "to": "now", "include_attachments": False})
temper.datadog_query({"query_kind": "dbm_query", "query": "dbm_type:activity service:temperpaw", "from_ts": 1778510000, "to_ts": 1778513600})
temper.datadog_query({"query_kind": "profiling_query", "query": "sum:datadog.profiling.rust.profiles_uploaded{service:temperpaw}.as_count()", "from_ts": 1778510000, "to_ts": 1778513600})
```

Supported surfaces are `monitor_status`, `recent_events`, `metrics_query`,
`logs_query`, `trace_query`, `llmobs_query`, `dbm_query`, and
`profiling_query`.

## Logs

Log Explorer should answer "what happened around this session or trace?" without
guesswork.

Use:

```text
service:temperpaw @session_id:<session id>
service:temperpaw @dd.trace_id:<trace id>
service:temperpaw @tool.name:<tool> status:error
service:temperpaw @gen_ai.request.model:<model>
```

Required facets are defined in `dd-pipelines/facets.json`. The pipeline must
avoid treating entity state as Datadog severity; state belongs in fields such as
`state`, `from_status`, `to_status`, or `entity_status`, while log severity comes
from the actual log level.

Sensitive-data scanner source rules live in
`dd-pipelines/sensitive-data-scanner.json`. They redact Datadog key assignments,
OpenAI/Anthropic keys, GitHub tokens, Slack tokens, email addresses, and AWS
access key IDs at ingest when the Datadog scanner is applied.

## Channel Transports

Slack and Discord transports are the first user-facing edge for messages,
slash commands, approvals, and reply delivery. Debug them before blaming the
agent loop when a user says nothing happened.

Useful Log Explorer pivots:

```text
service:temperpaw @observability_event:temperpaw.transport
service:temperpaw @transport.name:slack
service:temperpaw @transport.name:discord
service:temperpaw @transport.operation:receive_message
service:temperpaw @transport.operation:slash_command
service:temperpaw @transport.outcome:error
service:temperpaw @transport.channel_id:<channel id>
service:temperpaw @transport.message_id:<message id>
```

Transport logs expose platform, channel id, message id, operation, outcome,
message length, command name, and webhook port. They intentionally do not log
message bodies. The `[TemperPaw] Channel Transport Dispatch Failures` monitor
fires when inbound Slack/Discord messages fail before or during
`Channel.ReceiveMessage` dispatch.

## Webhook Triggers

Webhook triggers are the first HTTP edge for external event ingestion. They
must create one `WebhookEvent`, dispatch one `Received` action, and return
immediately; downstream routing belongs to WASM integrations.

Useful Log Explorer pivots:

```text
service:temperpaw @observability_event:temperpaw.webhook
service:temperpaw @webhook.route_key:<route key>
service:temperpaw @webhook.event_id:<event id>
service:temperpaw @webhook.operation:receive
service:temperpaw @webhook.operation:dispatch_received
service:temperpaw @webhook.outcome:error
```

Webhook logs expose route key, created event id, operation, outcome, HTTP-style
status, and payload byte length. They intentionally do not log raw payload
bodies. If a webhook error fires, pivot from `webhook.event_id` to the
`WebhookEvent` entity before investigating channel transport or agent sessions.

## Governance Approvals

Approval waits have two sides: the agent session paused on a Cedar
`GovernanceDecision`, and the human notification path that delivers buttons
through the bound channel. Debug the approval path before assuming the agent or
LLM is stuck.

Useful Log Explorer pivots:

```text
service:temperpaw @observability_event:temperpaw.approval
service:temperpaw @decision_id:<decision id>
service:temperpaw @session_id:<session id>
service:temperpaw @approval.operation:register_callback
service:temperpaw @approval.operation:notify_human
service:temperpaw @approval.outcome:error
service:temperpaw @approval.delivery:skipped
```

Approval logs expose `decision_id`, `session_id`, `agent_id`,
`parent_session_id`, `active_plan_id`, `approval.operation`,
`approval.outcome`, `approval.delivery`, `approval.reason`, `approval.action`,
and `approval.http_status`. They intentionally do not duplicate the human
notification body. If notification fails, pivot from the same time window into
`@observability_event:temperpaw.transport` for Slack/Discord delivery details.

## TemperFS Blob & Document Services

TemperFS is the document/data backbone for agent plans, session artifacts,
workspace files, app documentation, prepared context files, and large content
externalized from entity fields. Operators should debug it as its own service
surface instead of treating every file symptom as an LLM or sandbox failure.

Useful Log Explorer pivots:

```text
service:temperpaw @observability_event:temperpaw.blob
service:temperpaw @observability_event:temperpaw.fs
service:temperpaw @workspace_id:<workspace id>
service:temperpaw @file_id:<file id>
service:temperpaw @content_hash:<sha256>
service:temperpaw @fs.operation:create_file
service:temperpaw @blob.operation:put @blob.cache_hit:false
```

The `blob_adapter` WASM emits structured fields through `fields_json`; the
Datadog pipeline parses those into facets such as `workspace_id`, `file_id`,
`content_hash`, `stream_id`, `content_type`, `blob.operation`,
`blob.backend`, `blob.cache_hit`, `blob.status_code`, and `blob.size_bytes`.
The `workspace_fs` WASM emits `observability_event=temperpaw.fs` for metadata
operations with `workspace_id`, `fs.operation`, `fs.path`, `fs.outcome`, and
`fs.backend`.

Dashboard metrics to start with:

- `temper_blob_io_wait_duration_ms` for local blob backpressure.
- `temper_blob_local_fast_path_requests_total` for in-process local blob route
  usage.
- `temper_blob_transport_wait_duration_ms` for remote R2/S3 transport
  backpressure.
- `temper_blob_transport_requests_total` for remote blob request volume.
- `temper_session_large_content_externalized_total` for large session content
  spilling into blob-backed storage.

If a session appears slow while reading/writing plans, wiki pages, prepared
context files, screenshots, or other documents, first pivot from the session
trace/logs to `workspace_id` and `file_id`, then check blob wait metrics and
structured `temperpaw.blob` events for cache misses or remote transport errors.

## Sandbox & Modal Bridge

Sandbox work should be observable as both host HTTP bridge metrics and structured
operation logs. The metrics show whether the bridge is slow or failing; the logs
show which sandbox and operation were affected.

Useful Log Explorer pivots:

```text
service:temperpaw @observability_event:temperpaw.sandbox
service:temperpaw @sandbox_provider:modal
service:temperpaw @sandbox_id:<sandbox id>
service:temperpaw @sandbox.operation:bash
service:temperpaw @modal_bridge.operation:create
service:temperpaw @modal_bridge.endpoint:exec
```

Dashboard metrics to start with:

- `temper_wasm_host_http_requests_total{service:temperpaw,call_kind:text}` by
  `status_code_class`.
- `temper_wasm_host_http_duration_ms{service:temperpaw,call_kind:text}` by
  `status_code_class`.

Structured sandbox logs expose `sandbox_provider`, `sandbox_id`,
`sandbox.operation`, `sandbox.backend`, `sandbox.exit_code`,
`sandbox.status_code`, and `sandbox.workdir`. Modal bridge logs also expose
`modal_bridge.operation`, `modal_bridge.endpoint`, and
`modal_bridge.duration_ms`. When Modal bridge calls fail before a sandbox id
exists, inspect runtime configuration for `modal_bridge_url` and then look at
host HTTP 5xx/latency metrics for the same time window.

## LLM Observability

Open LLM Observability for `ml_app:temperpaw`.

Expected views:

- Agent roots: `span_kind:agent`, root name `temperpaw.agent.session`.
- LLM calls: `span_kind:llm`, `gen_ai.operation.name=chat`.
- Tool calls: `span_kind:tool`, `gen_ai.operation.name=execute_tool`.
- Group by provider/model/token usage to find cost and latency drivers.

Useful searches:

```text
ml_app:temperpaw span_kind:agent
ml_app:temperpaw @tags:"session_id:<session id>"
ml_app:temperpaw @status:error
```

For agents, the intended workflow is:

1. Search LLMObs for the session or recent agent roots.
2. Fetch the trace tree.
3. Use the agent-loop view when an `agent` span is present.
4. Inspect child span details only where duration, errors, token use, or tool
   output indicate a bottleneck.

## Metrics And Monitors

Dashboard and monitor assets live in:

- `dd-dashboards/temperpaw-overview.json`
- `dd-monitors/temperpaw-monitors.json`
- `dd-log-metrics/temper-log-metrics.json`
- `dd-pipelines/temper-temperpaw.json`
- `dd-pipelines/facets.json`

Credentialed reconciliation sequence:

```bash
python3 scripts/deploy_dashboard.py --reconcile
python3 scripts/deploy_monitors.py --reconcile
python3 scripts/deploy_pipelines.py --reconcile
```

These scripts require `DD_API_KEY`, `DD_APP_KEY`, and optionally `DD_SITE`.
Reconciliation is intended to update the desired TemperPaw assets and remove
stale migration-owned Datadog assets that are not part of the desired set.

The monitors must cover at least:

- traffic/no-data
- error rate
- request latency
- session phase budget failures
- missing agent-session trace roots
- LLM error and latency regressions
- Postgres DBM latency and APM correlation gaps
- profiling upload health
- deployment/monitor freshness

The Railway deploy path pins `DD_SERVICE=temperpaw`, `DD_ENV=prod`, and
`DD_TAGS=team:temperpaw` on the runtime service. When `DD_API_KEY` is present,
it also sets `DD_PROFILING_ENABLED=true` so the installed native profiler starts
with the application.
If Datadog is enabled later from Railway instead of during deploy, add
`DD_API_KEY` to both the collector service and the runtime service, and add
`DD_PROFILING_ENABLED=true` to the runtime service.

## Postgres DBM

Postgres DBM is only considered verified when Datadog shows real DBM query
samples or plans attributable to upstream `service:temperpaw`, and those samples
can pivot back to APM traces/session ids.

For OpenTelemetry trace ingestion, the Railway collector must start with
Datadog's operation/resource-name v2 feature gate so DB spans can be processed
for DBM correlation:

```text
--feature-gates=datadog.EnableOperationAndResourceNameV2
```

The collector also inserts `span.type=sql` on spans that carry `db.system` and
do not already have a Datadog span type. This keeps custom or partially
instrumented Postgres spans visible to DBM/APM correlation without changing
LLMObs routing.

When Datadog is enabled during a Railway Postgres deploy, `temperpaw deploy`
also creates a `datadog-postgres-agent` service based on `datadog/agent:7`.
That service runs the Postgres integration with `dbm: true`, `service:temperpaw`
and `team:temperpaw` tags, and Railway Postgres variable references such as
`${{Postgres.PGHOST}}` and `${{Postgres.PGPASSWORD}}`.

Useful DBM filters:

```text
source:postgres service:temperpaw
service:temperpaw @db.system:postgresql
@query_signature:<signature>
```

Completion evidence must include at least one query sample or plan, not just a
monitor definition.

## Profiling

Profiling is expected for the Rust TemperPaw/Temper runtime. Operators should be
able to answer:

- Which functions dominate CPU during a slow session?
- Is memory growing during replay, hydration, dispatch, or WASM execution?
- Are profile uploads healthy for `service:temperpaw`?

Profiling completion requires live profile data and no stalled-upload monitor
for the active TemperPaw service.

## Current Verification Status

As of the live baseline proof in `.proofs/074-temperpaw-datadog-live-baseline.md`:

- Repo-local Datadog assets and identity tests are improved.
- Live Datadog still shows active traffic primarily under the legacy service
  identity.
- `ml_app:temperpaw` has no agent-root spans over the checked 24-hour window.
- The legacy LLM application still shows root `llm` spans with
  `parent_id: undefined`.
- Local TemperPaw managed-session start/resume paths now emit session-root span
  hints, but live Datadog has not shown those hints from deployed
  `service:temperpaw` traffic yet.
- Local Datadog API credentials are not present, so dashboard/monitor/log-metric
  reconciliation cannot be applied from this workspace yet.
- Temper platform span-parenting remains blocked by PM issue/auth constraints in
  the Temper repo.
