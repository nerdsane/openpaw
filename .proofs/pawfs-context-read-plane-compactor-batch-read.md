# Proof: Compactor Batch Read Plane

Date: 2026-04-23
Worktree: `/Users/seshendranalla/Development/openpaw-worktrees/pawfs-context-read-plane`

## Goal

Move the context compactor off per-file `$value` reads and onto the same
projection-backed batch file read plane already used by context preparation.

## Red -> Green

Added failing tests first for:

- parsing the batch text-file response in `wasm-helpers`
- rendering compactor context refs with file-backed content and inline fallback

## Implementation

### Shared helper

File: `os-apps/paw-agent/wasm/wasm-helpers/src/lib.rs`

- Added `BatchTextFileReadItem`
- Added `parse_batch_text_file_read_response(...)`
- Added `read_text_files_batch(...)`

This centralizes the client-side call to:

- `POST /api/files/read-text-batch`

so the agent WASM modules can share one implementation.

### Context compactor

File: `os-apps/paw-agent/wasm/context_compactor/src/lib.rs`

- Added host stubs so the crate can be unit tested on the native host
- Switched `resolve_context_refs_for_compaction(...)` to:
  - collect unique file IDs
  - batch-read when there is more than one file-backed ref
  - fall back to single-file reads if the batch API is unavailable
- Added pure helper `render_context_refs_for_compaction(...)`

### LLM caller cleanup

File: `os-apps/paw-agent/wasm/llm_caller/src/lib.rs`

- Replaced the local batch-read client with the shared `wasm_helpers::read_text_files_batch(...)`

## Verification

Focused tests:

- `cargo test --manifest-path os-apps/paw-agent/wasm/wasm-helpers/Cargo.toml -- --nocapture`
- `cargo test --manifest-path os-apps/paw-agent/wasm/context_compactor/Cargo.toml -- --nocapture`
- `cargo test --manifest-path os-apps/paw-agent/wasm/llm_caller/Cargo.toml -- --nocapture`

WASM builds:

- `cargo build --target wasm32-unknown-unknown --release --manifest-path os-apps/paw-agent/wasm/context_compactor/Cargo.toml`
- `cargo build --target wasm32-unknown-unknown --release --manifest-path os-apps/paw-agent/wasm/llm_caller/Cargo.toml`

Higher-level checks:

- `cargo test -p temperpaw --test session_turn_architecture -- --nocapture`
- `cargo test -p temperpaw --test session_lifecycle_and_config -- --nocapture`

All passed.

## Limits

This slice does not yet move session entries to immutable `FileVersion` or
`content_hash` references. The read plane is now shared across preparation and
compaction, but the session tree still stores mutable `content_file_id` refs.
