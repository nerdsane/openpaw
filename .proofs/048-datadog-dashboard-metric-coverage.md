# Proof Report: 048 — Datadog Dashboard Metric Coverage

## Date
2026-04-15

## Branch / Commit
- **openpaw**: `main` (`2e2c0abe`, worktree dirty)

## What Was Done
Updated the OpenPaw Datadog overview dashboard so it reflects the metrics that are currently being emitted by OpenPaw and Temper, and removed or replaced stale widget queries that were producing empty charts.

The dashboard work included:

1. Auditing the checked-in dashboard definition against the live dashboard and against the metrics Datadog is actively indexing for `service:openpaw`.
2. Replacing stale or non-existent dashboard queries such as:
   - `temper_startup_phase_duration_ms`
   - `temper_startup_time_to_healthy_ms`
   - `temper_wasm_module_load_failures_total`
   - `temper_wasm_module_skipped_total`
3. Adding coverage for live runtime metrics that were missing from the dashboard, including platform health, runtime state, persistence and recovery, Cedar authorization, WASM execution, WASM host HTTP, blob transport, and Monty REPL activity.
4. Wrapping sparse count-style queries in `default_zero(...)` where appropriate so widgets stay populated instead of rendering blank during quiet periods.
5. Deploying the updated dashboard to Datadog so the live dashboard `mn4-k3k-i66` matches the repo definition.

## Red-Green TDD
### Red
- Updated the dashboard contract test in `crates/openpaw/src/startup.rs` to assert the presence of the live metrics we want represented on the dashboard and the absence of the stale metric names that were causing empty widgets.
- Ran:
  - `cargo test -p openpaw datadog_configs_use_tenant_aware_entity_queries -- --nocapture`
- The test failed before the dashboard JSON was updated because the dashboard definition did not yet include the new coverage.

### Green
- Updated `dd-dashboards/openpaw-overview.json` with the new live metric queries and default-zero handling.
- Re-ran the targeted test successfully.

## Files Changed
- `dd-dashboards/openpaw-overview.json`
- `crates/openpaw/src/startup.rs`

## Verification Flow
1. Inspect the checked-in dashboard source and deployment script.
2. Inspect the live Datadog dashboard definition.
3. Query Datadog metric inventory and live series to identify what OpenPaw/Temper metrics are actually present.
4. Update the dashboard contract test to encode the expected coverage.
5. Run the targeted test and confirm it fails before the dashboard update.
6. Update the dashboard JSON to include live metrics and replace stale/blank queries.
7. Re-run the targeted test and confirm it passes.
8. Deploy the dashboard to Datadog.
9. Re-fetch the live dashboard and confirm the deployed definition matches the updated dashboard.

## Verification Results
| Step | Expected | Actual | Status |
|------|----------|--------|--------|
| Datadog metric audit | Identify live OpenPaw/Temper metrics and stale dashboard queries | Live metrics found for `temper_up`, `temper_active_entities`, `temper_active_actors`, `temper_indexed_entities`, `temper_projected_entities`, `temper_projection_coverage_ratio`, `temper_cedar_evaluations_total`, `temper_wasm_invocations_total`, `temper_wasm_host_http_requests_total`, `temper_turso_query_duration`, `temper_event_replay_duration`, blob metrics, and Monty REPL metrics; stale dashboard metrics confirmed absent | PASS |
| `cargo test -p openpaw datadog_configs_use_tenant_aware_entity_queries -- --nocapture` before dashboard update | Contract test should fail while dashboard coverage is incomplete | Failed before the JSON update | PASS |
| `cargo test -p openpaw datadog_configs_use_tenant_aware_entity_queries -- --nocapture` after dashboard update | Contract test should pass with updated coverage | Passed | PASS |
| `python3 scripts/deploy_dashboard.py` | Live Datadog dashboard should update successfully | Updated dashboard `https://app.datadoghq.com/dashboard/mn4-k3k-i66` | PASS |
| Live dashboard fetch after deploy | Live dashboard should reflect updated widget/query set | Live dashboard `mn4-k3k-i66` matched the deployed definition | PASS |

## What Worked
- The repo already had a single source of truth for the dashboard in `dd-dashboards/openpaw-overview.json`, so the fix was easy to encode and re-deploy.
- The existing startup contract test was a good place to lock in dashboard coverage and prevent regressions.
- Datadog metric inspection made it straightforward to separate real metric names from stale historical ones.
- `default_zero(...)` fixed the widgets that were blank because they were showing sparse count metrics.

## What Didn't Work
- Some existing dashboard widgets were pointing at metric names that are no longer being emitted, which made them permanently empty.
- A few runtime metrics are event-driven or sparse, so using raw queries without `default_zero(...)` made the dashboard look broken even when collection was healthy.

## Limitations
- This verification focused on the dashboard definition and the live Datadog dashboard deploy; it did not retune Datadog monitors.
- Event-driven widgets can still appear quiet when no events are occurring. The fix here ensures they render valid zero-valued series instead of blank panels where appropriate.
- The worktree contains unrelated pre-existing changes outside the dashboard files and test touched for this task.

## What Still Doesn't Work
- `dd-monitors/openpaw-monitors.json` still contains some stale metric references that were not part of this dashboard-only change.
- If additional OpenPaw/Temper metrics are introduced later, they will still need to be added to the dashboard intentionally; this change covers the metrics currently observed in Datadog during the audit.

## Artifacts
- Live dashboard URL: `https://app.datadoghq.com/dashboard/mn4-k3k-i66`
- Updated dashboard source: `dd-dashboards/openpaw-overview.json`
- Coverage contract test: `crates/openpaw/src/startup.rs`
- Deploy command used: `python3 scripts/deploy_dashboard.py`
- Targeted verification command: `cargo test -p openpaw datadog_configs_use_tenant_aware_entity_queries -- --nocapture`

## Architecture Diagram
```text
OpenPaw / Temper runtime
        |
        v
 Datadog metrics ingestion
        |
        v
 checked-in dashboard JSON
 (dd-dashboards/openpaw-overview.json)
        |
        v
 deploy script
 (scripts/deploy_dashboard.py)
        |
        v
 live Datadog dashboard
 (mn4-k3k-i66)
```
