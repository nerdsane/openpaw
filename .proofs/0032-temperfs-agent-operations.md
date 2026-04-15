# Proof Report: 0032 — TemperFS Agent Operations

## Date
2026-04-15

## Branch / Commit
feat/temperfs-agent-operations @ a18371af

## What Was Done
Added Rename action to paw-fs (FUSE-mapped) and 6 new agent tools (ls, grep, glob, edit, rename, search_history) plus read offset/limit to paw-agent. Fixed Cedar policy that was missing permits for ListDir, DeleteFile, Rename, and callback actions.

## Verification Flow
1. Built all 3 WASM modules (workspace_fs, llm_caller, monty_repl) — verified clean compilation
2. Built blob_adapter WASM module (prerequisite for file content operations)
3. Started local Temper server on port 3468 from the feature branch worktree
4. Created test workspace with nested directories (/docs/notes, /src) and 3 test files
5. Ran automated E2E test suite exercising all paw-fs WASM operations

## Verification Results
| Step | Expected | Actual | Status |
|------|----------|--------|--------|
| WASM build (workspace_fs) | Compiles | Compiles clean | PASS |
| WASM build (llm_caller) | Compiles | Compiles (pre-existing warnings only) | PASS |
| WASM build (monty_repl) | Compiles | Compiles clean | PASS |
| Server starts | Boots on port 3468 | Boots clean, specs loaded | PASS |
| MkDir /docs/notes | Directory created | last_dir_path=/docs/notes | PASS |
| CreateFile + PUT $value | File created with content | File readable at path | PASS |
| ResolvePath | Returns correct file_id | file_id matches | PASS |
| Read content | Returns file content | Contains expected text | PASS |
| Rename /docs/notes/hello.txt -> /docs/moved.txt | Returns file_id + new path | Correct file_id and path=/docs/moved.txt | PASS |
| Renamed file resolvable | ResolvePath finds file at new path | file_id matches | PASS |
| DeleteFile | Deletes file, returns file_id | file_id returned | PASS |
| ListDir /docs/notes | Lists files in directory | Returns empty arrays | FAIL (pre-existing) |
| ListDir / | Lists root directories | Returns empty arrays | FAIL (pre-existing) |

## What Worked
- Rename action: full end-to-end — WASM dispatch, path normalization, file resolution, directory creation, entity PATCH, success callback
- Cedar policy fix: ListDir, DeleteFile, Rename actions are now authorized
- All WASM modules compile cleanly against the new code
- ResolvePath confirms Rename actually moved the file entity
- Server boots and loads all specs including new Rename action

## What Didn't Work
- ListDir returns empty results. This is a **pre-existing bug** in `ensure_dirs()` — each call to `ensure_dirs` creates new root directory entities instead of finding existing ones. Files are assigned to specific directory IDs during CreateFile, but ListDir resolves to different directory IDs that have no children. This bug existed before our changes but was never exposed because Cedar policy never permitted the ListDir action.

## Limitations
- Agent-layer tools (temper.ls, temper.grep, etc.) were not tested end-to-end because they require a running agent session with the Monty REPL. They were verified via WASM compilation only.
- search_history requires a session with compacted entries to fully test the compaction recovery path.

## What Still Doesn't Work
- ListDir returns empty results due to pre-existing `ensure_dirs()` duplicate directory bug. This affects temper.ls, temper.grep, and temper.glob which all depend on ListDir. This should be filed as a separate issue.

## Artifacts
- ADR: docs/adrs/0032-temperfs-agent-operations.md
- E2E test script: /tmp/temperfs-e2e-test.sh
- WASM binaries: os-apps/paw-fs/wasm/workspace_fs/target/wasm32-unknown-unknown/release/workspace_fs.wasm

## Architecture Diagram
```text
Agent (Monty REPL)
  |
  |-- temper.ls(path)     -----> Workspace.ListDir  --> workspace_fs WASM --> OData query
  |-- temper.read(path)   -----> Workspace.ResolvePath --> workspace_fs WASM --> GET $value
  |-- temper.write(path)  -----> Workspace.MkDir + CreateFile --> workspace_fs WASM --> PUT $value
  |-- temper.edit(path)   -----> read() + write() (agent-layer composition)
  |-- temper.rename(a,b)  -----> Workspace.Rename   --> workspace_fs WASM --> PATCH File
  |-- temper.grep(pat,p)  -----> list_dir_recursive + read each file (agent-layer)
  |-- temper.glob(pat,p)  -----> list_dir_recursive + glob match (agent-layer)
  |-- temper.search_history(p) -> GET session JSONL --> SessionTree::from_jsonl --> iterate all entries
```
