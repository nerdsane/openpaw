# Proof Report: 080 - Infinite Logical History With Bounded Actors

## Date

2026-05-28

## Branch / Commit

- Temper: `codex/segmented-event-history-current-main` at `d15c614ee04613a9ccb9b361bd96dc265a53032c`
- TemperPaw: `codex/infinite-history-bounded-actors-main`, local working tree pending final commit/deploy

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
- Added ADRs for segmented platform history and PawFS hot-path ownership.

## Local Verification Results

| Step | Expected | Actual | Status |
|------|----------|--------|--------|
| Temper pre-push suite | All platform gates pass | rustfmt, clippy, readability, full tests, and doctests passed | PASS |
| `cargo fmt --check` | Formatting clean | Passed | PASS |
| `cargo test -p temperpaw` | TemperPaw tests pass | 225 tests passed across unit/integration suites | PASS |
| `cargo test -p temperpaw --test paw_fs_hot_path` | PawFS contracts pass | 12 tests passed | PASS |
| `cargo test -p temperpaw --test temperpaw_identity_contract temperpaw_runtime_uses_bounded_large_stack_workers_for_wasm_loopback_io` | Runtime stack contract passes | Passed | PASS |
| `cargo test --manifest-path os-apps/paw-agent/wasm/monty_repl/Cargo.toml --lib` | Agent-facing PawFS tools pass | 69 tests passed | PASS |
| `os-apps/paw-fs/wasm/artifact_batch_apply/build.sh` | WASM builds | Built `artifact_batch_apply.wasm` | PASS |
| `os-apps/paw-fs/wasm/workspace_fs/build.sh` | WASM builds | Built `workspace_fs.wasm` | PASS |
| Local ready check | Server ready | `/readyz` returned `{"status":"ready"}` | PASS |
| ArtifactBatch apply | Batch completes | `ArtifactBatch` `ab-infinite-history-e2e-v8` reached `Completed` | PASS |
| File readback | Three files readable | 37-byte markdown, 28-byte JSON, 15-byte text read back exactly | PASS |
| Usage accounting | Usage bucket records one bounded delta | `WorkspaceUsageBucket.ApplyDelta` recorded `bytes_delta=80`, `file_delta=3` | PASS |
| Workspace hot path | No Workspace file IO events | No `MkDir`, `CreateFile`, `ResolvePath`, `ListDir`, `IncrementUsage`, or `IncrementFileCount` events | PASS |
| Workspace event count | Workspace unchanged after batch | Target Workspace has only `Created` | PASS |
| WASM hash reconcile | Observed hash equals packaged hash | `artifact_batch_apply` hash `b902cc83ceb17b1da8c52c064f565031a765a8b6c25304e86116d195af7e18d4` matched packaged file | PASS |

## Temper Verification

- `cargo test -p temper-server event_budget --lib`
- `cargo test -p temper-platform find_wasm_modules --lib`
- `cargo test -p temper-platform test_os_app_document_bootstrap_does_not_charge_workspace_file_count --lib`
- `cargo test -p temper-store-postgres migration --lib`
- `cargo test -p temper-store-turso event_history_schema_declares_segments_and_snapshot_history --lib`
- `cargo test -p temper-store-sim snapshot_save_records_history_and_rotates_segments --lib`
- `cargo test -p temper-store-redis --lib`
- `cargo check -p temper-server -p temper-platform -p temper-store-redis -p temper-store-turso -p temper-store-postgres -p temper-store-sim`
- Full Temper pre-push gate on `d15c614ee04613a9ccb9b361bd96dc265a53032c`: PASS

## Local E2E Artifacts

```text
Local DB: /tmp/temperpaw-infinite-history-e2e-v8.db
Workspace: ws-infinite-history-e2e-v8
ArtifactBatch: ab-infinite-history-e2e-v8
UsageBucket: en-019e6fa6-8e88-77e3-96ef-8cb6b9ba951b

Readbacks:
- /katagami/e2e-v8/language.md: 37 bytes
- /katagami/e2e-v8/tokens.json: 28 bytes
- /katagami/e2e-v8/review.txt: 15 bytes

WASM module:
- module_name=artifact_batch_apply
- sha256_hash=b902cc83ceb17b1da8c52c064f565031a765a8b6c25304e86116d195af7e18d4
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
238

snapshot_history rows:
0

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

## Issues Found And Fixed During E2E

- Initial local ArtifactBatch invocation crashed with a Tokio worker stack overflow during WASM loopback OData. Fixed by using an explicit bounded 16 MiB Tokio worker stack in TemperPaw.
- ArtifactBatch agents could apply workflow actions but could not read/list the direct PawFS entities they needed to query. Fixed Cedar policies for Directory, ArtifactBatch, and WorkspaceUsageBucket.
- The proof harness originally assumed top-level `Id`; OData entity read models expose `entity_id` and nested `fields.Id`, so the verifier now checks those shapes.

## Deployment Verification

Pending merge/deploy.

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
