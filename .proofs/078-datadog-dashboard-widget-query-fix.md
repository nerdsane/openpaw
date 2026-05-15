# 078 - Datadog Dashboard Widget Query Fix

Date: 2026-05-13

## Scope

Fixed misconfigured widgets in `dd-dashboards/temperpaw-overview.json` for the live dashboard `mn4-k3k-i66` / "TemperPaw - Platform Overview".

This is dashboard source maintenance, not an architecture change. No ADR was added because no entity spec, policy, integration, trigger, storage model, deployment behavior, or agent capability contract changed.

## Red

Added `platform_dashboard_widgets_do_not_blank_on_known_datadog_query_drift` in `crates/temperpaw/tests/datadog_monitor_config.rs`.

Initial failures:

- Failed on stale `trace.tool.llm_call` LLM widgets.
- After the first fix, failed on reserved handler-liveness metrics that Datadog does not emit.

## Green

Updated dashboard widgets:

- Replaced stale `trace.tool.llm_call.*` queries with live LLM Observability metrics:
  - `ml_obs.trace`
  - `ml_obs.span.error`
- Added `default_zero(...rollup(sum, 60))` to trace-agent and LLM count widgets so quiet windows render as zeros instead of no-data.
- Removed `service:temperpaw` filters from Monty REPL and large-content externalization metrics after Datadog showed those historical metrics have no `service` tag.
- Replaced reserved handler-liveness placeholder charts with an explicit note until the W3 telemetry contract is emitted.

## Verification

Local checks:

```text
jq empty dd-dashboards/temperpaw-overview.json
cargo test -p temperpaw --test datadog_monitor_config
```

Result:

```text
4 passed; 0 failed
```

Datadog live validation:

- Queried the exact dashboard URL window: `2026-05-13T21:35:50Z` to `2026-05-13T21:40:50Z`.
- Corrected trace-agent, LLM, Monty, and session externalization widgets returned zero-filled series instead of no-data.
- Datadog metric discovery confirmed no live handler-liveness metrics for the removed reserved widgets.

Deploy:

```text
python3 scripts/deploy_dashboard.py dd-dashboards/temperpaw-overview.json
```

Result:

```text
Updated temperpaw-overview.json: https://app.datadoghq.com/dashboard/mn4-k3k-i66
```

Post-deploy verification:

- Refetched `mn4-k3k-i66` through Datadog and confirmed the live dashboard contains the corrected `default_zero(...)`, `ml_obs.*`, unfiltered Monty/session, and handler-liveness note definitions.

The dashboard source is fixed, deployed to Datadog, and verified locally and through live Datadog reads.
