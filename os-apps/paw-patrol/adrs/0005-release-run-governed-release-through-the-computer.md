# ADR-0005: ReleaseRun — governed release through the computer

**Status:** Accepted (2026-08-24)

## Context

On 2026-08-23 the deep-sci-fi latency fix was merged *through the computer*
(a governed `Exec` on `dsf` calling the GitHub merge API), which made the
merge Cedar-authorized and attributed to the agent's own credential. But that
was where governance stopped: Railway auto-deployed on the push, nothing
watched whether the new build came up healthy, and the only rollback was a
human re-pinning a deployment by hand. `DeployRun` (dsf-deploy) exists but
deploys by HTTP to configured URLs, not through the computer, and was never
used for that release.

The team rule for closing this: orchestration is declarative (automaton +
kernel timers + `[[action.triggers]]`), WASM modules do side effects only and
must not dispatch transitions on other entities.

## Decision

Add `ReleaseRun` to paw-patrol and extend `WorkCycle`:

- `WorkCycle.ConfigureRelease` records the release target (repo, PR number,
  computer, health URL, probe budget) and sets `release_configured`.
- `WorkCycle.Complete` carries an entity-kind trigger guarded by
  `bool_true release_configured` that creates a `ReleaseRun` and fires
  `Request` with `params_from` — no code involved.
- `ReleaseRun`: `Requested → Merging → Watching → Healthy`, with
  `Watching → Unhealthy → RolledBack` on a bad rollout and `Failed` when a
  step cannot run.
- A single WASM module, `release_run_lifecycle`, runs one side effect per
  trigger on the named `Computer`'s sandbox (the same `wasm_helpers::sandbox`
  path as `computer_exec`) and reports through the named callback:
  - `Request` → merge the PR via the GitHub API with the token stored on the
    computer → `MergeSucceeded(merge_sha)`.
  - `Check` → one `curl` of the health URL → `CheckHealthy` when HTTP 2xx,
    `status == healthy`, and the served `git_sha` equals `merge_sha`;
    `CheckPending` otherwise; `CheckUnhealthy` when the new commit serves
    degraded or the probe budget is spent.
  - `CheckUnhealthy` → `git revert -m 1 <merge_sha>` and push `main` →
    `RollbackPushed(revert_sha)`. The push takes the same GitHub-connected
    deploy path as the merge, so the platform redeploys the previous build.
- The watch loop is a kernel `state_timeout` on `Watching` (30s,
  `reset_on = ["CheckPending"]`) that fires `Check`. There is no polling
  loop in code.

The service under release reports the commit it is serving as `git_sha` on
its health endpoint (deep-sci-fi: `RAILWAY_GIT_COMMIT_SHA`), which is what
makes "the rollout landed" a checkable fact rather than a timer guess.

## Consequences

- Merge, watch, and rollback are each a Cedar-gated transition on one row,
  so the whole release reads as state transitions and the audit answers
  who requested it and what happened.
- Rollback is event-driven: an unhealthy rollout reverts itself without a
  human noticing first. Reverting is the rollback (not re-pinning an image)
  so it works for any GitHub-connected service and leaves the reason in git
  history.
- The revert's own rollout is not watched by this version; a follow-up can
  chain a second `ReleaseRun` (or a `Watching`-only mode) on `RolledBack`.
- Fix-specific verification (e.g. "did p95 improve") stays a learned tool
  (`LatencyDiag`), not part of the generic release gate — the generic gate is
  health + served commit only.
- `work_cycle_lifecycle` still dispatches cross-entity actions from WASM
  (`WorkerRun.Configure`, `WorkCycle.StartWork`, …). That predates this rule
  and is flagged here for a separate refactor; `release_run_lifecycle` does
  not follow that pattern.
