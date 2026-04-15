# ADR-0032: TemperFS Agent Operations

**Status:** Accepted
**Date:** 2026-04-15
**Related:** ADR-0008 (Monty REPL agent tools), ADR-0020 (File-backed document content)

## Context

Paw agents interact with TemperFS through two primitives: `temper.read(path)` and `temper.write(path, content)`. These map to FUSE `open+read` and `creat+write` respectively, going through the `paw-fs` Workspace entity's state machine actions (`CreateFile`, `ResolvePath`, `MkDir`).

This limited surface caused a real failure: a user pasted a large document into a Paw conversation. When the context window grew large, `context_compactor` summarized older messages — but its prompt is designed for agent work conversations ("what was tried, what worked, what failed"), not document preservation. The summary discarded the document content. The agent then had no tool to search its own conversation history, even though the full text still exists in the session tree JSONL (session tree is append-only; compaction adds summary entries but never deletes).

More broadly, Paw agents lack filesystem exploration capabilities that Claude Code has on a local filesystem: listing directories, searching file contents, finding files by pattern, editing files in place, and partial reads. This forces agents to maintain their own indices or read entire files when they only need a few lines.

The `paw-fs` app also lacked the POSIX `rename` syscall, the only common FUSE operation not yet implemented.

## Decision

### 1. Rename added to paw-fs (FUSE-mapped)

`paw-fs` operations must map 1:1 to real POSIX/FUSE syscalls so a FUSE filesystem can be layered on top. The `Rename` action was the only missing syscall. It is added as a Workspace bound action (`Rename`) with a WASM integration (`fs_rename`) in `workspace_fs`, following the existing pattern of `MkDir`, `CreateFile`, `ResolvePath`, `ListDir`, `DeleteFile`.

The implementation normalizes both paths, resolves the source file, creates intermediate directories for the target path via `ensure_dirs`, PATCHes the File entity (Name, Path, DirectoryId), and updates parent directory children.

### 2. Agent-layer utilities built on paw-fs primitives

`grep`, `glob`, `ls`, `edit`, `search_history`, and `read` with offset/limit are **not** FUSE operations — they are userspace utilities. They are implemented in the agent layer (`monty_repl/entity_ops.rs`) using existing paw-fs actions:

- **`temper.ls(path)`** — wraps `Workspace.ListDir`, returns parsed JSON listing
- **`temper.read(path, opts)`** — extended with `offset`/`limit` (0-indexed line numbers); downloads full file and slices in WASM (Range headers are a future Temper optimization)
- **`temper.edit(path, old_string, new_string)`** — reads file, performs `replacen(old, new, 1)`, writes back
- **`temper.rename(old_path, new_path)`** — wraps the new `Workspace.Rename` action
- **`temper.grep(pattern, path, opts)`** — recursively lists files, reads each, searches with `str::contains` (case-insensitive option)
- **`temper.glob(pattern, path)`** — recursively lists files, matches paths against glob pattern
- **`temper.search_history(pattern)`** — reads full session JSONL, iterates ALL entries (including pre-compaction), searches inline content and content_file_id references

### 3. No ReadAt in paw-fs

Agent-layer `read()` downloads the full file and slices by line number. This is correct for the current architecture where files are small (conversation content, code). HTTP Range headers would be a Temper platform optimization — not an OpenPaw concern.

### 4. search_history reads the full session tree

`search_history` uses `SessionTree::from_jsonl()` then iterates `tree.entry_ids()`, which returns ALL entries in insertion order — including those before compaction boundaries. This is the key fix: `build_context_refs()` stops at Compaction entries (by design, for LLM context assembly), but `entry_ids()` walks the complete tree.

Content larger than 4096 bytes is stored as separate TemperFS files referenced by `content_file_id`. `search_history` fetches these on demand, capped at 20 file fetches per search to bound latency.

### 5. No regex dependency

Pattern matching uses `str::contains` (grep) and a simple inline glob matcher (glob). This keeps the WASM binary small — the `regex` crate adds significant code size. If regex support is needed later, it can be added behind a feature flag.

### 6. Recursive listing has N+1 query cost

`grep` and `glob` both use a `list_dir_recursive` helper that makes one `ListDir` call per directory level. This is mitigated by depth cap (5 levels) and total file count cap (500 files). A future optimization could add a recursive listing action to `paw-fs`, but the current approach is correct and sufficient for typical workspace sizes.

## Consequences

- Agents gain full filesystem exploration capability, matching Claude Code's local tools
- Compaction no longer causes permanent data loss — agents can search their full conversation history
- The new tools are available in plan mode (read-only subset: `ls`, `grep`, `glob`, `search_history`) and execute mode (all tools including `edit`, `rename`)
- `paw-fs` now implements all common POSIX filesystem operations: `mkdir`, `creat`, `open+read`, `write`, `stat`, `readdir`, `unlink`, `rename`
- No new WASM crate dependencies; all pattern matching is inline
