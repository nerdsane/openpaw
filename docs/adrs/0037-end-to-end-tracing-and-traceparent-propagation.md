# ADR-0037: End-to-End Tracing and W3C Traceparent Propagation

- Status: Proposed
- Date: 2026-04-20
- Deciders: OpenPaw maintainers
- Related:
  - ADR-0005 (temper-native-orchestration): Temper-primitives-only mandate — the tracer must reuse Temper's OTEL setup, not bolt on a parallel stack.
  - ADR-0029 (deployment-architecture): `scripts/otel-collector-datadog.yaml` is the current OTEL→Datadog exporter pipeline this ADR fixes.
  - ADR-0035 (ots-trajectory-emission): defines `gen_ai_parent_trace_id` / `gen_ai_parent_span_id` — this ADR wires those fields to real OpenTelemetry span IDs.
  - ADR-0054 (agent-log-schema-datadog-correlation): log schema already carries `trace_id` / `span_id`; this ADR finally gives those IDs real spans to correlate against.
  - temper ADR (to be filed alongside this one): `temper-wasm-sdk` `traceparent` propagation API.
  - `crates/temperpaw/src/main.rs:98` (existing `temper_observe::otel::init_observability()` call — do not duplicate).
  - `os-apps/paw-agent/wasm/llm_caller/src/lib.rs` (primary consumer of `gen_ai.*` attributes).
  - `os-apps/paw-agent/wasm/monty_repl/src/dispatch.rs:117-131` (tool-dispatch loop — primary site for per-tool spans).
  - `dd-pipelines/temper-temperpaw.json` (DD log pipeline — `entity_status` rename lives here).

## Context

The 2026-04-20 Katagami investigation exposed three defects in the current observability story:

1. **Sessions fail silently.** Two research jobs (`ss-019dab1b…` for "2026 UI UX trends", `ss-019dab1c…` for "Chinese calligraphy") have failed 5 times in a row. Between session creation and Fail transition there is a 13–19 minute window with **zero logs**. The `temper.chat.duration_ms` metric is not emitted for the window, the `call_llm` integration has `timeout_secs=600` (10 min) with no intermediate timing signals, and no state-timeout fires. Operators cannot tell whether the LLM hung, the WASM module crashed, or the dispatch queue starved.
2. **No end-to-end trace exists for a job.** APM span search (`service:openpaw`, 6 h window) returns 0 spans, even though `trace.temper.server` aggregate metrics exist for two HTTP endpoints (`get_/tdata/curationjobs`, `get_/tdata/curationqueries_en-_guid`). The OTEL collector is clearly emitting aggregate rollups but not the underlying span payloads reachable from DD APM. Per-action `duration_ms` metric families (`temper.ProcessToolCalls.duration_ms`, `temper.CreateFile.duration_ms`, etc. — 19 families total) exist but carry no `trace_id` and cannot be joined into a flame graph. The user cannot answer "show me this job's full breakdown" with today's instrumentation.
3. **DD log severity is polluted.** `temper-observe` emits a structured-log field named `status` holding the entity state-machine state (`CallingProvider`, `Executing`, `PreparingContext`). DD's log ingestion interprets that as log severity and promotes these `INFO`-level logs to `critical`/`emergency`/`alert` — **197 false-alarm logs per 22 min** observed live. Monitors fire on noise; real criticals are hidden.

The slowness itself is visible but not decomposable: `temper.write` tool calls take 26–37 s each; underlying `workspace_fs.CreateFile` is ~10 s alone. We cannot attribute those 10 s to Turso writes vs. projection replay vs. blob transport without spans.

## Decision

Adopt **OpenTelemetry W3C Trace Context** as the single end-to-end propagation protocol, and commit to the `traceparent` header as the one cross-boundary carrier. Every hop in the job's execution path creates or continues a span; the Datadog OTLP exporter reassembles the tree.

### 1. W3C `traceparent` is the single propagation header

Every HTTP request entering Temper extracts `traceparent`/`tracestate` and either continues the trace or starts a new root. Every outgoing HTTP call from anywhere in the system — host code, WASM modules, `ctx.http_call()` from inside WASM — injects the current span's `traceparent`. No custom trace-id-as-entity-attribute schemes, no per-component header flavors.

### 2. Root span per action dispatch

In `temper-server`, each action dispatch starts a root span named `temper.action` with tags:

- `entity_id`, `entity_type`, `action`, `tenant`
- `session_id` (when the dispatch is on a Session entity or a child spawned from one)
- `trigger_action` (matches today's metric tag for continuity)

This replaces the 19 per-action `duration_ms` metric families — span duration carries the same information with richer tags and joinability.

### 3. Host ↔ WASM boundary transparency

The WASM invocation API widens: the host passes the current span's `traceparent` string into the WASM module at invocation entry. The WASM module starts a `temper.wasm.invoke` child span, then the `temper-wasm-sdk` `ctx.http_call()` automatically injects that `traceparent` into outgoing request headers. Host code receiving those requests continues the same trace. The boundary becomes invisible in the trace tree.

### 4. Per-tool spans in agent loops

`monty_repl::dispatch` wraps each tool call in a `temper.tool` span with attributes `tool_name`, `tool_call_id`, `duration_ms`, `success`. `llm_caller` wraps the LLM HTTP call in a `temper.llm.chat` span with OpenTelemetry `gen_ai.*` semconv attributes (`gen_ai.request.model`, `gen_ai.system`, `gen_ai.usage.input_tokens`, `gen_ai.usage.output_tokens`, `gen_ai.usage.cache_read_input_tokens`), and populates the `gen_ai_parent_trace_id` / `gen_ai_parent_span_id` fields defined by ADR-0035 with the real OTEL span IDs.

### 5. DD log pipeline: rename `status` attribute

In `dd-pipelines/temper-temperpaw.json`, add an attribute-remapper that renames incoming `status` → `entity_status` before the status-remapper runs, and point the status-remapper at `otel.severity_text` (with `level` as fallback). Log correlation stays intact: `@trace_id` / `@span_id` are already mapped to `dd.trace_id` / `dd.span_id` in the existing pipeline (lines 31–51).

## Deprecated Metrics

Once spans are flowing end-to-end and dashboards have migrated to APM trace metrics, the following are removed. They become redundant and cost cardinality without adding information the trace does not already carry:

| Deprecated | Replaced by |
|---|---|
| `temper.{ACTION}.duration_ms` (19 families) | `temper.action` span duration, filterable by `entity_type`, `action`, `tenant` |
| `temper.{ACTION}.invocation_count` | `temper.action` span count |
| `temper.{ACTION}.decision_count`, `transition_count`, `item_count` | span tags on `temper.action` |
| `temper_wasm_invocation_duration_ms` | `temper.wasm.invoke` span duration |
| `"tool dispatch complete tool_name=… duration_ms=…"` INFO log | `temper.tool` span attributes (log is downgraded to `debug`) |
| log-based metric aggregates over `@duration_ms` in `dd-log-metrics/temper-log-metrics.json` | APM trace metrics |

Retained: metrics that are per-state/per-host gauges, not per-request latency, and whose aggregation shape is cheaper as a metric than as a span rollup — `temper_active_actors`, `temper_indexed_entities`, `temper_projection_backfill_snapshot_misses_total`, `temper_admission_*`, `temper_cedar_evaluations_total`, `temper_actor_mailbox_utilization`, `temper_state_timeout_fired_total`.

## Readiness Gates

- Retry both failing katagami sessions (`ss-019dab1b…`, `ss-019dab1c…`) with the new instrumentation and confirm: (a) the session's Failed transition is attributable to a specific span with a non-zero error/duration; (b) DD APM shows a single connected trace tree from trigger → session → agent turn → tool calls → LLM call → workspace_fs CreateFile; (c) total trace duration matches observed wall-clock within 5 %.
- DD log query `service:openpaw status:critical "entity actor stopped"` over 15 min post-deploy returns **0 matches** (was ~9/min).
- `temper.llm.chat` span count over 1 h equals the `call_llm` integration invocation count ± 5 % (parity with existing metric).
- `temper.tool` span count per session matches the agent's tool-call count recorded in OTS trajectory (ADR-0035 parity).
- Dashboards that queried deprecated metrics are migrated and still render.

## Consequences

### Positive
- Every job becomes a single clickable trace. "Why is this slow?" becomes a flame graph, not a forensic log hunt.
- Log-trace correlation already works (ADR-0054 schema); this ADR finally gives it traces to correlate to.
- 19+ metric families removed; dashboard surface shrinks; cardinality pressure reduced.
- OpenTelemetry `gen_ai.*` semconv gives LLM observability parity with third-party APM tools (Langfuse-equivalent, but native).
- `traceparent` is vendor-neutral — if Datadog is ever swapped for another APM backend, no re-instrumentation is needed.

### Negative
- Two-repo change: OpenPaw + the Temper platform clone. C1/C2 land in `nerdsane/temper` and require a coordinated merge.
- `temper-wasm-sdk` contract widens (new `traceparent` argument at WASM entry, auto-injection in `ctx.http_call`). WASM modules built against older SDK versions continue to work but emit disconnected child traces until rebuilt.
- Dashboard query migration has a window of overlap (old metrics still emitted, new spans also emitted) — costs double-ingest for ~1 week during rollout.
- Sampling strategy becomes load-bearing: at current 12 k logs / 22 min rate, full-fidelity span capture may require head-based sampling on the OTEL collector.

### Risks
- **OTEL export pipeline is already broken** (0 spans in APM explorer despite aggregates existing). The fix to `scripts/otel-collector-datadog.yaml` is a prerequisite. If the broken pipeline ships to prod alongside the new spans, the new spans are also lost — verification must happen in staging first.
- **Cardinality explosion on `entity_id`.** Each entity has a unique `entity_id`; using it as a span tag creates unbounded cardinality at DD. Mitigation: `entity_id` is a span **attribute**, not a tag; DD APM attribute search works on attributes without counting toward tag cardinality.
- **`traceparent` length in WASM invocation ABI.** The header is 55 bytes; passing it via the existing WASM function signature is feasible but requires a version bump on the SDK contract. Covered in the companion temper ADR.

## Non-Goals

- Rewriting the metrics pipeline. Remaining metrics stay; only the listed families are deprecated.
- Introducing a third-party APM side-car (Datadog APM agent, Honeycomb, etc.). Temper is already OTEL-native — we fix what exists, not parallel-stack it.
- Retrofitting spans onto historical data. Traces start with rollout; past jobs remain log-only.
- Replacing ADR-0035's OTS trajectory emission. Trajectories stay as durable forensic artifacts; traces are the live-debugging surface.

## Alternatives Considered

1. **Custom `trace_id` as entity attribute, no OTEL spans** — Rejected. Would work for log correlation but gives no flame graph in DD APM, leaves the "which sub-step inside `CreateFile` is slow" question unanswered, and creates a non-standard propagation scheme that future tooling cannot consume.
2. **Per-action metrics only, with richer tags (`entity_id`, `session_id`)** — Rejected. Tags on Datadog metrics are tag-cardinality-bounded; adding `entity_id` (unbounded) would blow the tag budget. Also leaves the nested flame-graph view unsolved.
3. **Datadog APM agent side-car instead of OTEL exporter** — Rejected. Violates ADR-0005 (Temper-native). Duplicates the existing `temper-observe` OTEL setup. Requires a second propagation library in WASM modules.
4. **Ship tracing in openpaw only; skip Temper repo changes** — Rejected. Without C1 (root span) and C2 (`traceparent` injection in `temper-wasm-sdk`), the trace tree fragments at every host↔WASM boundary. Every tool call becomes its own disconnected trace. Partial instrumentation gives worse UX than clearly-scoped-but-broken observability.

## Rollback Policy

- Revert commits in reverse order: metric-deprecation → C4 → C3 → C2 → C1 → OTEL pipeline → Fix A (DD pipeline).
- Deprecated metrics are re-emittable from a feature-flag in `temper-observe` (off-by-default after this ADR lands; can be flipped back on without code changes if dashboards regress).
- DD pipeline attribute-remapper can be removed in the DD UI or by re-running `scripts/deploy_pipelines.py` against a reverted JSON.
- No persistent-state migration required. Existing OTS trajectories remain valid.
