# ADR-006: Keep Fresh Session Hot Path Out Of PawFS

- Status: Accepted
- Date: 2026-04-26

## Context

ADR-005 moved prepared/provider turn artifacts from PawFS files to Session fields,
but fresh sessions still paid PawFS costs before the first provider call:

- `workspace_provisioner` created empty conversation and manifest Files and wrote
  `$value` content to both.
- `context_preparer` wrote a `system-prompt-cache.txt` File on prompt cache miss.

Those writes are governed file lifecycle events. They are useful for user-visible
artifacts, but they are too expensive for transient operational state.

## Decision

Fresh sessions use Temper-native hot state by default:

- Session history starts as `SessionEntry` entities.
- Conversation and manifest PawFS files are not created unless a legacy opt-in is
  supplied with `bootstrap_temperfs_session_files=true`.
- System prompts are cached through the inline `PreparedContextArtifact`; the
  prompt-cache file path is opt-in via `system_prompt_cache_file_enabled=true`.

PawFS remains available for durable files, exported transcripts, user artifacts,
and explicit `temper.write` operations. It is no longer part of the required
first-turn Session control path.

## Consequences

- First-turn provisioning no longer waits for empty File `$value` writes,
  `FileVersion` creation, workspace usage updates, and projections.
- Context preparation no longer waits for a system prompt cache File write.
- Existing sessions and legacy file-backed sessions continue to read their
  existing file IDs.
- Agents still get durable session history through `SessionEntry` entities, while
  PawFS is reserved for actual file semantics.
