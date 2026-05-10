# ADR-0003: Queued Evaluation Terminal Cleanup

Date: 2026-05-10

Status: Accepted

## Context

Patrol queues an `EvaluationRun` while a `WorkCycle` is still waiting for
independent review. The local worker must not start the evaluation until
`WorkCycle.review_passed` is true.

Before this decision, `paw-codex-worker` waited only for `review_passed`. If the
linked `ReviewRun` ended as `ChangesRequested`, `Escalated`, or `Failed`, or if
the parent `WorkCycle` was already terminal, the queued evaluation could not
become valid but remained `Queued` until the 24-hour entity timeout.

## Decision

Terminal review outcomes clean up the obsolete evaluation in Temper state:

- `review_gate_lifecycle` dispatches `EvaluationRun.Fail` for the attached
  queued or running evaluation when a terminal non-approval review means it can
  no longer become the active evaluation gate.
- The cleanup failure uses
  `failure_classification: "review_terminal_without_approval"`.
- `review_gate_lifecycle` treats cleanup classifications as terminalization
  evidence only; it does not request more rework or fail a parent cycle again
  when the failed evaluation is already obsolete.

The worker also performs a safety check when polling queued evaluations:

- parent `WorkCycle` statuses `Complete` or `Failed` fail the queued evaluation
  with `failure_classification: "parent_work_cycle_terminal"`;
- linked `ReviewRun` statuses `ChangesRequested`, `Escalated`, or `Failed` fail
  it with `failure_classification: "review_terminal_without_approval"`;
- review states that can still lead to approval continue to wait.

This remains Temper-native: state changes are still `EvaluationRun.Fail`
actions, with WorkCycle and ProofPacket side effects handled by
`review_gate_lifecycle` and Cedar policies.

## Consequences

- Queued evaluation residue no longer waits for the coarse entity timeout after
  review dead-ends or parent WorkCycle completion/failure.
- Operators can distinguish cleanup from test failures through
  `failure_classification`.
- Ordinary evaluation command failures still request rework while the WorkCycle
  is `Reviewing`.

## Verification

The behavior is covered by:

- `cargo test -p paw-codex-worker queued_evaluation -- --nocapture`
- `cargo test -p temperpaw --test paw_patrol_foundation queued_evaluation -- --nocapture`
