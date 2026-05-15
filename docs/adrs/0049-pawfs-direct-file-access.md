# ADR 0049: PawFS Direct File Access

Date: 2026-05-14

Status: Accepted

## Context

PawFS agent tools were using the `Workspace` actor as a syscall/result bus for normal path operations. A single artifact workspace could accumulate thousands of `MkDir`, `CreateFile`, `ResolvePath`, `ListDir`, and `Rename` events even though the durable resources being operated on were `Directory` and `File` entities.

Temper's 10,000-event actor cap is intentional bounded-execution behavior. The failure was not the cap; the failure was routing every agent filesystem operation through one hot actor.

## Decision

`temper.read`, `temper.write`, `temper.ls`, `temper.grep`, `temper.glob`, `temper.edit`, and `temper.rename` resolve PawFS paths directly through `Directory` and `File` metadata:

- directory lookup/create uses `Directories?$filter=Path eq ... and WorkspaceId eq ...`
- file lookup/create uses `Files?$filter=Path eq ... and WorkspaceId eq ...`
- file bytes are read and written through `Files('{file_id}')/$value`
- parent directory child counts are updated through `Directory.AddChild` / `Directory.RemoveChild`

The `Workspace` entity remains the lifecycle/config object. Workspace-bound filesystem actions stay in `workspace.ioa.toml` as legacy FUSE compatibility only; agent tools must not call them.

`Workspace.used_bytes` and `Workspace.file_count` are legacy, non-authoritative counters until a separate quota/accounting design exists. Agent-path file writes must not emit Workspace usage/file-count events.

Temper collection reads that resolve exact `Directory` and `File` metadata must not hydrate the capped target actor after SQL filter push-down. Pushed-down OData filters should materialize matching rows from the durable entity catalog when present, falling back to actors only for missing catalog rows.

## Consequences

The hot PawFS path no longer consumes Workspace actor event budget for read/list/grep/glob/write/edit/rename. Existing Workspace filesystem actions remain available for old FUSE integrations, but they are not the agent path.

Quota admission is explicitly out of scope for this change. Any future quota design must not depend on forcing all path operations through the Workspace actor.

## Verification

Regression tests assert the Monty PawFS tool implementation contains no Workspace-bound filesystem action calls. Temper tests assert pushed-down filters prefer catalog materialization. End-to-end verification must cover workspace creation, file write, read, list, grep, glob, edit, rename, and operation against a Workspace actor that has already exhausted its event budget.
