# No DB Blob Storage Local E2E

Date: 2026-04-28

## Scope

Temper worktree: `/Users/seshendranalla/Development/temper-worktrees/no-db-blobs`
OpenPaw worktree: `/Users/seshendranalla/Development/openpaw-worktrees/no-db-blobs`

This proof covers the change that routes blob bytes out of Turso/SQL tables:

- WASM module bytes are stored in the object store at `wasm-modules/{sha256}`.
- Local TemperFS file bytes are stored in the object store at `temper-fs/{content_hash}`.
- SQL tables retain metadata only.
- Legacy Turso blob reads remain available as a read-only fallback.

## Red Test

Command:

```sh
cargo test -p temper-store-turso upsert_wasm_module_stores_metadata_only_without_db_blob -- --nocapture
```

Initial result before implementation: failed with `new WASM artifacts must not create Turso blob rows`.

## Automated Verification

Commands completed successfully after implementation:

```sh
cargo test -p temper-store-turso upsert_wasm_module_stores_metadata_only_without_db_blob -- --nocapture
cargo test -p temper-server file_blob_key -- --nocapture
cargo test -p temper-server blob -- --nocapture
cargo test -p temper-server persisted_wasm_modules -- --nocapture
cargo test -p temper-store-turso wasm -- --nocapture
cargo check -p temper-server
cargo test -p temper-platform os_apps -- --nocapture
```

OpenPaw compile with local Temper patches completed successfully:

```sh
cargo check -p temperpaw \
  --config 'patch."https://github.com/nerdsane/temper.git".temper-platform.path="/Users/seshendranalla/Development/temper-worktrees/no-db-blobs/crates/temper-platform"' \
  --config 'patch."https://github.com/nerdsane/temper.git".temper-observe.path="/Users/seshendranalla/Development/temper-worktrees/no-db-blobs/crates/temper-observe"' \
  --config 'patch."https://github.com/nerdsane/temper.git".temper-server.path="/Users/seshendranalla/Development/temper-worktrees/no-db-blobs/crates/temper-server"' \
  --config 'patch."https://github.com/nerdsane/temper.git".temper-runtime.path="/Users/seshendranalla/Development/temper-worktrees/no-db-blobs/crates/temper-runtime"' \
  --config 'patch."https://github.com/nerdsane/temper.git".temper-jit.path="/Users/seshendranalla/Development/temper-worktrees/no-db-blobs/crates/temper-jit"' \
  --config 'patch."https://github.com/nerdsane/temper.git".temper-authz.path="/Users/seshendranalla/Development/temper-worktrees/no-db-blobs/crates/temper-authz"' \
  --config 'patch."https://github.com/nerdsane/temper.git".temper-store-turso.path="/Users/seshendranalla/Development/temper-worktrees/no-db-blobs/crates/temper-store-turso"'
```

## Local E2E

Built required app WASM modules locally, then booted `temperpaw-server` with an isolated home:

```sh
HOME=/tmp/openpaw-no-db-blobs-e2e-home
PORT=4491
TEMPERPAW_WASM_STARTUP_POLICY=warn
TEMPERPAW_QUERY_PROJECTION_BACKFILL_ON_STARTUP=0
OTEL_ENABLED=false
RUST_LOG=warn
RUST_MIN_STACK=16777216
```

Startup reached the running banner and readiness passed:

```text
healthz: 200
readyz: 200 {"status":"ready","healthz":"/healthz","discord":{"status":"disconnected","configured":false,"connected":false}}
```

Post-start object/DB checks:

```text
filesystem object count: 58
filesystem WASM object count: 31
SQL blobs rows: 0
wasm_modules rows: 31
inline SQL WASM bytes: 0
wasm metadata size_bytes total: 15251400
```

Exercised a live OData TemperFS upload and read-back:

```text
file_id=fl-019dd491-eb3f-7a01-93ad-e2a96b54c3ac
read_back=no db blobs e2e
objects_before=58
objects_after=59
db_blob_rows=0
db_wasm_bytes=0
```

Restarted against the same data directory so persisted metadata-only WASM had to reload with object-store bytes present. Readiness passed again:

```text
readyz: 200
objects=59
wasm_objects=31
db_blob_rows=0
db_wasm_inline_bytes=0
wasm_metadata_bytes=15251400
```

## Result

Local E2E confirms fresh startup, live TemperFS writes, and same-data restart all keep blob bytes outside SQL/Turso blob storage.
