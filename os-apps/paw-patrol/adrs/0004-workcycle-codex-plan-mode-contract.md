# ADR-0004: WorkCycle Codex Plan Mode Contract

**Status:** Accepted
**Date:** 2026-05-12

## Context

Patrol `WorkCycle`s already require `has_plan` before implementation can start,
but intake WASM modules were satisfying that guard with short generic
`plan_summary` strings. High-risk work could pause in
`AwaitingHumanStartApproval` with no meaningful plan for the human to inspect.
The local `paw-codex-worker` also executed implementation directly, so Codex did
not have an enforced read-only planning pass before changing files.

## Decision

`WorkCycle` plans are now revisable state. A `RevisePlan` self-loop is allowed
from `Planned`, `AwaitingHumanStartApproval`, and `InProgress`, and increments
`plan_revision_count`. Patrol intake modules write structured markdown plans
with context, Codex Plan Mode, approach, verification, and risk sections instead
of one-line placeholders.

For implementation WorkerRuns, `paw-codex-worker` runs Codex once with a
read-only sandbox to produce a focused plan. The worker dispatches
`WorkCycle.RevisePlan` with that plan before starting the mutating implementation
pass, and injects the same plan into the implementation prompt.

## Consequences

Humans can inspect a real plan before approving L3 work, and future low-risk
work records the Codex-authored plan shortly after the worker starts. The plan is
still visible through WorkCycle state transitions, preserving the Patrol audit
test: the flow is understandable from entity state alone.
