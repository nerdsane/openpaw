# PERF-028 Local Proof: Acknowledged SessionEntry Create Verification

Date: 2026-05-19

Worktree: `/Users/seshendranalla/Development/temperpaw-worktrees/session-entry-ack-verify-20260519`

Branch: `codex/session-entry-ack-verify-20260519`

Baseline production version: `8dcbe10c8b4b257e2e968c7662ab1e7a16e74cb2`

Baseline Datadog trace: `https://app.datadoghq.com/apm/trace/958e12b4dd7faf4030bdc68bf4a48fdf`

## Decision

ADR: `os-apps/paw-agent/adrs/017-session-entry-acknowledged-create.md`

The normal hot path now accepts a `SessionEntry` create only when the returned
OData entity state proves the requested `SessionId` and `EntryId`. The old
immediate filtered read-back remains available through
`session_entry_create_verify_readback=true`.

## Local Checks

- Red test first: `cargo test session_entry_create_ack -- --nocapture` failed
  before the ack parser existed.
- `cargo test` in `os-apps/paw-agent/wasm/wasm-helpers`: 34/34 passed.
- `cargo test` in `os-apps/paw-agent/wasm/provider_response_applier`: 13/13 passed.
- `cargo build --target wasm32-unknown-unknown --release` in
  `provider_response_applier`: passed.
- `cargo test -p temperpaw --test session_turn_architecture -- --nocapture`:
  21/21 passed.
- `cargo test -p temperpaw --test session_lifecycle_and_config -- --nocapture`:
  6/6 passed.
- `cargo test -p temperpaw`: passed.
- `cargo fmt --all -- --check`: passed.
- `cargo clippy --locked -p temperpaw --all-targets -- -D warnings`: passed.
- `git diff --check`: passed.

## Remaining Gates

- Commit, push, PR, PR CI.
- Merge, main CI, Docker image, Railway deploy.
- Live production Session proof with independent `SessionEntries` read-back.
- Datadog fixed-version after comparison against the `sha-8dcbe10c` baseline.
