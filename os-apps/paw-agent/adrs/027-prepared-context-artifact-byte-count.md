# ADR-027: Prepared Context Artifact Byte Count Metric

## Status

Accepted

## Context

PERF-034 raised the default prepared-context inline artifact budget from 32 KiB
to 128 KiB and added two metrics:

- `temper_session_prepared_context_artifact_bytes`
- `temper_session_prepared_context_artifact_storage_total`

The live production proof on `sha-b30947f` showed that
`temper_session_prepared_context_artifact_storage_total{mode:inline}` reached
Datadog, but the byte-size metric did not appear in Datadog metric metadata or
metric queries. The context-preparer emits the byte metric immediately before
the storage-mode count metric, so the execution path is proven. The likely gap
is in the current histogram/distribution export path for newly introduced sparse
guest metrics, while count-style guest metrics are already proven visible.

The latency program needs byte-size observability for correctness and speed:
inline artifact choices must be auditable by size, and future tuning must know
whether contexts are staying within the intended budget.

## Decision

Keep the existing distribution-style metric
`temper_session_prepared_context_artifact_bytes` and add a count-style companion:

- `temper_session_prepared_context_artifact_bytes_total`

The companion records the artifact byte length as a count value with the same
tags as the storage-mode metric (`provider`, `model`, and `mode`), plus the host
context tags added by Temper (`tenant`, `entity_type`, `trigger_action`, and
`wasm_module`).

This gives Datadog an immediately reliable total-byte signal through the metric
path that is already proven live, while preserving the intended distribution
metric for p50/p95 artifact-size analysis once the histogram export behavior is
understood or repaired.

## Consequences

- Datadog can sum prepared-context artifact bytes by mode, provider, model,
  version, and WASM module.
- The storage-mode count and byte total can be joined to estimate mean bytes per
  artifact for a mode/version window.
- The existing distribution metric remains in place and can still become useful
  without another app change if the exporter issue is resolved.
- The extra count metric adds one low-cardinality custom metric family.
