# Spec - temperpaw CI for ARN-438

## Item 1: shadow sweep across two repos (one workflow)

`.github/workflows/shadow-sweep.yml` gains a job matrix over
`repo: [nerdsane/temperpaw, nerdsane/temper]`. Each matrix leg:

- checks out ITS repo (`repository: ${{ matrix.repo }}`) so the planning /
  decision-log mirror scripts read that PR's real `docs/efforts/` tree;
- resolves a PR window for that repo (nightly: PRs updated in the last ~48h,
  newest-updated first so the `--limit` cap cannot drop in-window PRs);
- runs `shadow-sweep.py --repo <repo> --repo-dir .` per PR, checking each PR head
  out from that repo's `origin`.

The sweep mints repo-qualified entity ids (`sv-<slug>-prN-gate`, from the stack
half already merged), so temper#5 and temperpaw#5 never collide in prod Temper.

**Token.** GITHUB_TOKEN is scoped to this repo only and cannot read
`nerdsane/temper`. Every git and `gh` call in the workflow uses `STACK_TOKEN` (a
rita-aga PAT that already reads both repos - it clones `arni-labs/stack` and edits
this repo's PR bodies in `sdlc-decision-intake.yml`).

**Dispatch.** `workflow_dispatch` gains a `repo` choice (`both`/`temperpaw`/
`temper`). Explicit `pr_numbers` require a single repo (otherwise which repo owns
the numbers is ambiguous - fail loudly). A targeted dispatch skips the other leg.

Still shadow only: never blocks a merge, never writes to GitHub.

## Item 2: temper pin-bump automation

`.github/workflows/temper-pin-bump.yml`, scheduled daily + `workflow_dispatch`:

1. Read temper main HEAD (`git ls-remote` - temper is public) and the current pin
   (`rev = "<40-hex>"` in `crates/temperpaw/Cargo.toml`).
2. If equal, stop. If a branch `bot/temper-pin-<12hex>` already exists for that
   rev, stop (a prior run already opened it - never re-open a merged/closed bump).
3. Otherwise bump the rev in BOTH `crates/temperpaw/Cargo.toml` and
   `crates/paw-codex-worker/Cargo.toml`, then `cargo update -p <each temper crate>`
   to refresh `Cargo.lock` (a pure sed would miss any new transitive deps the new
   kernel pulls and break `--locked`).
4. Commit, push, and open ONE PR with `STACK_TOKEN`.

**Why STACK_TOKEN, not GITHUB_TOKEN.** A PR created with GITHUB_TOKEN does not
trigger `pull_request` workflows, so the bump's own gates would silently never
fire. STACK_TOKEN is a real PAT whose events are not suppressed (the same reason
`sdlc-decision-intake.yml` already uses it for body edits).

**Gates.** The bump PR touches `Cargo.toml`, so the planning gate is not exempt and
needs `docs/efforts/<id>/`. The PR body references ARN-438, whose design chain
lives on main (this folder), so the planning gate resolves it. The body carries a
`## Decisions & Tradeoffs` section ("No decisions - mechanical bump") for the
decision-log gate. CI compiling and testing `--locked` against the new kernel IS
the proof.

## What this is NOT

No cross-repo event plumbing (no repository_dispatch from temper). A scheduled diff
check is enough and simplest. The pin bump is the deploy leg; the release itself
follows temperpaw's existing release flow once the bump merges.
