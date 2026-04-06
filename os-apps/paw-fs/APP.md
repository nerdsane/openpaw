# paw-fs

TemperFS — the platform file system. Manages workspaces, directories, and files with versioning, quota enforcement, and locking.

## Entity Types

### Workspace
Top-level container. Manages storage quota and usage tracking.

- **States**: Active <-> Frozen -> Archived
- **Key actions**: `Create` (name, quota_limit), `UpdateQuota`, `IncrementUsage`, `DecrementUsage`, `IncrementFileCount`, `DecrementFileCount`, `Freeze`, `Thaw`, `Archive`
- **Invariant**: `used_bytes <= quota_limit` while Active

### Directory
Hierarchical container within a workspace.

- **States**: Active -> Archived
- **Key actions**: `Create` (name, path, workspace_id), `AddChild`, `RemoveChild`, `Rename`, `Archive`
- **Invariant**: Can only archive when empty (`item_count <= 1`)

### File
Single file with content lifecycle. Content stored externally; `StreamUpdated` fires after upload succeeds.

- **States**: Created -> Ready <-> Locked -> Archived
- **Key actions**: `Create` (name, path, directory_id, workspace_id), `StreamUpdated` (content_hash, size_bytes), `Lock`, `Unlock`, `Archive`
- **Invariant**: Files in Ready or Locked state always have content

### FileVersion
Immutable record of a specific file version. Created on content upload, superseded when newer version replaces it.

- **States**: Current -> Superseded
- **Key actions**: `Create` (file_id, version_number, content_hash, size_bytes), `Supersede`

## Setup

No dependencies. This is a foundational app — other apps (paw-agent, paw-foresight) depend on it for file storage.
