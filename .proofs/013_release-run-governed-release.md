# Proof Report: 013 — ReleaseRun: governed merge, rollout watch, and rollback through the computer

## Date
2026-08-24

## Branch / Commit
`claude/release-run` @ `22e20aaf` (temperpaw) · Genesis `temperpaw/paw-patrol@a3bdcbce` · PR nerdsane/temperpaw#466

## What Was Done
Closed the gap found on 2026-08-23: the DSF fix was *merged* through the computer (governed Exec) but the rollout was **never watched** and rollback was **manual**. Added `ReleaseRun` to paw-patrol so merge → watch → rollback is one governed state machine:
- `WorkCycle.ConfigureRelease` records the release target; `WorkCycle.Complete` carries a **guarded entity-kind trigger** (`bool_true release_configured`) that declaratively creates a `ReleaseRun` and fires `Request`. No WASM dispatches it.
- `release_run_lifecycle` WASM does **side effects only**, one per trigger, on the named Computer's sandbox, reporting via named callbacks: `Request`→merge PR via GitHub API; `Check`→one health probe; `CheckUnhealthy`→`git revert -m 1` + push.
- The watch loop is a kernel `state_timeout` on `Watching` (30s, `reset_on = CheckPending`), bounded by `max_checks` (default 60 = 30 min).
- DSF `/health` now emits `git_sha` (`RAILWAY_GIT_COMMIT_SHA`, PR arni-labs/deep-sci-fi#106) so "the rollout landed" is checkable (served `git_sha == merge_sha`).

## Verification Flow
1. Formal: `verify-ioa` on ReleaseRun + WorkCycle; whole-app composite verify.
2. Unit: `cargo test` on `release_run_lifecycle`.
3. Build + boot: `temperpaw-server` boots and loads paw-patrol incl. `release_run_lifecycle`.
4. Publish + install: Genesis PublishNewVersion → `install-from-genesis` on openpaw-production.
5. Live healthy path: drove a real WorkCycle to `Complete` for DSF PR #109; watched the ReleaseRun merge on `dsf`, poll DSF `/health` through CI+deploy, reach `Healthy`.
6. Live rollback path: same for PR #110 with `max_checks=2`; watched it declare `Unhealthy` and `git revert` + push, reaching `RolledBack`; confirmed on GitHub.

## Verification Results
| Step | Expected | Actual | Status |
|------|----------|--------|--------|
| verify-ioa ReleaseRun | L0–L3 pass | L0–L3 pass (17 states, 100 prop cases, 0 unreachable) | ✅ |
| verify-ioa WorkCycle | L0–L3 pass | L0–L3 pass (358 states) | ✅ |
| composite verify paw-patrol | PASS | PASS (20 entity types) | ✅ |
| release_run_lifecycle unit tests | all pass | 21/21 | ✅ |
| local server boot | loads release_run_lifecycle | loaded, listening | ✅ |
| Genesis publish + prod install | ReleaseRun added, WASM loaded | added ReleaseRun, updated WorkCycle, release_run_lifecycle loaded | ✅ |
| Healthy: trigger creates ReleaseRun | ReleaseRun on Complete | created, `Request` fired | ✅ |
| Healthy: merge #109 on computer | PR merged, merge_sha recorded | PR #109 MERGED, mergeCommit `386301bbdda4` == merge_sha | ✅ |
| Healthy: watch to healthy | served git_sha == merge_sha → Healthy | 26 CheckPending during CI+deploy, then `Healthy`, observed_sha `386301bbdda4` | ✅ |
| Rollback: merge #110 | PR merged | PR #110 MERGED, merge_sha `1021a38e4ec9` | ✅ |
| Rollback: unhealthy after budget | CheckUnhealthy at max_checks | Unhealthy at check 2 (reason: "not healthy after 2 checks") | ✅ |
| Rollback: auto-revert | git revert + push → RolledBack | `RolledBack`, revert_sha `fd082b3eb5`; main HEAD = `fd082b3eb52e Revert "Merge pull request #110…"`, #110 marker gone from main | ✅ |
| DSF stays healthy | no broken prod | healthy throughout (served #110 then converges to revert) | ✅ |

## What Worked
- The declarative `Complete → ReleaseRun.Request` trigger fired with no imperative dispatch.
- Merge, watch (kernel timer), and rollback all ran on the real `dsf` sandbox via governed transitions.
- The `git_sha`-on-`/health` contract made "healthy" a real check (served commit == merge), not a timer guess.
- The 30-min watch budget correctly outlasted DSF's CI-gated deploy (~13 min / 26 checks) on the healthy path.

## What Didn't Work (found + fixed by the live run)
- **First merge failed with GitHub "Bad credentials"** — the initial `cut`-based token extraction from `~/.git-credentials` produced a wrong token. Fixed to a `sed` capture of the credential password field, verified to authenticate (HTTP 200) on the computer before redeploying. This is exactly why the live run matters.

## Limitations
- The revert's *own* rollout is not watched by this version (a follow-up can chain a second ReleaseRun on `RolledBack`).
- Fix-specific verification (e.g. "did p95 improve") stays a learned tool (LatencyDiag), not part of the generic release gate — the gate is health + served commit only.
- Driving `WorkCycle` as agent_type `harness` requires Cedar approval per action (surfaced via elicitation) — expected governance, approved inline during the run.

## What Still Doesn't Work
- `work_cycle_lifecycle` (pre-existing) still dispatches cross-entity actions from WASM — the same anti-pattern. Flagged in ADR-0005; scheduled as the immediate follow-up (separate PR).

## Artifacts
- PR: https://github.com/nerdsane/temperpaw/pull/466
- Genesis: `temperpaw/paw-patrol@a3bdcbce`
- DSF PRs exercised: #109 (healthy, merged `386301bbdda4`), #110 (rollback, merged `1021a38e4ec9` then reverted `fd082b3eb5`)
- ReleaseRun rows: healthy `01a0317b-68dc-7451-bb97-d944f15087c9` (Healthy), rollback `01a03188-198a-7611-9258-3e56b6039d42` (RolledBack)
- ADR: `os-apps/paw-patrol/adrs/0005-release-run-governed-release-through-the-computer.md`

## Architecture Diagram
```text
 WorkCycle ...gates... ─► Complete ─┐  (guard: release_configured)
                                    │  [[action.triggers]] kind=entity  (declarative)
                                    ▼
                              ReleaseRun.Request
                                    │
      release_run_lifecycle (WASM, side-effect only, on computer dsf)
         Request  ─► merge PR via GitHub API ─► MergeSucceeded(merge_sha) ─► Watching
                                                                               │
   kernel state_timeout (30s, reset_on=CheckPending, ≤max_checks) ─► Check ────┤
         Check ─► curl /health ─► served git_sha == merge_sha & healthy ─► CheckHealthy ─► Healthy ✔
                               ─► else, budget left                     ─► CheckPending  (re-arm)
                               ─► else, budget spent / degraded         ─► CheckUnhealthy ─► Unhealthy
                                                                               │
         CheckUnhealthy ─► git revert -m 1 <merge_sha> + push ─► RollbackPushed ─► RolledBack ✔
```
