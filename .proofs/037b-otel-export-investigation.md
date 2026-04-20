# Proof Report: 037b — OTEL Export Investigation (APM Spans to Datadog)

## Date
2026-04-20

## Branch / Commit
`dd-observability` (investigation, no code change in this commit)

## What Was Done

Investigated why `/api/v2/spans/events/search?service:openpaw` returned **0 spans** over 6 h during the earlier audit, while `trace.temper.server` aggregate metrics showed low but non-zero activity for two HTTP endpoints (`get_/tdata/curationjobs`, `get_/tdata/curationqueries_en-_guid`).

Read:
- `scripts/otel-collector-datadog.yaml` — collector pipeline config.
- `/Users/seshendranalla/Development/temper/crates/temper-observe/src/otel.rs:235-443` — `init_observability` / `init_tracing` resource attribute setup.

## Findings

### 1. The collector config is structurally sound

The OTEL Collector in `scripts/otel-collector-datadog.yaml` has two distinct trace pipelines:

- **`traces/llmobs`** — keeps only spans with `gen_ai.system` set → routes to `clickhouse` + `otlphttp/llmobs` (DD LLM Observability product).
- **`traces/apm`** — keeps only spans WITHOUT `gen_ai.system` → routes to `clickhouse` + `datadog` (DD APM).

This is the intended split per ADR-0035: LLM spans go to LLMObs, everything else to APM. Both filters and exporters are wired correctly.

### 2. Temper sets `service.name` resource attribute

`temper-observe::init_tracing` (line 330) builds the resource with `service.name = <service_name>` passed from `crates/temperpaw/src/main.rs:98` (value: `"openpaw"`). Also adds `deployment.environment.name` and `service.version` when env vars are set.

What it does **NOT** set explicitly:
- `host.name` — DD APM uses this for host tagging; OTEL Collector's `datadog` exporter tries to auto-resolve via systeminfo, which may return container IDs like `64ff970159e7` (matches `host:64ff970159e7` observed in our logs).
- `host.id`, `container.id`, `k8s.*` — no OTEL resource detector is running.

### 3. API-key scope limits verification

`/api/v2/spans/events/search` returns `{"errors": ["Unauthorized"]}` with our `DD_APP_KEY`. The key has Logs read + Metrics read + Pipeline admin, but not APM trace read. I cannot definitively verify span ingestion from the API alone. Metric-level `trace.temper.server.hits` aggregates exist for **two** resource names over 24h, which is consistent with either (a) only two endpoints being instrumented, or (b) only two endpoints' spans reaching DD APM due to some filter/tag issue.

## Hypotheses Ranked

1. **Most likely: only two routes are instrumented.** The axum handlers in `temper-server` don't uniformly emit request spans. Only `GET /tdata/CurationJobs…` and `GET /tdata/curationqueries…` carry a `#[instrument]` / span-generating handler today. Every other endpoint (`POST /tdata/.../action`, etc.) has no span. Resolution: Fix C1 adds root spans to the action-dispatch path uniformly.
2. **Second: `host.name` not set.** If the collector's `datadog` exporter can't resolve hostname, it may drop traces. Mitigation below (resourcedetection/resource processors).
3. **Third: DD tier or trace intake throttling.** Unverifiable without DD-side logs.

## Mitigation (not deployed in this commit)

Proposed addition to `scripts/otel-collector-datadog.yaml` — runs in a later commit once the user confirms via DD UI whether spans are flowing:

```yaml
processors:
  resourcedetection:
    detectors: [env, system, docker]
    timeout: 5s
    override: false
  resource:
    attributes:
      - key: service.name
        value: openpaw
        action: upsert

# ...then add resourcedetection, resource to every trace pipeline:
service:
  pipelines:
    traces/apm:
      processors: [resourcedetection, resource, filter/traces_apm, batch]
    traces/llmobs:
      processors: [resourcedetection, resource, filter/traces_llmobs, batch]
```

This is a safe additive change: if DD already had everything it needs, these processors are no-ops.

## Why We're Not Blocking C1-C4 On This

The real test of the OTEL → DD APM path is whether the NEW spans introduced by C1-C4 show up in the DD APM UI. If Fix C lands and those spans appear, the pipeline works and we can close this out. If they don't, we run the mitigation above.

Either way, the investigation outcome is the same: **proceed with C1-C4, verify in DD APM UI after deploy, apply the resourcedetection/resource mitigation if spans don't appear**.

## Verification Results

| Step | Expected | Actual | Status |
|------|----------|--------|--------|
| Collector YAML structure | Two distinct trace pipelines routing to appropriate DD products | Confirmed | PASS |
| `temper-observe` sets `service.name` | `service.name=openpaw` in resource | Confirmed at `otel.rs:330` | PASS |
| `temper-observe` sets `host.name` | Any value | NOT set explicitly; relies on collector auto-detect | NOTE |
| API span search for service:openpaw | Returns spans | `{"errors": ["Unauthorized"]}` — key scope issue | BLOCKED |
| Trace metric aggregate `trace.temper.server.hits` | Matches span count | 2 resources only (curationjobs, curationqueries) | CONSISTENT WITH INCOMPLETE INSTRUMENTATION |

## Artifacts
- Collector config: `scripts/otel-collector-datadog.yaml`
- Temper OTEL init: `/Users/seshendranalla/Development/temper/crates/temper-observe/src/otel.rs:235-443`
- Mitigation patch (staged, not applied): see block above.

## Next Step
Proceed to Fix C1 (root span per action dispatch, in Temper repo worktree). After the rebuild + redeploy, query `trace.temper.*` aggregates; a count > 2 distinct resource names confirms wider instrumentation is flowing. If aggregate count is still ≤ 2, apply the resourcedetection/resource mitigation above.
