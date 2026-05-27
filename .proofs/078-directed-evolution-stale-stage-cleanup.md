# Directed Evolution Stale Stage Cleanup Proof

Date: 2026-05-27

## Scope

TemperPaw local Codex worker cleanup for stale Directed Evolution reviewer and
simulated-user work items. When a queued evaluation work item is stale because
its target variant is already terminal, the worker now terminalizes the target
`StageResult` with `EliminateStageResult` before cancelling the work item.

## Static Verification

```text
cargo fmt -p paw-codex-worker
cargo test -p paw-codex-worker directed_evolution --quiet
running 23 tests
23 passed

cargo check -p paw-codex-worker --quiet
passed

git diff --check
passed
```

## Live State Verification

Control tenant:

```text
de-control-agent-answers-20260527001135
```

Before cleanup, completed episode `ep-answer-calibration-live-001` had a stale
stage result:

```text
StageResult en-019e67ac-ab1d-7423-8843-ade1c1a5f466
Status: Running
VariantId: en-019e67ac-aa60-7cf1-a349-fe26ad2aeecb
WorkItemId: en-019e67ac-ab54-7b10-91b7-445c1596e47c
```

The already-hot-loaded Directed Evolution app action was invoked, with no
Railway deployment:

```text
StageResults('en-019e67ac-ab1d-7423-8843-ade1c1a5f466')/Temper.EliminateStageResult
EliminationRuleId: stale-after-variant-terminal
EvidenceArtifactId: en-019e67b0-317f-71b1-b905-b0aa7ab62978
```

After cleanup, the same completed episode had no running or pending stage
results:

```text
Eliminated: 2
Passed: 4
```

## Notes

This proof exercises the live app action path that the worker now calls. The
worker code path is covered by the focused `directed_evolution` unit tests.
