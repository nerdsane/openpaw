# Proof Report: 080 - Infinite Logical History With Bounded Actors

## Date

2026-05-29

## Branch / Commit

- Temper: merged PR #287 to `main` at `5ee4429f45d8f2bcf48f1269e377ef79b2c5544c`
- TemperPaw implementation: merged PR #348 to `main` at `ec02343441d643de848da2e1a6a03e2c523591d2`
- TemperPaw deploy-version fix: merged PR #350 to `main` at `702f42693fc567c03c0c2e049a86a4d3dab611e7`
- Production image: `ghcr.io/nerdsane/temperpaw:sha-702f426`

## What Was Done

- Moved Temper actor budgeting from lifetime event count to snapshot-tail count.
- Added storage-level event segment metadata and immutable snapshot history in Temper stores.
- Updated TemperPaw to pin the new Temper revision across server, Dockerfile, and packaged WASM crates.
- Updated Temper's OS app loader to discover packaged `wasm/<module>/<module>.wasm` files after Docker target pruning.
- Moved PawFS hot file IO off the Workspace actor for Monty agent operations.
- Added `WorkspaceUsageBucket` and `ArtifactBatch` entities.
- Added `artifact_batch_apply` WASM for multi-file direct Directory/File/FileVersion writes plus one usage-bucket delta.
- Added bounded 16 MiB Tokio worker stacks in TemperPaw so WASM loopback OData requests do not overflow the default worker stack.
- Added direct-entity read/list Cedar permits for PawFS hot-path agents.
- Added deploy/runtime version variable proof so Railway `BUILD_SHA`, `BUILD_VERSION`, and `DD_VERSION` match the deployed image.
- Added ADRs for segmented platform history and PawFS hot-path ownership.

## Local Verification Results

| Step | Expected | Actual | Status |
|------|----------|--------|--------|
| Temper pre-push suite | All platform gates pass | rustfmt, clippy, readability, full tests, and doctests passed before merge | PASS |
| Temper GitHub CI | Remote PR checks pass | PR #287 passed compile/lint, tests, DST shards, spec verification, instrumentation, and verification contract | PASS |
| `cargo fmt --check` | Formatting clean | Passed | PASS |
| `cargo check --locked -p temperpaw -p paw-codex-worker` | Locked merged Temper SHA resolves | Passed | PASS |
| `cargo clippy --locked -p temperpaw -p paw-codex-worker --all-targets -- -D warnings` | CI clippy surface passes | Passed | PASS |
| `cargo test --locked -p temperpaw --quiet` | TemperPaw tests pass | Passed | PASS |
| `cargo test --locked -p temperpaw --test paw_fs_hot_path` | PawFS contracts pass | 12 tests passed | PASS |
| `cargo test --locked -p temperpaw --test datadog_observability_contract` | Observability pin contracts pass | 32 tests passed | PASS |
| `cargo test --locked -p temperpaw --test temperpaw_identity_contract temperpaw_runtime_uses_bounded_large_stack_workers_for_wasm_loopback_io` | Runtime stack contract passes | Passed | PASS |
| `cargo test --locked -p temperpaw --test temperpaw_identity_contract manual_railway_redeploy_workflow_is_secret_backed_and_version_proven` | Deploy workflow contract passes | Passed | PASS |
| `cargo test --manifest-path os-apps/paw-agent/wasm/monty_repl/Cargo.toml --lib` | Agent-facing PawFS tools pass | 69 tests passed | PASS |
| `os-apps/paw-fs/wasm/artifact_batch_apply/build.sh` | WASM builds | Built `artifact_batch_apply.wasm` | PASS |
| `os-apps/paw-fs/wasm/workspace_fs/build.sh` | WASM builds | Built `workspace_fs.wasm` | PASS |
| CI-style os-app WASM build sweep | Runtime WASM modules link against merged Temper SDK | All CI-listed os-app build scripts passed locally with shared host-import linker env | PASS |
| Local ready check | Server ready | `/readyz` returned `{"status":"ready"}` | PASS |
| Production verifier script, local target | Same script planned for Railway works locally | `scripts/production_artifact_batch_e2e.sh` passed against `http://127.0.0.1:4792` | PASS |
| ArtifactBatch apply | Batch completes | `ArtifactBatch` `ab-artifact-batch-e2e-local-merged-20260529c` reached `Completed` | PASS |
| File readback | Three files readable | 48-byte markdown, 43-byte JSON, 38-byte text read back exactly | PASS |
| Usage accounting | Usage bucket records one bounded delta | `WorkspaceUsageBucket.ApplyDelta` recorded `bytes_delta=129`, `file_delta=3` | PASS |
| Workspace hot path | No Workspace file IO events | No `MkDir`, `CreateFile`, `ResolvePath`, `ListDir`, `IncrementUsage`, or `IncrementFileCount` events | PASS |
| Workspace event count | Workspace unchanged after batch | Target Workspace has only `Created` | PASS |
| WASM hash reconcile | Observed hash equals packaged hash | `artifact_batch_apply` hash `4b3b6f5ea2b6bf0d4dab46a9e17a6d286b82f2014ad07a7b3059df62ed4fba23` matched packaged file | PASS |

## Temper Verification

- `cargo test -p temper-server event_budget --lib`
- `cargo test -p temper-platform find_wasm_modules --lib`
- `cargo test -p temper-platform test_os_app_document_bootstrap_does_not_charge_workspace_file_count --lib`
- `cargo test -p temper-store-postgres migration --lib`
- `cargo test -p temper-store-turso event_history_schema_declares_segments_and_snapshot_history --lib`
- `cargo test -p temper-store-sim snapshot_save_records_history_and_rotates_segments --lib`
- `cargo test -p temper-store-redis --lib`
- `cargo check -p temper-server -p temper-platform -p temper-store-redis -p temper-store-turso -p temper-store-postgres -p temper-store-sim`
- Full Temper pre-push gate on `0242d318a107a5399c62ed4e0d2e90240c11146b`: PASS
- GitHub CI for Temper PR #287 on `0242d318a107a5399c62ed4e0d2e90240c11146b`: PASS
- Merged Temper commit: `5ee4429f45d8f2bcf48f1269e377ef79b2c5544c`

## Local E2E Artifacts

```text
Local DB: /tmp/temperpaw-infinite-history-e2e-merged.db
Workspace: ws-artifact-batch-e2e-local-merged-20260529c
ArtifactBatch: ab-artifact-batch-e2e-local-merged-20260529c
UsageBucket: en-019e71bc-b11c-7863-b089-d454851417fd

Readbacks:
- /katagami/deploy-e2e/local-merged-20260529c/language.md: 48 bytes
- /katagami/deploy-e2e/local-merged-20260529c/tokens.json: 43 bytes
- /katagami/deploy-e2e/local-merged-20260529c/review.txt: 38 bytes

WASM module:
- module_name=artifact_batch_apply
- sha256_hash=4b3b6f5ea2b6bf0d4dab46a9e17a6d286b82f2014ad07a7b3059df62ed4fba23
- total_invocations=1
- success_count=1
```

## Event Store Proof

```text
Workspace events:
sequence_nr  segment_index  event_type
1            0              Created

Workspace hot path file IO events:
0

event_segments rows:
present and populated

snapshot_history rows:
present

ArtifactBatch events:
1  Created
2  Submit
3  Apply
4  RecordFileApplied
5  RecordFileApplied
6  RecordFileApplied
7  Complete
```

## What Worked

- Existing Workspace actors are no longer charged for hot PawFS writes.
- The local server discovered the packaged nested WASM module path after startup rebuild.
- The direct ArtifactBatch WASM path created directories, files, file versions, and a usage bucket without Workspace file IO events.
- Snapshot-tail metadata is visible in OData entity responses as `events_since_snapshot` and `last_snapshot_sequence_nr`.
- The platform segment tables are present and populated in local Turso.
- Production survived restart with the durable `ArtifactBatch` spec, `artifact_batch_apply` module, and PawFS Cedar policies intact.

## Issues Found And Fixed During E2E

- Initial local ArtifactBatch invocation crashed with a Tokio worker stack overflow during WASM loopback OData. Fixed by using an explicit bounded 16 MiB Tokio worker stack in TemperPaw.
- ArtifactBatch agents could apply workflow actions but could not read/list the direct PawFS entities they needed to query. Fixed Cedar policies for Directory, ArtifactBatch, and WorkspaceUsageBucket.
- The proof harness originally assumed top-level `Id`; OData entity read models expose `entity_id` and nested `fields.Id`, so the verifier now checks those shapes.
- The deploy verifier needs both mutation and observe privileges. It now uses an `agent/system` principal for OData actions and an `admin` principal for `/observe` module/hash/history proof.
- GitHub CI exposed that merged Temper SDK host functions must remain unresolved imports during standalone WASM linking. Fixed all os-app build scripts to source `os-apps/wasm-build-env.sh`, which applies `-C link-arg=--allow-undefined`, and added a static regression test.
- First Railway deploy proved the image had changed but `/paw/version` was stale because Railway service variables still held the previous `BUILD_SHA` and `BUILD_VERSION`. Fixed the setup API and manual redeploy workflow to set `IMAGE_TAG`, `BUILD_SHA`, `BUILD_VERSION`, and `DD_VERSION` together.
- GitHub `Railway Redeploy` workflow run `26617570543` could not execute because the repository/environment secrets `RAILWAY_TOKEN`, `RAILWAY_PROJECT_ID`, `RAILWAY_ENVIRONMENT_ID`, `RAILWAY_SERVICE_ID`, and `TEMPER_API_KEY` were not configured. Deployment and verification were completed with the authenticated local Railway CLI against the checked-in production project/service IDs.
- Production genesis bootstrap still pins `temperpaw/paw-fs@65f3ee9659500d11a54c22b9e5519d52dd0db1d4`, and the current Genesis smart HTTP route returned `{"error":"no route matches","path":"/temperpaw/paw-fs.git/info/refs"}`. Production was repaired through Temper's supported spec/WASM/policy hotload APIs; follow-up is to publish the new PawFS app ref through Genesis once that route or an equivalent registry path is restored.
- After the first production hotload, `ArtifactBatch` specs and WASM survived restart but Cedar grants did not because the inline spec load activated policies in memory. Fixed production by persisting the PawFS policy bundle as named tenant policy rows.

## Deployment Verification

| Step | Expected | Actual | Status |
|------|----------|--------|--------|
| Temper PR | Platform implementation merged | PR #287 merged with GitHub CI green | PASS |
| TemperPaw PR | PawFS/ArtifactBatch implementation merged | PR #348 merged with GitHub CI green | PASS |
| Deploy-version PR | Railway version variables stay aligned | PR #350 merged with GitHub CI and Docker build green | PASS |
| Docker image | Image built from deployed SHA | `ghcr.io/nerdsane/temperpaw:sha-702f426`, digest `sha256:95c0be0e9ee1ac837f951302d01a6025fc6543d2e819b15e404b50faf137a442` | PASS |
| Railway deploy | Production runs final image | Deployment `26754ae2-ed21-4c10-997c-4a08451957fd` pulled `sha-702f426` | PASS |
| Runtime version | `/paw/version` reports deployed SHA | `version=sha-702f4269`, `sha=702f42693fc567c03c0c2e049a86a4d3dab611e7` | PASS |
| PawFS spec hotload | Production has `ArtifactBatch` and `WorkspaceUsageBucket` specs | `/api/specs/load-inline` verification passed all PawFS entities | PASS |
| WASM hotload | Production module hash matches packaged file | `artifact_batch_apply` hash `4b3b6f5ea2b6bf0d4dab46a9e17a6d286b82f2014ad07a7b3059df62ed4fba23` | PASS |
| Production E2E | Multi-file batch completes without Workspace hot-path events | `prod-20260529-final-702f426b` completed, files readable, usage bucket updated, Workspace events `["Created"]` | PASS |
| Policy persistence | PawFS Cedar grants survive restart | Six named PawFS policy rows persisted under tenant `default` | PASS |
| Restart proof | Deployed process restarts on same image | Railway redeploy `69b2d082-d0c4-4920-9e5c-f835ec904dbc` returned ready | PASS |
| Post-restart E2E | Same multi-file batch completes after restart | `prod-20260529-restart-policy-702f426` completed, files readable, usage bucket updated, Workspace events `["Created"]` | PASS |

## Production E2E Artifacts

```text
Production base URL: https://openpaw-production.up.railway.app
Final deployed SHA: 702f42693fc567c03c0c2e049a86a4d3dab611e7
Final deployed version: sha-702f4269
Production image: ghcr.io/nerdsane/temperpaw:sha-702f426
Image digest: sha256:95c0be0e9ee1ac837f951302d01a6025fc6543d2e819b15e404b50faf137a442
Deployment id: 26754ae2-ed21-4c10-997c-4a08451957fd
Restart proof deployment id: 69b2d082-d0c4-4920-9e5c-f835ec904dbc

WASM module:
- module_name=artifact_batch_apply
- sha256_hash=4b3b6f5ea2b6bf0d4dab46a9e17a6d286b82f2014ad07a7b3059df62ed4fba23
- packaged_hash=4b3b6f5ea2b6bf0d4dab46a9e17a6d286b82f2014ad07a7b3059df62ed4fba23
- persisted after restart: yes

Post-policy production run:
- Workspace: ws-artifact-batch-e2e-prod-20260529-policy-702f426
- ArtifactBatch: ab-artifact-batch-e2e-prod-20260529-policy-702f426
- UsageBucket: en-019e7230-ff38-7732-97d8-d41cbc0aa1fb
- Workspace events: Created only

Post-restart production run:
- Workspace: ws-artifact-batch-e2e-prod-20260529-restart-policy-702f426
- ArtifactBatch: ab-artifact-batch-e2e-prod-20260529-restart-policy-702f426
- UsageBucket: en-019e7231-d9cb-7651-beee-550d607af307
- Readbacks:
  - /katagami/deploy-e2e/prod-20260529-restart-policy-702f426/language.md: 48 bytes
  - /katagami/deploy-e2e/prod-20260529-restart-policy-702f426/tokens.json: 43 bytes
  - /katagami/deploy-e2e/prod-20260529-restart-policy-702f426/review.txt: 38 bytes
- Usage delta: bytes_delta=129, file_delta=3
- Workspace events: Created only
```

## Architecture Diagram

```text
Logical Workspace history
  events.sequence_nr: 1..N
  event_segments: segment 0, segment 1, ...
  snapshots/latest + snapshot_history

Hot actor hydration
  latest snapshot
  + tail events after snapshot_sequence_nr
  <= MAX_EVENTS_SINCE_SNAPSHOT

PawFS hot writes
  ArtifactBatch.Apply
    -> Directory/Create/AddChild
    -> File/Create + $value PUT
    -> FileVersion/Create
    -> WorkspaceUsageBucket.ApplyDelta
    -> ArtifactBatch.Complete

Workspace
  create/freeze/archive/policy metadata only
```
