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
  `reset_on = ["Check", "CheckPending"]`) that fires `Check`. There is no
  polling loop in code.

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

## Hardening addendum (2026-08-24, post-review)

Three changes from the adversarial re-review, beyond the first-round P0 fixes:

- **Commit-binding.** The merge PUT pins the PR head sha it just read (`"sha"`),
  so GitHub refuses if the head moved between read and PUT (read→PUT TOCTOU).
  An optional `expected_head_sha` (set via `WorkCycle.ConfigureRelease` →
  `release_expected_head_sha`, carried on `Request`) binds the release to a
  specific reviewed commit; the merge refuses unless the PR head still matches.
  The merged head is recorded (`head_sha`, on `MergeSucceeded`) for audit.
  The merge PUT pins only the head, not the base, so after merging `merge()`
  re-reads the PR and refuses to emit `MergeSucceeded` unless it is now merged
  into `main` at the bound head (base-branch TOCTOU: a maintainer could retarget
  the PR's base between preflight and PUT; GitHub's merge API exposes no
  expected-base). The same re-read reconciles an ambiguous PUT whose connection
  dropped after the merge landed.
- **Rollback isolation.** The revert runs in a fresh `mktemp -d` checkout per
  attempt (never a reused, potentially-poisoned dir), with `GIT_CONFIG_GLOBAL`/
  `GIT_CONFIG_SYSTEM` neutralized and `core.hooksPath=/dev/null`,
  `commit.gpgsign=false` so a planted global config or repo hook cannot execute
  under the release workflow. The token is supplied per-invocation via an
  `http.extraHeader` Authorization header (never written into the remote URL or
  `.git/config`, so it cannot leak through a config left behind by a kill).
  Idempotency is tip-only (HEAD's message), so a stale historical revert never
  skips a needed rollback. The revert adapts to the merge shape — `-m 1` for a
  true merge commit, plain `git revert` for a single-parent (squash/rebase)
  merge reconciled from an out-of-band merge — so no merge shape is
  un-rollbackable.
- **Per-repo serialization (ARN-397, reject).** Before merging, `merge()` reads
  other ReleaseRuns and refuses (→ Fail) if any for the same repo is active
  (Requested/Merging/Watching/Unhealthy). The query filters on active status
  only and matches the repo case-insensitively in code: the kernel's OData `eq`
  is case-sensitive, so pushing `repo eq …` server-side would drop an active
  `Owner/Repo` row before a new `owner/repo` run could be caught (GitHub
  owner/repo is case-insensitive — the same target). The active set is tiny
  (≤1 per repo), so status-only is page-safe, and the read fails closed on
  pagination or a malformed response. This is a read + the run's own Fail — no
  cross-entity dispatch. Residual TOCTOU (two Requests racing the check before
  either commits Merging) needs an atomic per-repo lane entity — the stronger
  form of ARN-397, tracked separately.

Known residual (kernel, ARN-396): `state_timeout`s are in-memory and arm only
on dispatch, so a `Requested`/`Watching` run can still hang across a restart
until durable timeout delivery lands.
