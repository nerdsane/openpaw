# ADR-0002: Evaluation Timeout Classification

Date: 2026-05-07

Status: Accepted

## Context

Patrol `EvaluationRun`s are the automated gate after implementation and
review. For code-change work, the local `paw-codex-worker` runs configured
shell commands and then reports `EvaluationRun.Pass` or `EvaluationRun.Fail`.

Before this decision, evaluator commands had no local execution timeout. A hung
command could sit until the `EvaluationRun` state timeout fired after 6 hours,
and the resulting failure looked like a generic evaluation failure. That made
reviewer-requested rework harder to classify: a broken test, a hung command,
and an entity-level timeout all collapsed into similar evidence.

## Decision

Evaluator shell commands run under the same local execution budget as local
Codex execution, currently `PAW_CODEX_EXEC_TIMEOUT_SECS`.

Each evaluator command runs in its own process group. If the command exceeds
the budget, `paw-codex-worker` terminates that process group, records command
evidence with:

- `timed_out: true`
- `timeout_ms`
- `failure_classification: "evaluator_timeout"`

The top-level `results_json` also records `failure_classification`. The
`EvaluationRun.Fail` action and entity state now carry a first-class
`failure_classification` field so the Patrol state graph and OData reads expose
the reason without requiring operators to parse `results_json`.

Entity state timeouts remain as crash safety:

- queued but unstarted evaluations fail as `evaluation_not_started_timeout`
- running evaluations that are not reported by a worker fail as
  `evaluation_entity_timeout`

These are distinct from `evaluator_timeout`, which means the local evaluator
worker was alive and classified a specific command as timed out.

## Consequences

- Hung evaluator commands fail quickly and visibly instead of waiting for the
  6-hour `EvaluationRun` timeout.
- Review and rework can distinguish command exit failures from evaluator
  timeout failures.
- The decision keeps the flow Temper-native: the worker only reports
  `EvaluationRun.Fail`; WorkCycle and ProofPacket effects still happen through
  Patrol WASM and Cedar-authorized actions.
- No Datadog dashboard, Discord transport, proof-of-approval, or production
  deployment behavior changes are required by this ADR.

## Verification

The behavior is covered by:

- `cargo test -p paw-codex-worker evaluation_commands_classify_local_timeouts -- --nocapture`
- `cargo test -p temperpaw --test paw_patrol_foundation worker_run_done_fans_out_to_review_evaluation_and_proof -- --nocapture`
