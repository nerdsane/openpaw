# ADR-0059: Directed Evolution Stale Stage Cleanup

- Status: Proposed
- Date: 2026-05-27
- Deciders: TemperPaw maintainers

## Context

Directed Evolution evaluation work is asynchronous. A reviewer or simulated-user
`WorkItem` can still be queued after its target `Variant` has already reached a
terminal state. The worker already detects this and cancels the stale
`WorkItem`, but a live Agent Answers run exposed a second-order state leak: the
target `StageResult` can remain `Running` after the work item is cancelled.

That is misleading for Mission Control. Completed episodes should not show live
evaluation stages that no worker will ever complete.

## Decision

When the local Codex worker detects a stale reviewer or simulated-user
`WorkItem`, it will also terminalize the target `StageResult` before cancelling
the work item. If the target stage is still `Running` or already `Failed`, the
worker calls the Directed Evolution app action `EliminateStageResult` with a
stable cleanup rule:

- `EliminationRuleId = stale-after-variant-terminal`
- `EvidenceArtifactId` copied from the stage when present
- `Reason` matching the stale work-item cancellation reason

This keeps app state truthful while preserving the existing hot-load boundary:
the worker uses the already-installed Temper-native Directed Evolution app
action rather than requiring a Railway deployment.

## Consequences

- Mission Control can treat terminal episodes as terminal without hiding stale
  stage rows in the UI.
- Worker cleanup is idempotent with respect to already-terminal stage results.
- Existing `CancelWorkItem` behavior remains intact.
- No Codex process is expected to run in Railway; this remains a trusted local
  TemperPaw worker responsibility.

## Non-Goals

- This ADR does not change the Directed Evolution state machine.
- This ADR does not add a new evaluation stage or selection rule.
- This ADR does not deploy a new Temper server image.
