# PawFS Context Read Plane Batch Read

Date: 2026-04-23

## What changed

- `llm_caller` now calls `POST /api/files/read-text-batch` when resolving multiple file-backed context refs.
- Batch reads fall back to the existing single-file `$value` path if the batch API is unavailable.
- Context rendering moved behind a pure helper so message reconstruction is testable without a live WASM host.

## Verification

### Unit tests

Command:

```bash
cargo test --manifest-path os-apps/paw-agent/wasm/llm_caller/Cargo.toml -- --nocapture
```

Result:

- passed
- includes:
  - `render_context_refs_uses_loaded_file_content_and_inline_fallbacks`
  - `batch_text_file_read_response_deserializes_found_and_missing_items`

### WASM build

Command:

```bash
cargo build --target wasm32-unknown-unknown --release --manifest-path os-apps/paw-agent/wasm/llm_caller/Cargo.toml
```

Result:

- passed
- produced a release `llm_caller` WASM artifact with the batch-read integration compiled in

## Cross-repo note

- Full local end-to-end server verification across `openpaw` + the modified `temper` worktree was not run in one process because `openpaw` still depends on Temper via the git `main` source, not the local worktree path.
- The contract was verified on both sides independently:
  - Temper route + blob/projection behavior via server tests
  - OpenPaw batch client + context rendering via unit tests and WASM build

