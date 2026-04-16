# ADR-0033: web_fetch — Migrate Off TemperFS File Workaround

- Status: Accepted
- Date: 2026-04-16
- Deciders: OpenPaw maintainers
- Related:
  - ADR-0032: TemperFS agent operations
  - temper ADR-0040: Blob-backed overflow for large entity field values
  - temper ADR-0045: Field-overflow inline ceiling (128KB)
  - temper ADR-0046: WASM host function for blob-ref field reads
  - `os-apps/paw-research/wasm/web_fetch/src/lib.rs`
  - `os-apps/paw-research/specs/web_query.ioa.toml`
  - `os-apps/paw-research/specs/model.csdl.xml`
  - `os-apps/paw-agent/wasm/monty_repl/src/dispatch.rs`

## Context

`web_fetch` used to split its result path by size: values under 30KB went inline into `WebQuery.results`, values over 30KB went through a hand-rolled TemperFS File detour — create a File entity via OData, PUT the content to `Files('{id}')/$value`, store the `result_file_id` on the WebQuery, and have `monty_repl` read the file then `POST Temper.Archive` to delete it.

That workaround existed entirely to dodge Temper's 32KB field-sync truncation. It was a lot of moving parts for something that is conceptually "a large string field": an extra entity, an extra network hop, a delete-after-read race, and a second code path in the consumer. The existing code literally pointed at [nerdsane/temper#106](https://github.com/nerdsane/temper/issues/106) as the real fix.

Temper ADR-0045 raised the inline ceiling to 128KB. Temper ADR-0046 added `host_read_field` so any value above the ceiling (stored as a blob ref) is readable from WASM transparently via `ctx.read_field_string`. Together those two ADRs make the workaround unnecessary: a 400KB web_fetch result can land directly in `WebQuery.results` and the consumer reads it like any other field. The File-entity detour becomes dead code.

## Decision

Retire the TemperFS File path in `web_fetch` and the matching file-read-then-archive branch in `monty_repl`. Drop `result_file_id` from the IOA spec and CSDL. Declare `overflow_ttl_seconds = "3600"` on `WebQuery.results` as forward-compatible metadata for temper ADR-0047b (spec-declared TTL wiring); the declaration is inert until that ADR lands, after which overflow blobs from web_fetch expire after 1h.

### Sub-Decision 1: Unconditional inline write in web_fetch

`web_fetch` always emits `RecordResults({"results": <content>})`. The existing `MAX_CONTENT_LEN = 100_000` (chars) keeps worst-case UTF-8 output at ~400KB of bytes, which is well within Temper's field-overflow handling. No explicit size guard on top of that — the existing character cap is the cap.

### Sub-Decision 2: Drop `result_file_id` end-to-end

- `web_query.ioa.toml` — remove the `[[state]]` block and remove `result_file_id` from `RecordResults.params`.
- `model.csdl.xml` — remove `<Property Name="ResultFileId" ...>` and the matching `<Parameter>` on `RecordResults`.
- `monty_repl/dispatch.rs` — `interpret_web_query_entity_result` returns `(status, results_raw)` instead of `(status, Option<file_id>, results_raw)`. The file-read-then-archive block is deleted. OData GET already hydrates blob refs, so `fields.get("results").as_str()` returns the full value regardless of whether Temper stored it inline or as a blob ref.

**Why this is safe without the SDK helper**: OData `GET /tdata/WebQueries('{id}')` flows through `hydrate_blob_refs_for_tenant`, which resolves blob refs before returning the JSON. The consumer sees the hydrated string.

### Sub-Decision 3: No active orphan cleanup in this PR

Existing Files already created by the old workaround sit in TemperFS with `Status = Ready` until someone archives them. An audit-and-cleanup script would belong in operator tooling, not the migration PR. The operational impact is small — File blobs are persistent anyway, and the archival state machine still works by hand.

## Rollout Plan

1. **Phase 5 (this ADR)** — ship the migration. Depends on temper Phase 2 landing (SDK `read_field_string` is used by the Phase 3 migration, which unblocks `llm_caller` + `workspace_provisioner`; monty_repl's web_query path does not use the SDK helper because OData GET is already hydrating).
2. **Operational follow-up** — archive orphan `web-fetch-*` Files left by the old code. Can run any time.
3. **Future** — once temper ADR-0047b ships, the inert `overflow_ttl_seconds` declaration starts expiring `WebQuery.results` blobs after 1h.

## Consequences

### Positive

- One code path for web_fetch results. No more dual-storage contract, no more delete-after-read race.
- No File entity churn per fetch. Fewer OData actions, fewer state transitions, less audit noise.
- `monty_repl` simplifies: one tuple arm, one text path.
- Template for future modules that hit field-overflow: "use the field, trust the platform."

### Negative

- Orphan Files from prior deployments remain until manually archived. Acceptable — not a correctness concern.
- `overflow_ttl_seconds` on `results` is inert until temper ADR-0047b. Callers reading the spec might be surprised by a declared TTL that doesn't fire. Mitigated by the inline comment on the `[[state]]` block.

### Risks

- Any agent or external consumer that was reading `WebQuery.result_file_id` directly (bypassing monty_repl) will break after this spec change. Known consumers: monty_repl only, per grep. Low risk but worth flagging in the PR.

## Non-Goals

- Orphan-file cleanup script (operator task, separate follow-up).
- Migrating any other module off its own hand-rolled large-payload path (future ADRs per module).
- Spec-declared TTL wiring (that is temper ADR-0047b).

## Alternatives Considered

1. **Keep the TemperFS File path as a fallback for >128KB results.** Rejected — the whole point of temper ADR-0045 + ADR-0046 is to remove the need for per-module large-payload workarounds. Keeping one "just in case" undermines the platform contract.
2. **Migrate consumer to `ctx.read_field_string("results")`.** Considered but unnecessary for monty_repl because OData GET already hydrates. Using the SDK helper would be correct but redundant for this path.
3. **Emit a `WebFetchTruncated` action for size-capped results.** Considered during planning; dropped because `MAX_CONTENT_LEN = 100_000` chars already caps the output, and the existing truncation is silent-by-design (the module just trims). Adding an explicit event is a nice-to-have but not blocking.

## Rollback Policy

Revert the PR. The old TemperFS code path returns; any File rows created between ship and revert remain readable. `result_file_id` state+CSDL come back; no schema migration needed on Temper's side because the column was never removed from the entity-field side (it's just a string field that goes away from the projection). WebQuery rows written during the ADR-0033 window have empty `result_file_id` which the old code already tolerated.
