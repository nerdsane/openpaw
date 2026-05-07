# ADR-002: Datadog No-Data Monitor Semantics

Date: 2026-05-07

## Status

Accepted

## Context

OpenPaw was healthy enough to emit APM spans and runtime metrics, but several
Datadog monitors reported `No Data`. The highest-impact gap was the OpenPaw
traffic/error/latency trio, which still queried `trace.custom*` metrics that
are not emitted for `service:openpaw`. Several sparse failure counters also
reported `No Data` when the healthy value was simply zero.

## Decision

OpenPaw traffic, error-rate, and latency monitors use currently emitted
OpenPaw/Temper metrics instead of generated `trace.custom*` metric names:

- traffic: `temper_cedar_evaluations_total`
- error numerator: `openpaw.logs.errors`
- latency: `temper_dispatch_ask_latency_ms`

Sparse failure-counter monitors use `default_zero(sum:<counter>.as_count())`
and keep `notify_no_data=false`. Missing datapoints for these counters mean no
failure happened in that window; if the counter appears, thresholds still fire
normally.

Gauge-style liveness monitors can still notify on missing data when missing
data is itself a signal that the telemetry pipeline or runtime heartbeat is
broken.

## Consequences

- `[OpenPaw] No Traffic`, `[OpenPaw] Error Rate Spike`, and
  `[OpenPaw] Request Latency Spike (P95)` now evaluate against currently
  emitted metric series.
- Healthy sparse counters evaluate as `0` instead of staying `No Data`, so the
  monitor list can distinguish green coverage from absent telemetry.
- A Trace Analytics migration was considered because OpenPaw spans are present,
  but Datadog rejects changing an existing monitor from `metric alert` to
  `trace-analytics alert`. Keeping the existing monitor IDs preserves webhook
  and AlertCycle continuity for the current remediation. A future migration can
  replace these monitors with new Trace Analytics IDs under an explicit
  delete/create rollout.
