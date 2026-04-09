# ADR-0020: File-Backed Storage for Document-Sized App Content

## Status

Accepted

## Context

OpenPaw apps often generate or manage content that is larger than normal entity metadata:

- markdown pages
- long analyses
- fetched documents
- transcripts
- rendered artifacts
- long LLM outputs

Temper now preserves oversized field values through blob-backed overflow references instead of silently truncating them. That fixes the survivability problem, but it does not make large inline entity fields the right storage model for application design.

Inline entity fields are still the wrong abstraction for durable document artifacts because:

- they blur metadata and content storage
- they make schemas harder to reason about
- they invite accidental use of entity state as a document store
- they complicate evolution when content later needs versioning, download, or reuse

## Decision

OpenPaw apps must store document-sized artifacts in `Files` and reference them from entities using `content_file_id` or another explicit `*FileId` field.

Inline string fields remain appropriate for:

- names
- titles
- descriptions
- comments
- bounded notes
- short prompts or summaries

Document-sized content includes:

- markdown pages
- reports
- compiled analyses
- fetched raw source text
- transcripts
- HTML or rendered output
- large JSON artifacts
- long model outputs that must be read back in full

## Guidance

When designing an app:

1. keep entity fields focused on lifecycle state and metadata
2. create a `Files` entity for the actual content
3. upload bytes through `Files('{id}')/$value` or `temper.write(...)`
4. persist only the file id on the workflow/document entity
5. read the artifact back through the file id

If an inline field starts life as a note but later becomes document-sized, migrate it to a file-backed field rather than relying on Temper's overflow behavior.

## Consequences

### Positive

- app schemas stay clear about what is metadata versus artifact content
- large outputs remain easy to download, reuse, and evolve
- apps use TemperFS intentionally instead of by accident
- platform overflow handling remains defense in depth rather than the primary storage strategy

### Negative

- app authors must add one more step to create/update flows
- some existing apps may need migration when bounded notes turn into large artifacts

## Notes

The repository audit in [docs/os-app-content-storage-audit-2026-04-08.md](/Users/seshendranalla/Development/openpaw-codex/docs/os-app-content-storage-audit-2026-04-08.md) records the current state of OpenPaw apps as of 2026-04-08.
