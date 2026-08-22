# Proof Report: 126 — Scoped-routing Temper development image

## Date

2026-08-22

## Branch / Commit

- TemperPaw repository: `nerdsane/temperpaw`, isolated worktree `/private/tmp/temperpaw-task-126`
- TemperPaw branch: `codex/task-126-schema-routing-image`
- TemperPaw push remote: `fork` (`github.com/nikstern/temperpaw`)
- TemperPaw runtime/image commit: `04705038b`
- TemperPaw draft PR: <https://github.com/nerdsane/temperpaw/pull/460>
- Temper repository: isolated worktree `/private/tmp/temper-task-126-typed-auth`
- Reviewed Temper dependency: `nikstern/temper@0190ce8995de1d62cefd1dfe9c39edd3d032aea4`
- Temper draft PR: <https://github.com/nikstern/temper/pull/15>

## What Was Done

- Pinned every Temper manifest and checked-in lock entry to one immutable reviewed revision.
- Preserved typed, tenant-bound authority for verified TemperPaw session cookies after Temper removed the legacy pre-authenticated marker.
- Removed the raw principal-header-only authentication bypass.
- Added the missing app-required `artifact_batch_apply` WASM build to the production image and locked Docker/CI parity with a test.
- Re-ran exact-digest scoped routing, governed deployment/migration/retirement, persistence/restart, reaction recovery, and the complete TemperPaw validation suite.
- Built an immutable local development image without replacing historical task 115 image `sha256:d51ba53f21b6f36a047debad9caa1a5676ac98489d69e44e695676f457cca761`.

Temper ADR-0176 and TemperPaw ADR-0066 record the typed outer-authentication composition. The fork dependency is development-only. Before production merge, replace it with an exact `nerdsane/temper` descendant containing scoped-routing merge `7e3c70dcc00f6e693a637b219d065e10ec862e87` and typed outer authentication, then repeat all pin, auth, scoped-restart, image, and runtime checks.

## Verification Flow

1. Branched an isolated worktree from current GitHub `origin/main` and opened draft PR 460 before implementation.
2. Strengthened the exact dependency contract first and observed all stale manifest/lock pins fail.
3. Selected a reviewed Temper descendant of both routing merge `7e3c70dc` and upstream merge parent `8741bd0`; the initial compile exposed removal of the obsolete authentication marker.
4. In an isolated Temper worktree, wrote failing typed-auth tests, implemented the tenant-bound outer-context primitive, ran mandatory code and DST reviews, and opened draft PR 15.
5. Pinned TemperPaw to final reviewed head `0190ce89`, updated 87 manifest pins and 49 lockfile entries, and proved zero drift.
6. Re-ran Configure → Simulate → restart → Simulate at the exact schema digest, plus scoped pointer-change and Turso-reopen cases.
7. Added a failing image-packaging test for the missing app-required ArtifactBatch integration, then added its Docker build and reran the contract green.
8. Built and inspected the immutable image, booted it with a fresh file-backed store, checked health/readiness and packaged installed-app reconciliation, restarted the same container, and checked recovery again.
9. Ran formatting, check, clippy, and the full TemperPaw test suite.

## Verification Results

| Step | Expected | Actual | Status |
|------|----------|--------|--------|
| Temper ancestry | Exact head contains merged routing fix and reviewed upstream baseline | `git merge-base --is-ancestor` returned 0 for `7e3c70dc` and `8741bd0` against `0190ce89` | PASS |
| Pin contract | One immutable repository/revision everywhere | 87 manifest pins and 49 lock entries at `nikstern/temper@0190ce89`; zero drift | PASS |
| Red/green pin contract | Stale refs fail before rewrite | Initial contract listed stale manifests/locks; exact contract is green | PASS |
| Typed session authority | Verified cookie produces tenant-bound typed principal | Cookie test observed tenant `default`, principal `owner@example.com`, kind `Admin` | PASS |
| Header security | Raw principal headers do not grant authority | Legacy header-only bypass removed; Temper bearer suite 20/20 passed | PASS |
| Configure → Simulate → restart → Simulate | Entity remains on exact scoped schema digest | `task_scoped_action_continuity_survives_restart_with_exact_digest` passed | PASS |
| Scoped routing/restart | Collision, pointer-change, restart, and Turso reopen preserve exact authority | Scoped schema pin suite 9/9 passed | PASS |
| Governed lifecycle and retirement | Submit/verify/activate/migrate/retire preserve governance | Application-data schema deployment suite 4/4 passed | PASS |
| Turso deployment/migration | Durable records and cutover remain valid | Turso schema deployment suite 2/2 passed | PASS |
| Scoped reaction recovery | Reaction reconciles at exact durable pin | Scoped reaction recovery suite 3/3 passed | PASS |
| Image packaging red/green | All app-required WASM is built in CI and Docker | Docker omission failed first; Docker/CI parity contract then passed | PASS |
| Immutable image | Full digest and platform recorded | `sha256:28d3e511ceed57a36dae453b46bf4f81a0d0caa0f4754a94eaf3f2772c20272b`, `linux/arm64` | PASS |
| Initial image boot | Packaged server and installed apps become ready | `/healthz` 200; `/readyz` `status=ready`; image version `task-126` / `04705038b`; ready in 6,490 ms | PASS |
| Container restart | Same durable state recovers and becomes ready | Same container and file-backed volume restarted; `/readyz` 200 and `status=ready`; ready in 441 ms | PASS |
| Installed-app compatibility | Packaged core app surface reconciles | 10 apps reconciled with `wasm_failures: []`; `artifact_batch_apply` registered and post-restart metadata exposes `ArtifactBatch` / `ArtifactBatches` | PASS |
| Full TemperPaw validation | All local gates pass | `cargo test --locked -p temperpaw` passed; format/check/clippy passed | PASS |
| Temper review CI | Reviewed dependency branch passes required jobs | Integrity, compile/lint, storage, foundations, and completed DST lanes green; final random/server lanes were still running at proof capture | PENDING |
| Railway and Datadog | Live production deployment observable | `RAILWAY_TOKEN`, `RAILWAY_PROJECT_ID`, `RAILWAY_ENVIRONMENT`, `DD_API_KEY`, and `DD_APP_KEY` absent | BLOCKED |

## What Worked

- The runtime and every guest dependency resolve to one immutable Temper revision.
- Exact scoped schema authority survives action dispatch, restart, Turso reopen, active-pointer changes, migration, retirement, and reaction recovery.
- Verified local sessions cross the embedded-router boundary as typed tenant-bound authority; raw identity headers no longer bypass authentication.
- Full local TemperPaw validation passes with all app-required image modules represented by a packaging contract.

## What Didn't Work

- The first unprivileged full test run reproduced expected loopback-bind sandbox failures and inherited `DD_ENV=prod`; rerunning with loopback permission and `DD_ENV` removed passed.
- The first Temper PR CI run rejected `.unwrap()` in the new test module. Descriptive `expect(...)` calls fixed the integrity lane; no runtime logic changed.
- Two preflight Docker builds were deliberately canceled when later review exposed a newer exact Temper head and then the missing ArtifactBatch image build. Neither canceled build produced an artifact.

## Limitations

- This is a local development image for the host platform, not a multi-architecture or production release.
- The exact dependency is a temporary fork commit and is forbidden from production merge unchanged.
- No app bytes changed, so Genesis publication is not part of this effort. Genesis remains the production source of truth for installed apps.
- Production deploy and Datadog observation cannot be performed without the absent Railway and Datadog credentials.

## What Still Doesn't Work

- Replace `nikstern/temper@0190ce89` with an exact upstream `nerdsane/temper` descendant and rerun all gates before production merge.
- Railway deployment, live external smoke, and Datadog confirmation await restored credentials and an approved production change after both draft PRs are reviewed.

## Artifacts

- Image tag: `temperpaw-local:task-126-04705038`
- Image digest/platform: `temperpaw-local@sha256:28d3e511ceed57a36dae453b46bf4f81a0d0caa0f4754a94eaf3f2772c20272b` (`linux/arm64`)
- Smoke container: `temperpaw-task126-smoke`
- Durable decision: mem note `121`
- Historical task 115 image retained: `sha256:d51ba53f21b6f36a047debad9caa1a5676ac98489d69e44e695676f457cca761`

## Architecture Diagram

```text
TemperPaw development image
  -> exact reviewed Temper 0190ce89
       -> merged scoped-routing fix 7e3c70dc
       -> typed tenant-bound outer authentication
  -> packaged Genesis-owned OS app bytes
       -> app-required ArtifactBatch WASM included
  -> exact scoped schema authority
       -> governed lifecycle + migration + retirement
       -> durable pin across restart/Turso reopen
```
