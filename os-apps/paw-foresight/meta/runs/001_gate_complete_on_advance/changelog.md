# Run 001 Changelog

## Changed File
`os-apps/paw-foresight/specs/projection.ioa.toml`

## What Changed
Added a counter-predicate guard to the `Projection.Complete` action requiring
`current_step > 0`. The orchestrator can no longer dispatch `Complete`
directly after step 0; at least one `AdvanceStep` must have fired first,
forcing the engine's multi-step progression path to execute as designed.

## Diff
```toml
[[action]]
name = "Complete"
kind = "input"
from = ["Running"]
to = "Complete"
params = []
+guard = "current_step > 0"
-hint = "All steps finished. Mark the projection as complete."
+hint = "All steps finished. Mark the projection as complete. Requires at least one AdvanceStep (current_step > 0) — the state machine refuses Complete on a single-step run so the orchestrator can't short-circuit multi-step progression."
```

## Mechanism

The Temper IOA guard vocabulary supports counter-vs-literal comparisons
(see `paw-pm/cycle.ioa.toml:49`, `paw-agent/specs/session.ioa.toml:591`).
`current_step` is a counter with `initial = "0"`, incremented only by the
`AdvanceStep` action's `increment` effect. Therefore `current_step > 0`
cannot be satisfied without at least one `AdvanceStep` dispatch, making the
precondition state-machine-enforced rather than prompt-advisory.

## Install

Spec-only change. Hot-reload via `POST /api/os-apps/paw-foresight/install`;
no WASM rebuild.
