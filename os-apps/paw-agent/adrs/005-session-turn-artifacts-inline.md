# ADR-005: Session Turn Artifacts Use Session Fields

- Status: Accepted
- Date: 2026-04-26

## Context

The staged Session loop produced two turn-local artifacts:

- `PreparedContextArtifact`, written by `context_preparer` and read by `provider_caller`
- `ProviderResponseArtifact`, written by `provider_caller` and read by `provider_response_applier`

These artifacts were stored through PawFS Files. For a normal chat turn this meant a tiny handoff payload synchronously paid the full governed file lifecycle: blob write, `File.StreamUpdated`, `FileVersion.Create`, `File.RecordVersion`, old-version supersede, workspace usage, and projections. Production traces showed this adding multi-second delays to simple Discord replies.

Temper now has blob-backed field overflow for large string fields. That gives us a better hot-state path without losing durability.

## Decision

Store turn-local Session handoff artifacts directly on the Session:

- `prepared_context_inline_json`
- `provider_response_inline_json`

The existing file-id fields remain as compatibility fallbacks for sessions already in flight or older rows. New turns emit inline JSON and leave the file-id empty.

The artifacts are operational control-plane state, not user-authored governed files. PawFS remains the right home for exported transcripts, published artifacts, attachments, and reviewable files, but not for every internal turn handoff.

## Consequences

- Provider turns no longer synchronously create/supersede FileVersions just to pass data between staged WASM modules.
- Large artifacts still use Temper's content-addressed field-overflow blob path.
- Existing sessions with artifact file IDs continue to work.
- The latest prepared context remains available for delta reuse and continuation.

## Follow-Ups

- Move system prompt cache off PawFS or make it digest-addressed and startup-independent.
- Make OS app docs/skills/ADR bootstrap digest-idempotent so deploy readiness is not blocked on repeated PawFS writes.
