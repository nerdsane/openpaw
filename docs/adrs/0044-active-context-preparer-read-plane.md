# ADR-0044: Active Context Preparer Owns Delta and Batch Reads

**Status:** Accepted
**Date:** 2026-04-25
**Related:** ADR-0005, ADR-0034, ADR-0040 (Immutable Session Content References and Delta Context Preparation), ADR-0040 (Remove `llm_caller` and Make Staged Turn WASMs Authoritative), Temper ADR-0057, Temper ADR-0061

## Context

OpenPaw landed the right read-path architecture in two steps:

1. Temper added the native TemperFS batch read plane for current `File` reads
   and immutable `FileVersion` reads.
2. OpenPaw added immutable session content refs, batch file resolution, and
   delta prepared-context reuse.

The OpenPaw implementation initially lived in the old `llm_caller` path. The
later staged-turn cutover correctly removed `llm_caller` and made
`context_preparer`, `provider_caller`, and `provider_response_applier`
authoritative. During that cutover, the active `context_preparer` kept prepared
artifact writing but did not preserve the delta reuse and batch read behavior.

That left production with the platform read primitive available but the active
context-prep module still able to rebuild long session context through serial
file reads.

## Decision

The active `context_preparer` is the sole owner of Session context hydration.
It must implement the read-path invariants from the immutable session content
ADR directly:

1. Reuse a prior prepared context artifact when the prior leaf is an ancestor of
   the current session leaf and the context contract is compatible.
2. Use `SessionTree::build_context_refs_since` for append-only deltas.
3. Resolve file-backed refs through TemperFS batch reads, preferring immutable
   `content_file_version_id` over mutable `content_file_id`.
4. Keep serial `$value` reads only as a bounded compatibility fallback for
   explicit batch misses or unavailable old data.
5. Carry `prepared_context_file_id`, `system_prompt_hash`, and
   `system_prompt_file_id` across continuation Sessions so reuse survives the
   one-session-per-turn model.
6. Dispatch `ProgressMade` from `PreparingContext` after real forward progress
   so state timeouts measure stalls, not large but healthy context assembly.

Timeout increases, manual resets, and external caches are not acceptable fixes
for this class of failure.

## Consequences

- The staged-turn architecture stays intact: no return to a monolithic
  `llm_caller` or hidden orchestration crate.
- Long existing DM threads may batch rebuild once after deploy, then subsequent
  turns should reuse deltas.
- Datadog should show batch current-file/version reads during context prep and
  should not show large serial `File/$value` storms for normal continuation
  turns.

## Verification

Implementation must include:

- regression tests that fail if the active `context_preparer` stops using
  `build_context_refs_since` and the TemperFS batch helper APIs
- continuation tests proving cache fields are carried into `Session.Configure`
- timeout tests proving `PreparingContext` resets on `ProgressMade`
- a local live `tpaw server` end-to-end proof with OData state checks and
  captured logs or Datadog evidence

## Non-Goals

- Migrating existing sessions ahead of time
- Resetting production DM threads as part of the architecture
- Adding a new Rust background worker or external read cache
