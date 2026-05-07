# Proof Report: 066 - Datadog Dashboard Runtime Telemetry Rework

## Date
2026-05-07

## Branch / Commit
- Branch: `codex/paw-rework-c3b38100`
- Head at proof time: `661c5d4` plus local rework edits
- WorkCycle: `wc-019e0467-7748-7941-ab0f-a4e007d38814`
- FactoryCase: `en-019e0467-76d5-7152-8e10-093976c4f668`

## What Was Done
Addressed the reviewer blocker directly: the checked-in Datadog dashboard was corrected and deployed to the live Platform Overview dashboard `mn4-k3k-i66`.

Changes made:

1. Added a dashboard contract test to prevent reintroducing retired/no-data queries.
2. Replaced `trace.custom` resource and dispatch-latency widgets with live Temper runtime metrics.
3. Removed `trace.wasm.invoke` migration overlays that Datadog rejected during deployment.
4. Replaced unsupported `p99:temper_dispatch_ask_latency_ms` dashboard queries with the live `avg:temper_dispatch_ask_latency_ms` aggregation.
5. Deployed `dd-dashboards/temperpaw-overview.json` to Datadog dashboard `mn4-k3k-i66`.

No ADR was added. This rework is a dashboard query/deployment correction, not a material change to Temper app specs, WASM integrations, Cedar policy, storage, triggers, or agent capability surfaces.

## Red-Green TDD
### Red
- Added `platform_dashboard_uses_live_runtime_metrics_instead_of_stale_trace_custom_queries` in `crates/temperpaw/tests/datadog_monitor_config.rs`.
- Initial targeted run failed because the local dashboard still contained `trace.custom`.
- Extended the same test and confirmed a second failure while `trace.wasm.invoke` overlays remained.
- Extended the same test again and confirmed failure while unsupported `p99:temper_dispatch_ask_latency_ms` remained.

### Green
- Updated `dd-dashboards/temperpaw-overview.json`.
- Re-ran `CARGO_TARGET_DIR=/tmp/temperpaw-review-target cargo test -p temperpaw --test datadog_monitor_config`.
- Result: `3 passed; 0 failed`.

## Live Datadog Evidence
Dashboard deployment:

```text
python3 scripts/deploy_dashboard.py dd-dashboards/temperpaw-overview.json
Updated temperpaw-overview.json: https://app.datadoghq.com/dashboard/mn4-k3k-i66
```

Live dashboard exact checks after deploy:

```text
trace.custom_absent=True
trace.wasm.invoke_absent=True
temper_active_entities_absent=True
p99_dispatch_latency_absent=True
cedar_replacement_present=True
avg_dispatch_latency_present=True
wasm_duration_present=True
```

Datadog MCP read-back confirmed dashboard `mn4-k3k-i66` now contains:

- `top(sum:temper_cedar_evaluations_total{service:openpaw} by {decision}.as_count(), 10, 'sum', 'desc')`
- `avg:temper_active_actors{service:openpaw}`
- `avg:temper_dispatch_ask_latency_ms{service:openpaw} by {entity_type,action}`
- `avg:temper_wasm_invocation_duration_ms{service:openpaw} by {trigger_action}.rollup(avg, 60)`

Replacement metric live checks over `now-2h`:

| Query | Live Result |
| --- | --- |
| `sum:temper_cedar_evaluations_total{service:openpaw}.as_count()` | 338,556 total events |
| `avg:temper_active_actors{service:openpaw}` | avg 114.18, max 987.5 |
| `avg:temper_dispatch_ask_latency_ms{service:openpaw}` | avg 21.29 ms, max 274.18 ms |
| `avg:temper_wasm_invocation_duration_ms{service:openpaw}` | avg 1244.46 ms, max 6366.68 ms |

Residual profiler evidence:

- Monitor `275383901` (`[Temper] Profiler Uploads Stalled`) is still `Alert`.
- Datadog metric context lookup for `datadog.profiling.rust.profiles_uploaded` still returns metric not found.
- This proof does not claim profiler telemetry is live; it proves the reviewer deployment blocker is resolved and records profiler telemetry as residual risk.

## OData Evidence
Production OData links:

- WorkCycle: `https://openpaw-production.up.railway.app/tdata/WorkCycles('wc-019e0467-7748-7941-ab0f-a4e007d38814')`
- FactoryCase: `https://openpaw-production.up.railway.app/tdata/FactoryCases('en-019e0467-76d5-7152-8e10-093976c4f668')`

Observed before local Codex exit:

| Entity | Status | Notes |
| --- | --- | --- |
| WorkCycle `wc-019e0467-7748-7941-ab0f-a4e007d38814` | `InProgress` | Contains reviewer `request_changes` text; worker self-report will occur after Codex exits. |
| FactoryCase `en-019e0467-76d5-7152-8e10-093976c4f668` | `Scoped` | Waiting on this rework WorkCycle to report completion. |

## Verification Results
| Step | Expected | Actual | Status |
| --- | --- | --- | --- |
| Datadog dashboard deploy | Updated dashboard `mn4-k3k-i66` | Deploy script updated live dashboard | PASS |
| Live dashboard exact query check | Retired queries absent, replacements present | All exact checks returned `True` | PASS |
| `git diff --check` | No whitespace errors | No output | PASS |
| `jq empty dd-dashboards/temperpaw-overview.json dd-monitors/temperpaw-monitors.json` | JSON valid | No output | PASS |
| `bash -n scripts/temperpaw-entrypoint.sh` | Shell syntax valid | No output | PASS |
| `CARGO_TARGET_DIR=/tmp/temperpaw-review-target cargo test -p temperpaw --test datadog_monitor_config` | Config tests pass | 3 passed | PASS |
| `cargo build -p temperpaw` | Build completes | Finished dev profile in 1m 26s | PASS |

## Reviewer / Evaluator Status
| Gate | Verdict |
| --- | --- |
| Previous reviewer | `request_changes`: local work was good, live dashboard was stale |
| This rework | Reviewer blocker addressed: dashboard is now deployed and verified live |
| Independent evaluator | Pending after worker report; local proof evidence is ready |

## Residual Risks
- Profiler upload telemetry is still not live in Datadog. Monitor `275383901` remains `Alert` until production profiler deployment/config is corrected or the profiler monitor is retired in a separate change.
- Some dashboard widgets still represent sparse/event-driven metrics. Empty periods are expected when no events occur, but the stale trace-metric overlays and invalid dispatch percentile query were removed.
- WorkCycle and FactoryCase state will not advance until the paw-codex-worker reports this local Codex result after process exit.

## Artifacts
- Live dashboard: `https://app.datadoghq.com/dashboard/mn4-k3k-i66`
- Source dashboard: `dd-dashboards/temperpaw-overview.json`
- Datadog monitor: `https://app.datadoghq.com/monitors/275383901`
- Contract test: `crates/temperpaw/tests/datadog_monitor_config.rs`

## State Diagram
```text
Reviewer RequestChanges
        |
        v
WorkCycle InProgress
        |
        v
Red test pins retired queries
        |
        v
Dashboard JSON corrected
        |
        v
Datadog deploy updates mn4-k3k-i66
        |
        v
Live dashboard read-back and metric checks pass
        |
        v
WorkerRun result ready for ReportDone after Codex exits
```

## Telemetry Flow
```text
OpenPaw / Temper runtime
        |
        v
Datadog metrics:
  temper_cedar_evaluations_total
  temper_active_actors
  temper_dispatch_ask_latency_ms
  temper_wasm_invocation_duration_ms
        |
        v
dd-dashboards/temperpaw-overview.json
        |
        v
scripts/deploy_dashboard.py
        |
        v
Live Datadog dashboard mn4-k3k-i66
```
