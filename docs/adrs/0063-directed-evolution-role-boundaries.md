# ADR-0063: Directed Evolution Role Boundaries

## Status

Accepted.

## Context

The first Directed Evolution worker path used Codex for variant generation,
simulated-user evaluation, review, selection, and promotion. It successfully
executed a full loop, but worker output contracts allowed simulated users to
return `passed` and metrics directly. Some simulated-user prompts also included
Datadog evidence work.

That made the proof hard to audit because simulated user observation,
deterministic validation, telemetry measurement, and selector judgment were
not cleanly separated.

## Decision

The worker keeps Codex as the development brain, but role contracts are split:

- `variant_generator`: may write an organism candidate and return app refs,
  changed files, diff refs, and verification notes.
- `simulated_user`: uses the app, records journey steps, observations, intent
  satisfaction, friction, and evidence refs; it must not emit pass/fail for a
  variant.
- `state_verifier`: checks Temper/OData/app state and emits state-verified
  measurements and evidence.
- `telemetry_evaluator`: queries Datadog or other telemetry and emits
  datadog-measured/runtime-measured summaries.
- `wasm_evaluator`: runs a deterministic evaluator module and emits
  wasm-computed measurements.
- `viability_evaluator` or `reviewer`: decides stage pass/fail from recorded
  evidence and constraints.
- `selector`: chooses among surviving variants and explains tradeoffs without
  modifying evaluator rules or variants.
- `promoter`: materializes the selected app ref through Genesis hot-load.

Every worker result must identify its provenance class. A simulated-user result
without evaluator interpretation is not a stage pass.

## Consequences

Older stage-evaluation prompts remain legacy. New proof runs must use split
roles so a reviewer can see what was observed, what was measured, what was
computed, and what was judged.
