# Spec Persistence Fast Path Local E2E

Date: 2026-04-28

## Scope

Temper worktree: `/Users/seshendranalla/Development/temper-worktrees/spec-upsert-fast`
OpenPaw worktree: `/Users/seshendranalla/Development/openpaw-worktrees/spec-upsert-fast-temper`

This proof covers the OpenPaw integration of Temper PR 198, which makes spec persistence idempotent:

- unchanged app specs bypass Turso write transactions;
- unchanged verification rows do not churn `updated_at`;
- already committed specs are not rewritten;
- OpenPaw passes the loaded verification cache into Temper bootstrap persistence.

## Red Test

Command:

```sh
cargo check -p temperpaw -p temperpaw-cli -p paw-transport
```

Initial result before the OpenPaw wiring fix: failed because `persist_system_verification` and
`persist_agent_verification` required the new verification-cache argument from Temper.

## Automated Verification

Commands completed successfully after implementation:

```sh
cargo check -p temperpaw -p temperpaw-cli -p paw-transport
cargo test -p temperpaw --bins -- --nocapture
cargo build -p temperpaw --bin temperpaw-server
```

The Temper implementation was separately verified before this OpenPaw lock update:

```sh
cargo test -p temper-store-turso --lib -- --nocapture
cargo test -p temper-platform test_hashes_requiring_persistence_skip_cached_verified_specs -- --nocapture
cargo test -p temper-cli -- --nocapture
cargo check -p temper-cli -p temper-platform -p temper-store-turso
cargo build -p temper-cli
```

## Local E2E

Built the required app WASM modules locally with the nightly toolchain:

```sh
RUSTUP_TOOLCHAIN=nightly bash os-apps/paw-fs/wasm/blob_adapter/build.sh
RUSTUP_TOOLCHAIN=nightly bash os-apps/paw-fs/wasm/workspace_fs/build.sh
RUSTUP_TOOLCHAIN=nightly bash os-apps/paw-agent/wasm/build.sh
RUSTUP_TOOLCHAIN=nightly bash os-apps/paw-research/wasm/build.sh
RUSTUP_TOOLCHAIN=nightly bash os-apps/paw-channels/wasm/build.sh
```

Booted `temperpaw-server` against an isolated Turso file DB:

```sh
HOME=/tmp/openpaw-spec-upsert-e2e-home
PORT=4492
TURSO_URL=file:/tmp/openpaw-spec-upsert-e2e.db
TEMPER_API_KEY=local-e2e-key
TEMPERPAW_WASM_STARTUP_POLICY=load-only
TEMPERPAW_QUERY_PROJECTION_BACKFILL_ON_STARTUP=0
OTEL_ENABLED=false
RUSTUP_TOOLCHAIN=nightly
RUST_LOG=warn,temperpaw_server::startup=info
RUST_MIN_STACK=16777216
```

First boot reached readiness and persisted all expected specs:

```text
readyz: 200
phase_6b_os_app_reconcile: 3496ms
startup time to ready: 3883ms
default|32|32|32
temper-system|13|13|13
```

Restarted against the same DB and home directory. The second boot restored 45 specs, found no
cold or changed startup apps, and skipped reconcile for all six startup apps:

```text
readyz: 200
phase_6a_pre_recovery: 104ms
runtime recovery scoped surface: ready=6 cold=0 needs_reconcile=0
Skipped unchanged OS app: paw-fs
Skipped unchanged OS app: katagami-commons
Skipped unchanged OS app: paw-agent
Skipped unchanged OS app: paw-research
Skipped unchanged OS app: katagami-curation
Skipped unchanged OS app: paw-channels
phase_6b_os_app_reconcile: 1279ms
startup time to ready: 1803ms
default|32|32|32
temper-system|13|13|13
```

The before/after `specs` snapshot included tenant, entity type, `updated_at`, version, verification
state, commit state, and content hash. The warm restart produced an empty diff:

```sh
diff -u /tmp/openpaw-spec-upsert-before.tsv /tmp/openpaw-spec-upsert-after.tsv
```

## Result

Local E2E confirms OpenPaw consumes the Temper idempotent spec persistence API, reaches readiness
on a clean boot, and performs a warm restart without rewriting unchanged spec rows.
