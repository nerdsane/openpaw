---
name: agent-storage-taxonomy
description: Choose between Temper entities, blob-backed fields, PawFS, and sandbox files for agent work
---

# Agent Storage Taxonomy

Use this when deciding where agent/session/workflow data belongs.

## Hot Operational State

Use Temper entities and state transitions.

Examples:

- session turns, tool calls, tool results, compactions, and steering messages
- parent/child session links
- curation/review/job progress
- memory records and resumability checkpoints
- anything another workflow must query, react to, repair, or replay

For large operational fields, keep the entity as the source of truth and rely on
Temper's blob-ref overflow. The entity is the control plane; the object/blob
store is the data plane for bytes.

## PawFS

Use `temper.write`/`temper.read` for governed artifacts.

Good uses:

- published specs and docs
- exported transcripts and reviewable snapshots
- generated media or HTML deliverables
- reusable instruction files and skills
- datasets or reports that should have file provenance

Bad use:

- every assistant turn
- every tool result
- job progress markers
- scratch notes that only matter during the current run

## Sandbox Files

Use sandbox `read`, `write`, `edit`, and `bash` for scratch work, code editing,
build outputs, downloads, and experiments. Promote only the useful final result
into Temper entities or PawFS.

## Rule Of Thumb

If the thing changes the state of the system, create or update a Temper entity.
If the thing is an artifact someone would open, version, review, or publish, use
PawFS. If the thing is temporary work material, keep it in the sandbox.
