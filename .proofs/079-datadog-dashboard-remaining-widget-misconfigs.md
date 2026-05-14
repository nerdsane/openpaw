# Datadog Dashboard Remaining Widget Misconfiguration Fix

Date: 2026-05-14

## Scope

Fixed the remaining user-visible misconfigured widgets on the TemperPaw Platform Overview Datadog dashboard (`mn4-k3k-i66`):

- Replaced dashboard percentile queries (`p50:`, `p95:`, `p99:`) on Temper runtime metrics with supported `avg:` / `max:` queries because Datadog percentiles are not enabled for those metrics.
- Replaced note-only dashboard groups with real Datadog `list_stream` widgets for:
  - Channel Transports
  - Webhook Triggers
  - Governance Approvals
- Removed the reserved Handler Liveness group until the W3 telemetry contract emits real metrics. It should not render as a comment-only section.

## Red

Added regression coverage in `crates/temperpaw/tests/datadog_monitor_config.rs`.

Initial focused test run failed as expected:

```text
cargo test -p temperpaw --test datadog_monitor_config
platform_dashboard_avoids_unsupported_percentile_queries ... FAILED
platform_dashboard_groups_are_not_comment_only_sections ... FAILED
log_oriented_dashboard_sections_have_list_widgets ... FAILED
platform_dashboard_widgets_do_not_blank_on_known_datadog_query_drift ... FAILED
```

The failing assertions identified:

- Unsupported percentile query prefix: `p95:temper_session_context_tokens{service:temperpaw}`
- Note-only groups: `Channel Transports`, `Webhook Triggers`, `Handler Liveness (W3 / temper#147 — reserved)`, `Governance Approvals`
- Missing logs list widgets for the log-oriented sections

## Green

Patched `dd-dashboards/temperpaw-overview.json`:

- `Session Context Tokens p95 / p99` -> `Session Context Tokens avg / max`
- `Ask Attempts Distribution` -> `Ask Attempts avg / max`
- `Dispatch Attempts Percentiles` -> `Dispatch Attempts by Entity avg / max`
- `Time-in-State p95` -> `Time-in-State avg`
- `Wait Time p95` -> `Wait Time avg`
- `Actor Reply Latency p95` -> `Actor Reply Latency avg`
- Dispatch contention p50/p95/p99 widgets -> avg/max widgets
- Channel/Webhook/Governance note-only groups -> `list_stream` widgets backed by logs queries
- Handler Liveness reserved note-only group removed

Final focused test run:

```text
cargo test -p temperpaw --test datadog_monitor_config --test datadog_observability_contract
datadog_monitor_config: 7 passed
datadog_observability_contract: 23 passed
```

Static checks:

```text
jq empty dd-dashboards/temperpaw-overview.json
git diff --check
rg -n "p50:|p75:|p90:|p95:|p99:|Percentiles Misconfiguration|Handler liveness metrics are reserved|Handler Liveness" dd-dashboards/temperpaw-overview.json
```

`jq` and `git diff --check` passed. The `rg` check returned no matches.

## Live Deployment

Deployed the dashboard source to Datadog:

```text
python3 scripts/deploy_dashboard.py dd-dashboards/temperpaw-overview.json
Updated temperpaw-overview.json: https://app.datadoghq.com/dashboard/mn4-k3k-i66
```

The first deployment attempt caught one Datadog API schema issue (`show_legend` is not allowed on `list_stream` widgets). Removed that property and redeployed successfully.

Live dashboard verification through the Datadog API:

```text
unsupported_percentile_markers= []
note_only_groups= []
missing_log_query_markers= []
```

Also refetched `mn4-k3k-i66` through Datadog MCP and confirmed the live dashboard contains:

- `Recent Transport Logs` as a `list_stream` widget with `service:temperpaw @observability_event:temperpaw.transport`
- `Recent Webhook Trigger Logs` as a `list_stream` widget with `service:temperpaw @observability_event:temperpaw.webhook`
- `Recent Approval Logs` as a `list_stream` widget with `service:temperpaw @observability_event:temperpaw.approval`
- avg/max replacements for the former percentile widgets
- no Handler Liveness group

## ADR Judgement

No ADR was added. This is dashboard source maintenance for existing Datadog observability surfaces, not a new architecture decision, telemetry contract, trigger, policy, or entity state-machine change.
