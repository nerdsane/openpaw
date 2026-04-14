# Proof Report: 039 — Durable Query Plane Rebuild

## Date
2026-04-13

## Branch / Commit
- **openpaw**: `main` (`a8fdc585`)
- **temper**: `feat/governance-decision-callbacks` (`e74dd6f`)

## What Was Done
Implemented the first production slice of the durable query-plane / bounded actor-residency re-architecture:

1. Added a durable `entity_catalog` projection alongside `entity_field_index` in Turso.
2. Centralized projection writes so dispatch updates and deletes both query-plane tables synchronously.
3. Reworked startup query-plane backfill to rebuild from snapshots plus persistence replay without hydrating actors.
4. Added projection coverage metrics:
   - `temper_projected_entities`
   - `temper_projection_coverage_ratio`
   - `temper_projection_backfill_snapshot_misses_total`
5. Updated the OpenPaw Datadog dashboard to surface the new projection metrics.
6. Added integration coverage in Temper for live projection maintenance and startup rebuild behavior.
7. Added an OpenPaw regression test asserting the dashboard carries the expected tenant-aware entity and projection metric queries.
8. Patched the OpenPaw workspace to resolve local Temper crates during verification so the build/test/e2e runs exercised the modified Temper checkout rather than GitHub.

## Verification Flow
1. Run focused Temper tests for the new projection storage and startup rebuild paths.
2. Run the full Temper workspace test sweep.
3. Build OpenPaw against the local Temper checkout.
4. Run OpenPaw workspace tests against the local Temper checkout.
5. Reproduce the pre-fix restart bug by wiping the query plane and restarting OpenPaw.
6. Patch OpenPaw startup so query-plane recovery finishes before post-boot bootstrap/soul startup.
7. Copy the reproduced database, wipe `entity_catalog` and `entity_field_index`, and restart OpenPaw on the fixed binary.
8. Confirm the restart log shows query-plane rebuild from snapshots + persistence replay before the server starts listening.
9. Confirm filtered OData reads succeed again and the active `Paw` agent count does not increase after the wiped-query-plane restart.

## Verification Results
| Step | Expected | Actual | Status |
|------|----------|--------|--------|
| `cargo test -p temper-store-turso -- --nocapture` | Query-plane storage tests pass | 17 passed | PASS |
| `cargo test -p temper-server --test query_projection_backfill -- --nocapture` | Startup/live projection integration tests pass | 2 passed | PASS |
| `cargo test -p temper-server -- --nocapture` | Full `temper-server` suite passes with the new projection path | PASS | PASS |
| `cargo test --workspace -- --nocapture` in `temper` | Full Temper workspace passes | PASS | PASS |
| `cargo build -p openpaw` with local Temper patch | OpenPaw builds against local Temper checkout | PASS | PASS |
| `cargo test -p openpaw -- --nocapture` | OpenPaw regression/unit tests pass on the fixed startup ordering | 15 passed | PASS |
| `cargo test --workspace -- --nocapture` with local Temper patch | OpenPaw workspace tests pass against local Temper checkout | `openpaw`: 15 passed; `paw_transport`: 0 tests / 0 doctests failed | PASS |
| Pre-fix wiped-query-plane restart | Reproduces the bug before the startup ordering fix | Query-plane rebuild finished after `Phase 9`, and active `Paw` agents grew from 1 to 2 | PASS |
| Fixed wiped-query-plane restart | Query-plane rebuild finishes before serving traffic | `Phase 7` at line 5366, projection rebuild completes at lines 7444-7446, `Phase 8/9` at 7447-7448, listener starts at 7466 | PASS |
| Fixed wiped-query-plane restart OData verification | Filtered reads work and no extra `Paw` agent is minted | `GET /tdata/Agents?$filter=name eq 'Paw' and Status eq 'Active'` returned `2`, unchanged from pre-wipe state | PASS |
| Fixed wiped-query-plane restart SQLite verification | Query-plane tables are repopulated from persistence replay without actor-hydration fallback | before wipe: `797/5991/3044`; after wipe: `0/0/3044`; after fixed rebuild: `1020/7615/3975` (`entity_catalog` / `entity_field_index` / `events`) | PASS |
| Fixed wiped-query-plane restart health check | API becomes healthy after recovery completes | `/healthz` returned `200` | PASS |

## What Worked
- The OpenPaw workspace now resolves `temper-server` and its supporting crates from `../temper` during verification.
- Durable projection writes are exercised both in the store layer and through server integration tests.
- The dashboard regression test now covers the new projection metrics as well as the tenant-aware entity query.
- The fixed startup ordering closes the bug we reproduced earlier: query-plane recovery now finishes before `Phase 8`, `Phase 9`, and the listener coming up, so `bootstrap_agent()` no longer races against an empty query plane.
- End-to-end restart verification on a wiped query plane kept the active `Paw` count stable at `2` instead of creating a third active `Paw`.

## What Didn't Work
- First-run OpenPaw boot remains very heavy because OS-app/bootstrap seeding is large.
- `monty_repl` still logs a pre-existing WASM compilation warning (`__wbindgen_describe` import missing) during boot.

## Limitations
- This slice does not yet reduce first-run bootstrap work; it removes actor hydration from projection rebuilds but does not yet shrink the broader app/doc bootstrap volume.
- The full `cargo test -p temper-server -- --nocapture` sweep is significantly slower than the targeted integration tests because the random/DST suites are large.

## What Still Doesn't Work
- Startup still does a large amount of app/content seeding on an empty database before the API becomes responsive.
- `monty_repl` still fails to compile during bootstrap.

## Artifacts
- Pre-fix reproduction temp dir: `/tmp/openpaw-query-plane-local2.JKnTw6`
- Pre-fix reproduction logs:
  - `/tmp/openpaw-query-plane-local2.JKnTw6/proof2/server2.log`
  - `/tmp/openpaw-query-plane-local2.JKnTw6/proof2/server3.log`
- Fixed restart proof temp dir: `/tmp/openpaw-query-plane-final.wqVmur`
- Fixed restart proof artifacts:
  - `/tmp/openpaw-query-plane-final.wqVmur/proof/server-fixed.log`
  - `/tmp/openpaw-query-plane-final.wqVmur/proof/counts-before-wipe.txt`
  - `/tmp/openpaw-query-plane-final.wqVmur/proof/paw-active-before-wipe.txt`
  - `/tmp/openpaw-query-plane-final.wqVmur/proof/counts-after-wipe.txt`
  - `/tmp/openpaw-query-plane-final.wqVmur/proof/counts-after-rebuild-fixed.txt`
  - `/tmp/openpaw-query-plane-final.wqVmur/proof/health-fixed-code.txt`
  - `/tmp/openpaw-query-plane-final.wqVmur/proof/agents-active-after-fixed.json`
  - `/tmp/openpaw-query-plane-final.wqVmur/proof/agents-active-count-after-fixed.txt`
  - `/tmp/openpaw-query-plane-final.wqVmur/proof/agents-paw-after-fixed.json`
  - `/tmp/openpaw-query-plane-final.wqVmur/proof/agents-paw-count-after-fixed.txt`
- Local Temper-backed OpenPaw binary: `/Users/seshendranalla/Development/openpaw-codex/target/debug/openpaw`

## Architecture Diagram
```text
                    collection read / OData filter
                                |
                                v
                      +---------------------+
                      |  durable query plane|
                      | entity_catalog      |
                      | entity_field_index  |
                      +---------------------+
                                |
                 rebuild from snapshots + persistence replay
                                |
                                v
                      +---------------------+
                      | event store truth    |
                      | events + snapshots   |
                      +---------------------+
                                |
                                v
                      +---------------------+
                      | actor runtime/cache |
                      | hot entities only   |
                      +---------------------+
```
