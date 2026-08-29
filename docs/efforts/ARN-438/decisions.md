# Decision log - temperpaw CI for ARN-438

Appended as each call was made. The PR body's `## Decisions & Tradeoffs` carries
these verbatim.

---

**Decision:** Reuse the existing `STACK_TOKEN` secret for the pin-bump PR (and for
the sweep's cross-repo reads) rather than minting a new bot PAT.
**Came up because:** The pin-bump PR must be created by a real PAT, never
GITHUB_TOKEN, or its own gates never fire; the lead asked to check for an existing
suitable secret before requesting a new one.
**Options:** (a) a new dedicated bot identity + secret; (b) reuse STACK_TOKEN,
already present and proven cross-repo.
**Chose (b) over (a) because:** STACK_TOKEN is a rita-aga PAT that already clones
`arni-labs/stack` AND edits this repo's PR bodies in `sdlc-decision-intake.yml` -
precisely because "GITHUB_TOKEN-created events are suppressed." It has the exact
capability needed, so a new identity is machinery for nothing. The panel is
author-independent, so no review value is lost. Given up: a slightly narrower blast
radius a dedicated token might have.
**Where:** `.github/workflows/temper-pin-bump.yml` (token guard + `gh pr create`);
`.github/workflows/shadow-sweep.yml` (checkout + gh calls).

---

**Decision:** Cover both repos with a job matrix inside the single existing
`shadow-sweep.yml`, not a second workflow.
**Came up because:** temper's PRs need the same shadow coverage; the brief said
"one workflow, two repos - don't duplicate."
**Options:** (a) copy the workflow to a temper-specific one; (b) a `repo` matrix.
**Chose (b) over (a) because:** one definition, no drift between two copies; the
per-PR checkout already parameterizes cleanly on `matrix.repo`. Given up: matrix
legs each re-clone stack (minor duplicated setup cost).
**Where:** `.github/workflows/shadow-sweep.yml` (`strategy.matrix.repo`).

---

**Decision:** The pin-bump refreshes `Cargo.lock` with `cargo update -p <crate>`,
not a sed of the lock file.
**Came up because:** CI builds `--locked`; a bumped `Cargo.toml` with a stale lock
fails, and a new kernel can add transitive deps.
**Options:** (a) sed the old sha -> new sha in Cargo.lock too; (b) run `cargo
update` scoped to the temper crates.
**Chose (b) over (a) because:** a sed only rewrites the source lines it already
sees; it cannot add lock entries for deps the new kernel introduces, so `--locked`
would break. `cargo update` re-resolves correctly. Given up: the workflow needs the
rust toolchain (no compile, just resolution - acceptable).
**Where:** `.github/workflows/temper-pin-bump.yml` ("refresh Cargo.lock" step).

---

**Decision:** Bump PRs reference ARN-438 rather than minting a new Linear issue per
nightly bump.
**Came up because:** the bump PR touches `Cargo.toml` (app code), so the planning
gate is not exempt and needs `docs/efforts/<id>/`.
**Options:** (a) a fresh issue + effort folder per bump; (b) reference ARN-438,
whose design chain lives on main; (c) exempt Cargo-only PRs in the stack planning
gate.
**Chose (b) over (a)/(c) because:** (a) floods Linear with one issue per night for
a mechanical change; (c) weakens a shared gate for everyone. ARN-438 owns the
automation, so attributing its executions to it is honest, and the folder already
on main satisfies the gate. Given up: bump PRs are not individually tracked issues
(acceptable - they are visible as PRs and CI runs).
**Where:** `temper-pin-bump.yml` PR body ("ARN-438"); `docs/efforts/ARN-438/`.

---

**Decision:** No cross-repo event plumbing (no `repository_dispatch` from temper);
a scheduled diff check drives the bump.
**Came up because:** the pin bump could be triggered by a temper-side push event.
**Options:** (a) temper fires a dispatch into temperpaw on each main push; (b) a
daily scheduled diff in temperpaw.
**Chose (b) over (a) because:** the lead's ruling - "keep simple, no cross-repo
event plumbing." A scheduled diff needs no secret in temper, no coupling, and a
one-day latency on a pin bump is immaterial. Given up: same-day pickup of a kernel
change (a bump lands within ~24h instead of minutes).
**Where:** `temper-pin-bump.yml` (`schedule` + `workflow_dispatch`).

---

**Decision:** In the sweep, both matrix legs authenticate with STACK_TOKEN and the
now-unused `pull-requests`/`checks` GITHUB_TOKEN permissions are dropped.
**Came up because:** the temper leg cannot use GITHUB_TOKEN (wrong repo scope), and
mixing tokens per leg is needless complexity.
**Options:** (a) GITHUB_TOKEN for the temperpaw leg, STACK_TOKEN for temper; (b)
STACK_TOKEN uniformly.
**Chose (b) over (a) because:** uniform, one code path, and temper needs
STACK_TOKEN regardless. Given up: least-privilege on the temperpaw leg (STACK_TOKEN
is broader than the repo's GITHUB_TOKEN) - acceptable for a nightly shadow job that
already needs the PAT for the other leg.
**Where:** `shadow-sweep.yml` (`permissions: contents: read`; `GH_TOKEN`/`token`).

---

**Decision:** Dedupe bumps by the existence of the rev-specific branch
`bot/temper-pin-<12hex>`.
**Came up because:** a daily schedule would otherwise re-open a PR for the same rev
every day until it merges, or re-open one a human deliberately closed.
**Options:** (a) check for an OPEN PR with that head; (b) check whether the branch
exists at all.
**Chose (b) over (a) because:** (b) also refuses to re-open a bump that was merged
(branch may persist) or closed on purpose - "already handled" is the right test,
not "currently open." Given up: if someone deletes the branch after closing, the
next run re-proposes it (rare, and arguably correct).
**Where:** `temper-pin-bump.yml` ("Skip if a bump for this rev is already open").
