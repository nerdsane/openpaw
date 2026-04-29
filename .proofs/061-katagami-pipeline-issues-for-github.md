# Katagami Pipeline E2E: Issues for GitHub

Source: 5-query local pipeline run (2026-04-28/29), 14 DesignLanguages, ~113 CurationJobs.

---

## temper (nerdsane/temper)

### T1: ContextReady stores full LLM context inline on every turn — 300MB for 112 sessions

**Severity**: Critical (storage)
**Evidence**: `Session.ContextReady` events account for **300MB of a 491MB database**. Each event stores the full `prepared_context_inline_json` (the complete messages array sent to the LLM) in the event payload. A 200-turn session emits 200 ContextReady events, each ~50-70KB, totaling ~12MB per session.

The prepared context is the **same data** that also exists in:
- `SessionEntry` events (individual messages, 15MB total)
- `ots_trajectories` table (full conversation trace, 5MB)

Triple-stored. The ContextReady inline JSON is only needed by the `provider_response_applier` WASM in the same turn — it has no long-term value.

**Fix**: Either (a) store `prepared_context_file_id` reference only (already supported — 6 events use it, 4432 don't), or (b) add `overflow_ttl_seconds` to the prepared_context field so the blob sweeper cleans it up after N minutes, or (c) don't persist the inline JSON at all — reconstruct from SessionEntries if replay is needed.

### T2: No event retention / cleanup for completed sessions

**Severity**: High (storage)
**Evidence**: 112 sessions produced 55,774 Session events (317MB). All sessions are in terminal state (Completed/Failed). There is no mechanism to prune or compact events for completed sessions.

The event journal is append-only by design (event sourcing), but completed sessions don't need per-turn event granularity. A snapshot + trajectory is sufficient for audit.

**Fix**: Add a session compaction mechanism: once a session reaches a terminal state, compact its events into a single snapshot + keep only Created/Configure/RecordResult events. Or introduce event TTL per entity type.

### T3: No FileVersion event pruning after supersede

**Severity**: Medium (storage)
**Evidence**: 12,053 `FileVersion.Create` events (4.8MB), 10,417 `FileVersion.Supersede` events. Each file version that gets superseded retains full event history. For embodiment files that go through multiple drafts (agent writes, compiles, rewrites), old versions accumulate.

**Fix**: Compact superseded FileVersion events — only keep the current version's events.

### T4: Server startup blocked by runtime index recovery at scale

**Severity**: High (reliability)
**Evidence**: With 200K+ `entity_field_index` entries, 120K+ events, and 18K+ `entity_catalog` entries, `populate_index_from_store` took >60 minutes at 100% CPU (Phase 6a.5). Required manual database trimming to enable restart.

**Fix**: Index recovery should be lazy or bounded. Options: (a) skip index rebuild on startup, populate on first query; (b) persist index state so it doesn't need full rebuild; (c) add a timeout with degraded-mode startup.

### T5: entity_field_index grows unbounded

**Severity**: Medium (performance)
**Evidence**: 17K index entries for 1.5K entities. The index is rebuilt from events on startup and grows with each entity update. No pruning of stale index entries.

**Fix**: Rebuild only for entities that changed since last snapshot, not from scratch.

---

## openpaw (temperpaw / katagami-curation)

### P1: `completion_contract` defaults to `legacy-json-v1` — silently breaks publish

**Severity**: Critical (correctness)
**Evidence**: CurationJob entity spec defaults `completion_contract` to `"legacy-json-v1"`. The `finalize_spawned_session` WASM checks `if completion_contract == "typed-v1"` to decide whether to run verification + publish. Batch 1 quality_review (10 jobs) all completed agent work successfully but **none published** because the WASM took the legacy path.

Silent failure — no error, no warning. Jobs appeared to complete normally.

**Fix**: Change the entity spec default from `"legacy-json-v1"` to `"typed-v1"`. The legacy path exists for backward compatibility but should not be the default. Alternatively, have `finalize_spawned_session` always run verification regardless of contract version.

**Code**: `os-apps/katagami-curation/specs/curation_job.ioa.toml:89`, `os-apps/katagami-curation/wasm/finalize_spawned_session/src/lib.rs:160`

### P2: `Revise` resets booleans but `finalize_spawned_session` doesn't re-set them

**Severity**: High (correctness)
**Evidence**: The `Revise` action resets `has_design_md`, `design_md_verified`, `has_valid_design_md`, `quality_review_passed` to false, but leaves `design_md_file_id` and `embodiment_file_id` intact. The `verify_design_md` function in the finalize WASM skipped `AttachDesignMd` because `design_md_file_id` was non-empty — but `has_design_md` was false. This caused Publish guard failures (HTTP 409).

**Status**: Patched in this run (added re-attach logic), but the fix is a workaround. The real issue is that the verify functions don't account for the Revise reset pattern.

**Code**: `os-apps/katagami-curation/wasm/finalize_spawned_session/src/lib.rs:818-850`

### P3: Agent `import json` omission (gpt-5.5 recurring bug)

**Severity**: Medium (reliability)
**Evidence**: `name 'json' is not defined` across 4 jobs in 3 different job types (source_search, quality_review, organize_taxonomy). The gpt-5.5 agent generates Python tool-call code that uses `json.dumps()`/`json.loads()` without importing `json`.

**Fix**: Either (a) auto-import common modules in the tool execution sandbox, or (b) add explicit `import json` to all code examples in SKILL.md files, or (c) add a pre-execution injection of common imports.

### P4: Session polling timeout too short (80 checks × 15s = 20 min)

**Severity**: High (reliability)
**Evidence**: 10 out of 16 source_search failures were timeouts. Deep web research sessions regularly exceed 20 minutes. The `finalize_spawned_session` WASM polls 80 times at 15s intervals.

**Fix**: Make poll count configurable via CurationJob field, or switch to event-driven completion (reaction on Session.Completed triggers finalize).

### P5: No automatic retry for transient failures

**Severity**: Medium (reliability)
**Evidence**: OpenAI Codex API returned 503 (upstream connect errors) intermittently. Jobs fail and must be manually retried. There is a `Retry` action on CurationJob but no automatic mechanism to use it.

**Fix**: Add retry logic in `finalize_spawned_session` for transient errors (503, timeout). Limit to 2-3 retries with backoff.

### P6: Session completed but CurationJob stuck at Running

**Severity**: Medium (reliability)
**Evidence**: 1 quality_review session completed (413 events) but CurationJob remained Running. The `finalize_spawned_session` WASM was either not triggered or failed silently without transitioning the job.

**Fix**: Add a watchdog mechanism — if a session reaches terminal state but the parent job doesn't transition within N minutes, fire a compensating action.

### P7: Modal sandbox token staleness — no health check

**Severity**: Medium (reliability)
**Evidence**: Modal API tokens (`ak-`/`as-` format) became stale without warning. Quality review batch 2 failed with `sandbox provisioning failed with Modal sandbox HTTP 401 unauthorized`. Required manual token rotation.

**Fix**: Add a sandbox connectivity health check at server startup. If Modal tokens are invalid, log a clear warning and prevent quality_review jobs from starting.

### P8: Codex token rotation fragility

**Severity**: Medium (ops)
**Evidence**: Every server restart requires manually refreshing the Codex access token from `~/.codex/auth.json` and re-setting the vault secret. Codex tokens rotate and have limited scopes (`api.responses.write` missing).

**Fix**: Support standard OpenAI API keys (`sk-` format) which don't rotate. Already being migrated per memory note, but the server should detect and warn about expired/scoped tokens.

### P9: `provider_response_applier` race condition

**Severity**: Low (transient)
**Evidence**: 1 failure: `provider_response_applier: missing prepared_context_file_id or provider_response_file_id`. Partial state in the session turn pipeline.

**Fix**: Add defensive retry or ensure atomicity of the context preparation → provider call → response application pipeline.

---

## katagami (arni-labs/katagami) — UI repo

### K1: No issues discovered for the UI repo in this pipeline run

The UI was not exercised in this run (local SQLite pipeline only). UI-specific issues (TSX rendering, gallery display) would surface when connecting to the remote Turso DB.

---

## Storage analysis — why 491MB for 14 languages?

| What | Size | % | Notes |
|------|------|---|-------|
| Session.ContextReady (inline LLM context) | 300 MB | 61% | **Full message array stored every turn, ~200× per session** |
| SessionEntry.Created | 15 MB | 3% | Individual messages — same data as ContextReady but granular |
| Session.* (other events) | 17 MB | 3% | ProgressMade, ProcessToolCalls, HandleToolResults, etc. |
| File + FileVersion events | 14 MB | 3% | Embodiment and DESIGN.md version history |
| trajectories table | 12 MB | 2% | Audit log rows (119K actions) |
| ots_trajectories | 5 MB | 1% | Full conversation traces (third copy of context) |
| wasm_invocation_logs | 3 MB | 1% | 24K WASM executions |
| Everything else | 3 MB | 1% | Entities, specs, policies, blobs |
| SQLite overhead / indexes / free pages | ~122 MB | 25% | B-tree pages, free list, WAL artifacts |

**Root cause**: `Session.ContextReady` storing the full prepared context inline accounts for 61% of the DB. This is the same data that exists in `SessionEntry` events and `ots_trajectories`. Removing it would reduce the DB from 491MB to ~190MB. Adding session event compaction for terminal sessions would bring it under 50MB.

**The 113 CurationJobs spawned 112 sessions, each averaging ~500 events and ~3MB of event data.** The per-language cost is reasonable (~35MB/language); the per-turn cost (ContextReady inline) is not.
