# Paw Patrol Queued Evaluation Terminal Cleanup

Date: 2026-05-10

## Scope

Fixed the Paw Patrol residue mode where queued `EvaluationRun`s stayed
`Queued` after their parent `WorkCycle` or linked `ReviewRun` could no longer
reach `review_passed`.

## Changes

- `paw-codex-worker` now reads `WorkCycle.Status` and detects terminal blockers
  before waiting indefinitely for `review_passed`.
- The worker claims and fails claimable queued evaluations with:
  - `parent_work_cycle_terminal`
  - `review_terminal_without_approval`
- `review_gate_lifecycle` fails obsolete attached evaluations when review ends
  as changes-requested, escalated, or failed.
- `review_gate_lifecycle` treats those cleanup classifications as
  terminalization evidence only, so cleanup does not fail or rework the parent
  cycle again.
- ADR recorded at
  `os-apps/paw-patrol/adrs/0003-queued-evaluation-terminal-cleanup.md`.

## Verification

Red checks observed before implementation:

- `cargo test -p paw-codex-worker queued_evaluation -- --nocapture`
  - Failed to compile because `WorkCycleState.status` and
    `queued_evaluation_terminal_blocker` did not exist.
- `cargo test -p temperpaw --test paw_patrol_foundation queued_evaluation -- --nocapture`
  - Failed because the cleanup ADR and `review_gate_lifecycle` cleanup hooks did
    not exist.

Final checks:

- `cargo test -p paw-codex-worker queued_evaluation -- --nocapture`
  - Passed: 4 passed, 0 failed.
- `cargo test -p paw-codex-worker review_evaluation_and_work_cycle_state_read_temper_odata_fields -- --nocapture`
  - Passed: 1 passed, 0 failed.
- `cargo test -p temperpaw --test paw_patrol_foundation queued_evaluation -- --nocapture`
  - Passed: 2 passed, 0 failed.
- `cargo test --manifest-path os-apps/paw-patrol/wasm/review_gate_lifecycle/Cargo.toml --quiet`
  - Passed: 3 passed, 0 failed.
- `cargo fmt --check`
  - Passed.

## Notes

No production endpoints, production data, or secrets were touched. The worker
handler regression uses a local in-process fake OData server to verify actual
`EvaluationRun.Claim` and `EvaluationRun.Fail` dispatches.
