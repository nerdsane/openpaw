# ADR-0040: Immutable Session Content References and Delta Context Preparation

**Status:** Accepted
**Date:** 2026-04-23
**Related:** ADR-0005 (Temper-Native Orchestration), ADR-0020 (File-Backed Storage for Document-Sized App Content), ADR-0022 (LLM Calling Infrastructure Optimizations), ADR-0034 (Bounded Session Context and LLM Turn Decomposition), `os-apps/paw-agent/wasm/session-tree-lib/src/lib.rs`, `os-apps/paw-agent/wasm/llm_caller/src/lib.rs`, `os-apps/paw-agent/wasm/context_compactor/src/lib.rs`

## Context

Session context preparation had two structural problems:

1. file-backed session content still mostly referenced mutable `content_file_id` heads
2. context preparation rebuilt too much state from scratch on every turn

That combination meant OpenPaw paid a high latency cost even when only a small delta had changed. It also made historical session content less stable than it should be, because a mutable file head is the wrong identity for archived turn content.

The platform work in Temper now gives us the missing primitive: explicit `FileVersion` lineage plus batch immutable reads. OpenPaw needs to consume that shape directly.

## Decision

### 1. Session-tree entries store immutable content identities

File-backed session entries now carry `content_file_version_id` in addition to `content_file_id`.

The rule is:

- `content_file_version_id` is the preferred identity for archived turn content
- `content_file_id` remains for compatibility and fallback

This applies to:

- assistant message externalization
- tool result externalization
- compaction artifacts
- steering artifacts
- workspace/session bootstrap content
- plan review feedback content

### 2. Context reads prefer immutable version batches

`llm_caller`, `context_compactor`, and other history readers now resolve file-backed content in this order:

1. batch immutable reads by `content_file_version_id`
2. fallback batch reads by current `content_file_id`
3. fallback single-file reads when the batch surface is unavailable

That makes current production behavior compatible with older session entries while letting new entries use stable immutable content.

### 3. Prepared context reuse is delta-first

`PreparedContextArtifact` remains the contract between context preparation and provider calling, but normal reuse is now incremental when the current leaf remains on the same ancestry chain.

The steady-state path is:

- load prior prepared artifact
- validate session/prune/config compatibility
- ensure the old leaf is an ancestor of the current leaf
- append only the new context refs

Full rebuild remains the repair path for divergence, compaction boundaries, or config drift.

### 4. Compaction remains a quality tool, not the latency fix

Compaction still manages relevance and token budget.

It is no longer the primary explanation for why context preparation stays responsive. The normal case should be fast because OpenPaw reuses prior prepared context and reads immutable content in batch, not because compaction happened to cut history recently.

## Consequences

### Positive

- Historical turn content points at immutable versions instead of mutable file heads.
- Context preparation cost scales with session delta more often than with full retained history.
- OpenPaw can use Temper's batch immutable read plane directly.
- Older sessions remain readable because `content_file_id` fallback stays in place.

### Negative

- Session entry shape is slightly more complex because both file id and version id may be present during the migration window.
- There is still a rebuild path, so some edge cases remain more expensive than the steady state.

### Risks

- Any write path that forgets to persist `content_file_version_id` would silently fall back to mutable file-head reads. We mitigate this by updating all current file externalization paths and keeping regression tests around the session tree shape.
- Mixed old/new sessions can make debugging noisier during the migration window. The fallback order is explicit to keep behavior predictable.

## Non-Goals

- Removing `content_file_id` compatibility immediately
- Eliminating the full rebuild path entirely
- Changing the Session entity's orchestration boundary

## Alternatives Considered

1. **Keep only `content_file_id` and read current file heads forever** — rejected because historical session content needs immutable identity.
2. **Require a full rebuild on every turn and rely on batch reads only** — rejected because batch reads reduce I/O overhead but do not fix replay cost by themselves.
3. **Introduce a separate cache store outside TemperFS for prepared context** — rejected because Session artifacts already fit naturally in Temper-native file storage.
