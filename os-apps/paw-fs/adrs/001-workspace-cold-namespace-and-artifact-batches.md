# ADR-001: Workspace As Cold Namespace And Artifact Batches

## Status

Accepted

## Context

PawFS originally routed routine path operations through `Workspace` actions such as `MkDir`, `CreateFile`, `ResolvePath`, and `ListDir`. That was convenient for a FUSE-like compatibility layer, but it charged every file write and lookup to the `Workspace` actor. Katagami-style synthesis turns one logical artifact set into many path operations, exhausting workspace actor history before the workspace itself has changed meaningfully.

## Decision

`Workspace` is namespace lifecycle and policy, not the hot file IO router.

- Agent hot-path tools call `Directory`, `File`, and `FileVersion` entities directly.
- Workspace-bound filesystem actions remain available as legacy/FUSE compatibility only.
- `File.StreamUpdated` no longer triggers `Workspace.IncrementUsage`.
- `workspace_fs` no longer increments/decrements workspace file count on hot create/delete paths.
- Usage deltas are recorded in bounded `WorkspaceUsageBucket` entities keyed by workspace and coarse batch/period.
- Multi-file artifact writes use `ArtifactBatch` with `Submit -> Apply -> RecordFileApplied -> Complete | Fail`.
- `temper.write_many(files, opts)` submits an artifact batch and applies files directly without mutating the Workspace actor for each path operation.

## Consequences

Routine file IO scales with file and directory entities instead of a single workspace actor. Workspace history reflects workspace lifecycle and policy changes, while auditability remains entity-first through file, version, batch, and usage-bucket histories.

Legacy FUSE-style workspace actions can continue to serve compatibility clients, but they are explicitly not the agent hot path.
