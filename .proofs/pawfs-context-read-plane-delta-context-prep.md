# Proof: Delta Context Preparation

Date: 2026-04-23
Worktree: `/Users/seshendranalla/Development/openpaw-worktrees/pawfs-context-read-plane`

## Goal

Reduce `PreparingContext` work by reusing the previously prepared context artifact when:

- the current session leaf descends from the previously prepared leaf
- the workspace/session identity is unchanged
- the prune window is unchanged
- the delta does not include a compaction entry

Fallback must remain a full rebuild whenever those conditions are not true.

## Red -> Green

Added failing tests first in `llm_caller` for:

- append-only reuse on a descendant leaf
- rebuild fallback when compaction enters the delta
- rebuild fallback when the prune window changes

Also added `session-tree-lib` coverage for:

- ancestry-aware delta extraction
- divergence fallback
- compaction detection inside the delta chain

## Implementation

### Session tree delta support

File: `os-apps/paw-agent/wasm/session-tree-lib/src/lib.rs`

- Added `ContextRefDelta`
- Added `SessionTree::build_context_refs_since(leaf_id, after_entry_id)`
- Marks `includes_compaction` when the delta chain crosses a compaction entry
- Returns `None` when the previous prepared leaf is not an ancestor of the current leaf

### Context preparer reuse

File: `os-apps/paw-agent/wasm/llm_caller/src/lib.rs`

- Added `PreparedContextReuse` decision enum
- Added `try_read_existing_prepared_context_artifact(...)`
- Added `try_reuse_prepared_context(...)`
- `run_context_preparer()` now:
  - reads the prior prepared artifact when session-tree mode is active
  - parses the current session tree
  - reuses the old prepared messages plus only the delta when valid
  - logs a reuse miss and falls back to full rebuild when invalid
- Stored `prune_tool_results_after_turns` inside `PreparedContextArtifact` with serde defaulting for backward compatibility

## Verification

Focused tests:

- `cargo test --manifest-path os-apps/paw-agent/wasm/session-tree-lib/Cargo.toml -- --nocapture`
- `cargo test --manifest-path os-apps/paw-agent/wasm/llm_caller/Cargo.toml try_reuse_prepared_context -- --nocapture`

Broader tests:

- `cargo test --manifest-path os-apps/paw-agent/wasm/llm_caller/Cargo.toml -- --nocapture`
- `cargo test -p temperpaw --test session_turn_architecture -- --nocapture`
- `cargo test -p temperpaw --test session_lifecycle_and_config -- --nocapture`

Build:

- `cargo build --target wasm32-unknown-unknown --release --manifest-path os-apps/paw-agent/wasm/llm_caller/Cargo.toml`

All passed.

## Limits of this slice

This does **not** yet make context preparation fully incremental.

Still true today:

- we still read and parse the full session JSONL file
- we still rebuild fully when the reuse guardrails fail
- session entries still reference file-backed content by file ID, not immutable file-version/content-hash IDs

This slice specifically removes repeated full content re-resolution when the leaf advances linearly and the prepared artifact is still valid.
