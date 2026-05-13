# TemperPaw Datadog Observability Guide

Status: Live verified for core production observability on 2026-05-13. Datadog
now shows real `service:temperpaw` traffic, ADR-0084 long-lived
`Session.workflow` APM roots, corrected LLMObs agent/workflow/LLM hierarchy,
live logs, chronological session traces, Postgres client spans, Postgres DBM
samples with full propagation and calling-service correlation, and Rust
profiling uploads. Dashboard, monitor, log-pipeline, and log-metric assets have
been applied to Datadog. Trace-analytics monitors now use the spans Datadog
actually receives. This guide is not final-complete until the remaining Railway
external resource naming, LLMObs agent-loop timeline, ManagedSession semantic
span-name export, and Datadog UI-only facet/scanner application gaps are all
closed.

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
| Trace/log join | `trace_id:<Datadog decimal trace id>`, `@otel.trace_id:<32-char hex trace id>`, `span_id:<span id>`, and `@otel.span_id:<span id>` |
| LLM provider/model | `@gen_ai.provider.name:<provider>`, `@gen_ai.request.model:<model>` |
| Tool call | `@tool.name:<tool>` and `@tool.call_id:<call id>` |
| WASM module/boundary | `@wasm_module:<module>`, `@trigger_action:<action>`, `@workflow_step:<step>`, `@progress.kind:<kind>` |
| Channel transport | `@observability_event:temperpaw.transport`, `@transport.name:<slack|discord>`, `@transport.operation:<operation>`, `@transport.outcome:<outcome>` |
| Webhook trigger | `@observability_event:temperpaw.webhook`, `@webhook.route_key:<route key>`, `@webhook.event_id:<event id>`, `@webhook.outcome:<outcome>` |
| Governance approval | `@observability_event:temperpaw.approval`, `@decision_id:<decision id>`, `@approval.operation:<operation>`, `@approval.outcome:<outcome>` |
| TemperFS/blob | `@workspace_id:<id>`, `@file_id:<id>`, `@content_hash:<sha256>`, `@blob.operation:<put|get>` |
| Workflow/deploy | `@workflow.cycle_id:<id>`, `@deployment.id:<id>` |
| Errors | `status:error`, `@error.kind:*`, `@error.message:*` |

## Agent Session Trace

The trace goal is one coherent tree per agent session. TemperPaw currently has
three related entry points that operators should read together:

1. Direct Session APM root: `Session.workflow`. This is the live ADR-0084 root
   span for a direct `TemperPaw.Session`, and it stays open across the
   background workflow rather than ending with the initial OData request.
2. Direct Session LLMObs root: `temperpaw.agent_session` with
   `span_kind:agent`.
3. Managed-agent bridge action spans: `ManagedSession.StartSession` and
   `ManagedSession.ResumeSession`, queryable with `@entity_type:ManagedSession`
   and `@action_name:(StartSession OR ResumeSession)`.

Under those roots, the useful children are session turns, context preparation,
WASM integrations, tool calls, LLM calls, approvals, recovery paths, external
I/O, Postgres calls, and terminal state. LLM spans use
`gen_ai.operation.name=chat`; tool spans use
`gen_ai.operation.name=execute_tool` with `tool.name` and `tool.call_id`.
Repeated low-value inner details belong in span events, logs, metrics, or OTS
rows instead of thousands of tiny child spans.

Useful searches:

```text
service:temperpaw @entity_type:ManagedSession @action_name:(StartSession OR ResumeSession)
service:temperpaw @entity_id:<managed session id>
trace_id:<Datadog decimal trace id>
@otel.trace_id:<32-char hex trace id>
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

Known live gap: direct `Session.workflow` roots are live-proven, but the
ManagedSession span-hint name `temperpaw.agent.session` is not yet exposed as a
searchable APM operation/resource in production. The live monitor therefore uses
the actual ManagedSession action spans above. Treat `temperpaw.agent.session` as
the intended managed-agent semantic name, not as the primary live search path,
until that export behavior is fixed and proven.

Broken-trace signs:

- LLMObs spans for TemperPaw have `parent_id: undefined` when they are not
  deliberate roots.
- The only visible LLMObs span is a single `llm` span such as
  `wasm:provider_caller`.
- APM has useful platform spans, but LLMObs shows a separate root instead of the
  same chronological session.
- Root trace duration hides long background children or appears much shorter
  than the session wall-clock time.

## WASM Host Boundary Visibility

Datadog APM does not automatically see inside a WASM guest. Temper therefore
makes the useful boundary observable from the host/runtime side, then correlates
guest logs and progress events with the active trace. Treat these as host-side
WASM boundary spans, not inside-WASM APM spans.

High-signal host spans to expect:

- `wasm.host.http_call`, `wasm.host.http_call_binary`, and
  `wasm.host.http_stream` for outbound HTTP, binary body work, and streaming
  calls.
- `wasm.host.connect_call` for Connect server-streaming RPC calls.
- `wasm.host.get_secret` for vault lookup timing and failure context; it records
  `secret.key`, never the secret value.
- `wasm.host.evaluate_spec` for host-side state-machine/spec evaluation.
- `wasm.host.cache_contains`, `wasm.host.cache_to_stream`,
  `wasm.host.cache_from_stream`, `wasm.host.read_field`, and
  `wasm.host.hash_stream` for stream/cache/blob-style guest host functions.

Each boundary span should carry `tenant`, `entity_type`, `entity_id`,
`trigger_action`, `action_name`, `session_id`, `wasm_module`, `workflow_step`,
`workflow_root_entity_type`, `workflow_root_entity_id`, and `workflow_run_id`
when that context exists. Guest progress emits a named `wasm_guest.progress`
event and searchable log with `progress.kind`, `workflow_step`, `tool.name`,
`success`, `trace_id`, `span_id`, `dd.trace_id`, and `dd.span_id`. Guest logs
use the same trace/session/module vocabulary.

Useful searches:

```text
service:temperpaw @wasm_module:<module>
service:temperpaw @session_id:<session id> @wasm_module:*
service:temperpaw @progress.kind:* @workflow_step:*
service:temperpaw resource_name:wasm.host.get_secret
service:temperpaw resource_name:wasm.host.http_stream @wasm_module:blob_adapter
```

Do not expect a dense span for every tiny guest statement. The intended shape is
session/workflow/action -> WASM invocation -> meaningful host boundary spans,
with smaller detail as `wasm_guest.progress` events, `wasm_guest.log` events,
metrics, and entity state. ADR-0086 in Temper documents why explicit
guest-created child spans are deferred until there is a tested host API such as
`host_start_span`, `host_end_span`, or `host_add_span_event`.

## Agent Query Surface

Agents use `temper.datadog_query({...})` for credential-gated Datadog reads. The
tool returns compact JSON summaries rather than raw Datadog payloads.

Useful query kinds:

```python
temper.datadog_query({"query_kind": "logs_query", "query": "service:temperpaw @session_id:<session id>", "from": "now-30m", "to": "now"})
temper.datadog_query({"query_kind": "trace_query", "query": "service:temperpaw @entity_type:ManagedSession @action_name:(StartSession OR ResumeSession)", "from": "now-30m", "to": "now"})
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
service:temperpaw trace_id:<Datadog decimal trace id>
service:temperpaw @otel.trace_id:<32-char hex trace id>
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

Published artifacts use the internal route `POST /api/files/publish-artifact`
and should show the trace shape
`POST /api/files/publish-artifact -> state.publish_file_artifact ->
state.read_file_stream_indexed` plus sibling `state.put_public_blob` and
`postgres.upsert_published_artifact` spans.
Datadog caught a live 500 on this route in trace
`895a791073db9e5dafb3b927caf8a266` at service version `sha-afeca721`; there
were no correlated logs for the lower-64 trace id, so the trace tree was the
primary diagnostic. Temper commit
`81760436f3302f50d50c539cf5b78865ee41b362` fixes that class by falling back to
current `File`/`FileVersion` entity state when the query projection is missing
or stale. Temper commit `6021d918d0f8daa88f0c9687f4e3c435a2568f4d` adds the
`state.put_public_blob` span with bucket, storage key, endpoint host, MIME type,
byte length, and HTTP status. Temper commit
`7b170cf71246e01c337e81062b54ea8c597b9293` adds backend-neutral
published-artifact metadata persistence for the Postgres production path. After
deploying a TemperPaw image that pins all three changes, verify the route with
Trace Explorer and expect the full span path without `status:error`.

Operator caveat from the 2026-05-13 live proof: a 200 response from
`publish-artifact` proves the API path returned successfully. It does not by
itself prove durable metadata persistence, successful object-store write, or a
readable public URL. Check for `state.put_public_blob` with HTTP 200, search the
trace/logs for `published artifact metadata persisted` with
`metadata_backend:postgres`, and confirm the trace contains
`postgres.upsert_published_artifact` with `published_artifacts` INSERT/SELECT
children. The old `published artifact metadata store unavailable` warning
should have count `0` for current production images. Then curl the returned
`public_url`.

Current production caveat: `PUBLISHED_BLOB_PUBLIC_BASE_URL` now points at
`https://temperpaw-assets.katagami.ai`, a TemperPaw-specific R2 custom domain
attached to the writable bucket. New returned public URLs should read back over
public Cloudflare DNS with HTTP 200 and a content hash matching the source file.
The old `assets.katagami.ai` host remains attached to
`katagami-published-assets`; changing the writable bucket to that bucket caused
R2 HTTP 403 with the current S3 credentials, so bucket-name cleanup still needs
new R2 S3 credentials or a planned object migration. Until then, the
user-facing public URL is fixed, but `PUBLISHED_BLOB_BUCKET` still contains the
legacy external bucket name and must remain allowlisted as a migration artifact.

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

- Direct Session LLMObs roots: `span_kind:agent`, root name
  `temperpaw.agent_session`.
- Managed-session APM roots: `temperpaw.agent.session`.
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
3. Use the LLMObs trace tree for the authoritative hierarchy. Use the
   agent-loop helper only when it returns a non-empty timeline.
4. Inspect child span details only where duration, errors, token use, or tool
   output indicate a bottleneck.

Live proof on 2026-05-12 showed the desired LLMObs shape for production session
`ss-019e1e65-c1ff-7ac3-bbf7-0feb6220fc7c`:

```text
agent temperpaw.agent_session
  workflow Session.ProviderAuthReady
    llm wasm:provider_caller
```

That trace had zero LLMObs errors and only `service:temperpaw`. The Datadog
agent-loop helper returned an empty timeline for the same direct-API trace, so
operators and agents should currently use `get_llmobs_trace`, the correlated APM
trace, and OData event history for chronology until direct LLMObs agent-loop
timeline enrichment is added.

The correlated APM trace was `c80f416a8f1a6c61c86d873747ca26e3`, and the
LLMObs trace was `265924810408958961160905709497239611107`. The APM tree showed
the chronological path through `Session.Configure`, `ProvisionWorkspace`,
`workspace_provisioner`, `WorkspaceReady`, `context_preparer`,
`ContextReady`, `ProviderAuthReady`, `provider_auth_gate`, `provider_caller`,
`ProviderResponseReady`, `provider_response_applier`, `CheckSteering`,
`steering_checker`, `FinalizeResult`, `agent_reply`, and
`emit_ots_trajectory`, with Postgres spans under the same trace.

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

Live reconciliation on 2026-05-12 applied dashboard
`TemperPaw — Platform Overview` (`mn4-k3k-i66`), updated or created the
TemperPaw monitors from `dd-monitors/temperpaw-monitors.json`, updated log
pipeline `TemperPaw / Temper Logs (ADR-0054)`, and verified log metrics
`temperpaw.logs.errors`, `temperpaw.logs.warns`, and
`temperpaw.logs.wasm.default_timeout_fallback`.

Datadog rejected one monitor definition during apply because
`default_zero(...)` cannot be paired with `on_missing_data:"resolve"`. The
source monitor now uses `on_missing_data:"default"` and validates through
Datadog's monitor validator.

The agent-session trace-correlation monitor is intentionally event-gated. It
alerts only when a `temperpaw.agent.session` log event is emitted without a
trace id, so idle managed-session traffic does not create a false missing-trace
alert. The Postgres DBM monitor alerts on reliable DBM activity rows; its
runbook includes the same SQL span attributes that live Trace Explorer returns
for APM correlation:

```text
service:temperpaw type:sql @db.system:postgresql @peer.service:temperpaw-postgres
```

Two asset surfaces remain partly manual on the current Datadog account/tier:
the log facet API returned 404, and Sensitive Data Scanner rules require a
scanner group context. Their source-of-truth definitions live in
`dd-pipelines/facets.json` and `dd-pipelines/sensitive-data-scanner.json`, and
operators should register them through the Datadog UI until API support is
available.

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

For Railway, profiling uploads are on-demand. The paging monitor is therefore
`[Temper] Profiler Upload Failures`; there is no continuous "uploads stalled"
monitor because the absence of a recent manual/on-demand profile is not a
runtime fault.

The Railway deploy path pins `DD_SERVICE=temperpaw`, `DD_ENV=prod`, and
`DD_TAGS=team:temperpaw` on the runtime service. When `DD_API_KEY` is present,
it also sets `TEMPER_PROFILING_ENABLED=true` and
`TEMPER_PROFILING_AUTO_UPLOAD=true` so Temper's pprof endpoint captures profiles
and uploads them through the Datadog Agent.
If Datadog is enabled later from Railway instead of during deploy, add
`DD_API_KEY` to both the collector service and the runtime service, and add
`TEMPER_PROFILING_ENABLED=true` plus `TEMPER_PROFILING_AUTO_UPLOAD=true` to the
runtime service. Do not set `DD_PROFILING_ENABLED=true` on Railway unless native
`ddprof` is explicitly being tested; Railway currently denies the perf events
that profiler needs.

## Postgres DBM

Postgres DBM is live-verified for the production service. Datadog shows a
`temperpaw-postgres` instance, query samples tagged `service:temperpaw` and
`team:temperpaw`, and calling-service attribution back to the upstream
TemperPaw APM service/resources. Full propagation mode adds Datadog service,
environment, version, peer database service, and W3C `traceparent` SQLCommenter
fields so DBM samples can be correlated with APM traces where Datadog samples
both sides.

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
service:temperpaw type:sql @db.system:postgresql @peer.service:temperpaw-postgres
@query_signature:<signature>
```

Completion evidence must include at least one query sample or plan, not just a
monitor definition.

Live proof on 2026-05-13 included:

- `database_instance:temperpaw-postgres`
- `calling_service:temperpaw`
- a fresh full-mode query sample for `entity_catalog`
- calling resource `GET /tdata/Sessions`
- full-mode DBM sample trace metadata with `trace.mode:full`,
  `trace.caller.service:temperpaw`, `trace.caller.env:prod`,
  `trace.caller.version:sha-afeca721`, and `trace.sampled:true`
- an APM trace for the propagated `traceparent`
  `e5139e30de2db2af1cb696ab7a25d899`, containing a `GET /tdata/Sessions` root
  and a matching `entity_catalog` Postgres span
- the direct proof session trace `00795a1c90435bf41a99f0a051f9d729`, which has
  Postgres APM spans even though DBM sampling did not select that exact session

## Profiling

Profiling is expected for the Rust TemperPaw/Temper runtime. Operators should be
able to answer:

- Which functions dominate CPU during a slow session?
- Is memory growing during replay, hydration, dispatch, or WASM execution?
- Are profile uploads healthy for `service:temperpaw`?

Profiling completion requires live profile data and no stalled-upload monitor
for the active TemperPaw service.

Current live status: profiling is proven for the deployed TemperPaw service. An
on-demand CPU profile during live proof traffic returned a 40,450-byte protobuf
profile from `2026-05-13T03:24:31Z` to `2026-05-13T03:24:36Z`, the runtime
logged capture start, capture complete, and Datadog Agent intake upload events,
and
`datadog.profiling.rust.profiles_uploaded{service:temperpaw,env:prod}` returned
`version:sha-afeca721,profile_type:cpu`. Matching upload-error metrics returned
no data.

## Current Verification Status

As of the live baseline proof in `.proofs/074-temperpaw-datadog-live-baseline.md`:

- Railway deployment `20079bb1-1e83-4ed2-84dd-1689df3d2907` is serving image
  `ghcr.io/nerdsane/temperpaw:sha-afeca72`.
- APM shows production spans with `service:temperpaw`,
  `service.namespace:temperpaw`, `team:temperpaw`, and
  `service.version:sha-afeca721`.
- The live runtime image pins Temper to
  `974b13bf02342a1b8faafdb1b762572933fe1c3e`, which includes direct LLMObs
  hierarchy, Postgres trace/DBM attribution support, Datadog-compatible pprof
  upload envelopes, Datadog-visible WASM host span hints, guest-log trace/span
  correlation fields, and ADR-0084 long-lived workflow root spans.
- Live proof session `ss-019e1f59-41b4-7993-870f-9bf9ac7e4a18` completed with
  result `TemperPaw workflow root trace verified.`
- LLMObs trace `630095599782866875251990789384427305` has exactly one
  agent root, one workflow child, and one LLM child, with no broken-parent
  warnings and no errors.
- APM trace `00795a1c90435bf41a99f0a051f9d729` has root resource
  `Session.workflow`, duration 15.7 seconds, and exposes the session's
  chronological Temper action/WASM path, including action names, entity ids,
  workflow run id, module names, Postgres client spans, span events, and elapsed
  times.
- WASM guest logs for the same session are searchable by `session_id`,
  `trace_id`, and `otel.trace_id`, and include module, trigger action,
  entity, severity, and human-readable guest messages.
- Logs in the checked fresh window aggregate under `service:temperpaw`. Search
  for the prior identity terms also returned zero log events, and APM span
  search for `service:openpaw OR OpenPAW OR OpenPaw` returned zero spans.
- Postgres DBM is live for `temperpaw-postgres`, and query samples are tagged
  `service:temperpaw` and `team:temperpaw`; full-mode DBM samples carry
  `traceparent` and `trace.caller.*` metadata back to APM.
- DBM/APM correlation is visible through propagated trace
  `e5139e30de2db2af1cb696ab7a25d899` and through live SQL spans matching
  `service:temperpaw type:sql @db.system:postgresql @peer.service:temperpaw-postgres`.
- Rust profiling uploads are live for `service:temperpaw`,
  `env:prod`, `version:sha-afeca721`.
- Dashboard `mn4-k3k-i66` is updated and describes TemperPaw using
  `service:temperpaw` queries.
- Monitor reconciliation created/updated the TemperPaw monitor set and deleted
  old migration-owned named monitors. Post-reconcile monitor search shows
  `service:temperpaw` and `@slack-temperpaw-alerts`.
- `monitor_groups_search(status:alert)` returned zero active alert groups after
  correcting the DBM activity monitor, nanosecond DBM latency threshold,
  on-demand profiling monitor behavior, and idle state-timeout reset monitor.
- Log pipeline `TemperPaw / Temper Logs (ADR-0054)` was updated in Datadog with
  id `Wyq_6z_fTviM9uVH9MUIrQ`, and live log metric config contains
  `temperpaw.logs.errors`, `temperpaw.logs.warns`, and
  `temperpaw.logs.wasm.default_timeout_fallback`.
- The trace-related monitors that depend on live APM spans are trace-analytics
  alerts, not generated trace metrics: ManagedSession Start/Resume spans for
  agent-session presence and provider-caller WASM spans for LLM latency.
- Postgres DBM activity is monitored with the `datadog.dbm.activity_rows`
  metric, while DBM/APM correlation is verified with the runbook query
  `service:temperpaw type:sql @db.system:postgresql @peer.service:temperpaw-postgres`
  and DBM samples carrying `trace.caller.*` metadata. The earlier trace
  absence monitor was removed because monitor evaluation under-counted child SQL
  spans that Trace Explorer returned correctly.
- A separate authenticated raw blob streaming probe to
  `POST /tdata/Blobs/Temper.IngestRaw` returned HTTP 201 and created
  `Blobs('c251953bbf2daa464647db9ffe6b7d9a80b07c5d')`. Datadog retained APM
  trace `993e74a7129a8c286ce53d8c5b1e9f8a` with root resource
  `POST /tdata/Blobs/Temper.IngestRaw`, HTTP 201, 130 ms duration,
  `service.version:sha-afeca721`, and child spans for invariant checks and
  entity persistence.

Remaining blockers before this is final:

- Railway project, service, generated domain, private domain variables, and
  storage/database URL variables still carry the previous external identity. The
  CLI can create services/domains but does not expose a safe rename operation.
- The Datadog LLMObs agent-loop helper returned an empty timeline for the
  direct Session trace even though the LLMObs tree is correct.
- Datadog APM does not yet expose the hinted `temperpaw.agent.session` span name
  from ManagedSession WASM host HTTP headers. Direct Session roots are now live
  as `Session.workflow`; ManagedSession semantic root-name export remains a
  Temper instrumentation gap.
- Log facets and Sensitive Data Scanner rules still need UI application proof
  because the Datadog APIs available to this account did not accept those
  registrations.
