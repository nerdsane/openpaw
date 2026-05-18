# ADR-023: SessionEntry Materialization Read-Back Compression

## Status

Proposed for PERF-024.

## Context

PERF-023 moved first-turn SessionEntry creation out of `workspace_provisioner`
and into provider-response materialization. INC-003 then repaired the
header/user/assistant sequence invariant so the materialized first turn reads
back deterministically as `0/1/2`.

The fixed-version production trace
`281a8c195707f8149dd6b52a29cde73a` shows the remaining first-turn tail is now
inside `provider_response_applier`:

- `Session.ProviderResponseReady.integrations`: about `324 ms`
- `wasm:provider_response_applier`: about `324 ms`
- materialization POST spans: about `84 ms` and `136 ms`
- read-back GET/OData spans: about `8 ms`, `12 ms`, and `16 ms`

The POST side is already sent through `ctx.http_call_batch`, and the pinned
Temper SDK revision runs default batch requests concurrently. The overlooked
cost is the read-back verification shape: after the batched POST succeeds, the
guest verifies each expected entry with a separate filtered projection read.
That preserves correctness, but it adds repeated guest-to-host calls, OData
handler work, projection enumeration, and response parsing for rows that belong
to the same freshly materialized Session.

## Decision

Keep the concurrent POST batch. Replace the multi-request read-back verification
for batched initial SessionEntry materialization with one session-scoped
SessionEntries read:

```text
GET /tdata/SessionEntries?$filter=SessionId eq '<session_id>'&$top=10000
```

The guest then validates the expected EntryIds in memory before returning
success. `session_entries_materialized=true` remains gated on this read-back
proof; no caller may claim success unless every expected header/user/assistant
EntryId is visible in the projection response.

The single-entry append path keeps its existing per-entry verification. That
path is not the selected PERF-024 hot span and it benefits from the narrower
query when only one newly appended EntryId is involved.

## Correctness Rules

1. A batched materialization is successful only if every expected EntryId is
   visible in one read-model response for the same SessionId.
2. The response parser must tolerate both canonical OData field names
   (`EntryId`) and any lower-case projection aliases already accepted elsewhere.
3. Invalid JSON, missing `value`, and empty result sets must fail closed.
4. The change must not weaken the INC-003 sequence invariant. The live after
   proof must still read back header/user/assistant as `0/1/2`.
5. If propagation lag delays visibility, the existing bounded retry budget
   remains in place.

## Observability And Verification

Local verification must include focused parser tests, full `wasm-helpers`
tests, provider-response applier tests, Session architecture tests, Datadog
observability contract tests, locked check/clippy, rustfmt, diff whitespace,
and release WASM builds for the affected modules.

Production acceptance must include:

- before Datadog trace/window from version
  `70c83302a213ed11ae834d40fc60fa16678688c6`
- after fixed-version trace/window for the PERF-024 PR
- `provider_response_applier` and `Session.ProviderResponseReady.integrations`
  before/after timing
- trace proof that the read-back shape no longer emits multiple per-entry GETs
  for first-turn materialization
- live read-back proof that the SessionEntry tree is still complete and ordered
  `0/1/2`

## Rollback

Revert the helper change. The rollback restores per-entry verification reads
for batched initial materialization while preserving the PERF-023 lazy
materialization and INC-003 sequence-ordering behavior.
