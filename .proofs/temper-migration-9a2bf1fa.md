# Proof Report: Temper dependency and typed-auth migration

## Date

2026-08-23

## Branch / Commit

- Repository: `nikstern/temperpaw` on GitHub
- Branch: `codex/temper-migration-9a2bf1fa`
- Red-test commit: `dd7104ba`
- Green implementation commit: `843bc95a`
- Pull request: https://github.com/nikstern/temperpaw/pull/2

## What Was Done

- Repointed every Temper Cargo dependency to `https://github.com/nikstern/temper.git` at merge revision `9a2bf1fa`.
- Regenerated the root and checked-in WASM `Cargo.lock` files at full commit `9a2bf1fa1f1688b4818d6b7e2a3e82449245a0e8`.
- Migrated cookie authentication to inject Temper's typed `AuthenticatedRequestContext` and removed raw principal-header authentication.
- Added a recursive repository contract test that rejects upstream URLs, mixed revisions, branches/tags, and lockfile source drift.

## Verification Flow

1. Added the dependency-source contract test before changing manifests. It failed on the upstream, moving `main` dependency in the deep-sci-fi reference harness.
2. Added typed-auth behavior tests before changing middleware. Cookie requests lacked a typed context, and forged raw principal headers authenticated successfully; both tests failed as expected.
3. Applied the dependency and middleware migrations, regenerated lockfiles, and reran targeted tests.
4. Ran the locked full workspace test and build.
5. Started a clean temporary server with isolated configuration and database. The build-if-missing startup path compiled every packaged WASM module against the migrated SDK.
6. Registered a temporary local user, exercised cookie-authenticated OData, and attempted the same request with forged principal headers only.

## Verification Results

| Step | Expected | Actual | Status |
|------|----------|--------|--------|
| Dependency guard (red) | Existing mixed/upstream source is rejected | Failed on `reference-projects/deep-sci-fi/dsf-harness/wasm/gate_verifier/Cargo.toml` | Pass |
| Typed-context test (red) | Cookie session supplies `AuthenticatedRequestContext` | Typed context was absent before implementation | Pass |
| Forged-header test (red) | Raw principal headers cannot authenticate | Request returned 200 before implementation | Pass |
| `cargo fmt --all --check` | No formatting drift | Clean | Pass |
| Targeted dependency guard | One fork and revision across manifests/locks | 1 passed | Pass |
| Targeted auth tests | Typed cookie context accepted; raw headers rejected | 2 passed | Pass |
| `cargo check -p temperpaw` | Server compiles against the pinned Temper revision | Succeeded | Pass |
| `cargo test --locked --workspace` | Entire workspace remains green | All unit, integration, CLI, and contract suites passed | Pass |
| `cargo build --locked --workspace` | Locked workspace builds | Succeeded | Pass |
| Clean server startup | Server and packaged WASM compile and boot | Listening at temporary `/tdata` endpoint | Pass |
| Local registration | Temporary user can register | HTTP 201 | Pass |
| Cookie OData query | Typed authenticated request reaches entity API | `GET /tdata/Agents?$top=1` returned HTTP 200 and an empty entity collection | Pass |
| Forged principal headers | Raw headers do not cross auth boundary | Same OData request returned HTTP 401 | Pass |

## What Worked

- The new guard covers every checked-in `Cargo.toml` and `Cargo.lock`, not only workspace members.
- Typed authentication composes with the Temper outer router without carrying identity in forgeable HTTP headers.
- The real build-if-missing path compiled and loaded the complete packaged app set.

## What Didn't Work

- The first clean startup in load-only mode correctly stopped because ignored WASM binaries were absent.
- The first build-if-missing attempt found the local `wasm32-wasip1` target missing. Installing that Rust target allowed the full live verification to complete.

## Limitations

- This is pre-merge PR verification. Genesis publication, installed pinned-ref verification, Railway deployment, and Datadog production confirmation must occur after merge.
- No TemperPaw ADR was added: this is a compatibility migration implementing the architecture already established by upstream Temper ADR-0176, not a new local architecture decision. The durable project decision is recorded in `mem` as required.

## What Still Doesn't Work

- Nothing found within the requested migration scope.

## Artifacts

- Draft PR: https://github.com/nikstern/temperpaw/pull/2
- Full Temper merge commit: `9a2bf1fa1f1688b4818d6b7e2a3e82449245a0e8`

## Architecture Diagram

```text
cookie session -> TemperPaw auth -> AuthenticatedRequestContext -> Temper router
raw identity headers -----------X--------------------------------> rejected (401)
```
