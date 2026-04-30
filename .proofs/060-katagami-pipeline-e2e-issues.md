# Proof Report: 060 — Katagami Pipeline E2E Issues (5-Query Run)

## Date
2026-04-29

## Branch / Commit
main (local TemperPaw server, PID 26142, localhost:3467, SQLite backend)

## What Was Done
Ran 5 source_search queries through the full Katagami curation pipeline:
1. `crossover-editorial` — Crossover editorial / ink-meets-digital
2. `minimalist` — Minimalist design systems
3. `illustration-graphics` — Illustration & graphics
4. `childrens-books` — Children's books / storybook aesthetics
5. `watercolor-illustration` — Watercolor illustration & editorial

Pipeline stages: source_search -> synthesize -> quality_review -> publish

Produced 14 DesignLanguages. Target: all published via paw agents (no manual intervention).

## Issues Encountered

### ISSUE-1: OpenAI Codex Token Scope (401 — `api.responses.write`)

**Severity**: Blocking
**Stage**: source_search (initial attempts)
**Error**: `OpenAI Codex API returned 401: insufficient permissions. Missing scopes: api.responses.write`
**Root cause**: Codex tokens from `~/.codex/auth.json` use a limited scope that doesn't include `api.responses.write`. The Temper OpenAI provider was using these tokens directly.
**Fix**: Refresh the Codex access token (which rotates) and re-set the vault secret each time the server restarts.
**Impact**: Every server restart requires re-setting the secret:
```bash
CODEX_TOKEN=$(python3 -c "import json; print(json.load(open('$HOME/.codex/auth.json'))['tokens']['access_token'])")
curl -s -X POST "http://localhost:3467/paw/setup/secrets" -H "Authorization: Bearer test-local-key" -H "x-tenant-id: default" -H "Content-Type: application/json" -d "{\"key\": \"openai_codex_access_token\", \"value\": \"$CODEX_TOKEN\"}"
```
**Systemic**: This is fragile. Need a persistent API key (standard `sk-` format) or automatic token refresh.

### ISSUE-2: Unresolved Secret Template

**Severity**: Blocking
**Stage**: source_search
**Error**: `provider=openai_codex api key is unresolved secret template: '{secret:openai_codex_access_token}'. set tenant secret and retry`
**Root cause**: Server started before vault secret was set.
**Fix**: Set the vault secret before submitting jobs.

### ISSUE-3: Session Timeout (80 Poll Checks)

**Severity**: High
**Stage**: source_search (batches 1 & 2)
**Error**: `Child Session did not reach a terminal state after 80 checks`
**Root cause**: Session polling loop in `finalize_spawned_session` checks 80 times at 15s intervals (20 min total). Some agent sessions took longer, especially when doing deep web research with many sources.
**Impact**: 10 out of 16 source_search failures were timeouts.
**Suggestion**: Make poll count configurable, or use event-driven completion instead of polling.

### ISSUE-4: OpenAI Codex API 503 (Upstream Failures)

**Severity**: Medium (transient)
**Stage**: source_search, quality_review
**Error**: `OpenAI Codex API returned 503: upstream connect error or disconnect/reset before headers`
**Root cause**: Transient upstream failures at the Codex API gateway.
**Impact**: Jobs fail and need retry. No automatic retry mechanism.

### ISSUE-5: `name 'json' is not defined` (Agent Code Bug)

**Severity**: High
**Stage**: source_search (minimalist-v5), quality_review, organize_taxonomy
**Error**: `name 'json' is not defined. Did you forget to import 'json'?`
**Root cause**: The gpt-5.5 agent generates Python code for tool calls that uses `json.dumps()` or `json.loads()` without importing `json` first. This happens intermittently across multiple job types.
**Impact**: 4 jobs across 3 different job types failed with this error.
**Suggestion**: The SKILL.md files should include explicit `import json` in code examples, or the tool execution sandbox should auto-import common modules.

### ISSUE-6: Modal Sandbox HTTP 401 (Stale Tokens)

**Severity**: Blocking
**Stage**: quality_review (batch 2)
**Error**: `sandbox provisioning failed with Modal sandbox HTTP 401 unauthorized`
**Root cause**: The server had stale Modal API tokens (`ak-n00wdRt...` / `as-EVVmejr...`) that didn't match the active Modal workspace. Modal tokens rotate or become workspace-specific.
**Fix**: Updated server secrets with current tokens from `~/.modal.toml`:
```bash
curl -s -X POST "http://localhost:3467/paw/setup/secrets" -d '{"key":"modal_token_id","value":"ak-mMEQY3xDXQPHlJ6ulhpuS7"}'
curl -s -X POST "http://localhost:3467/paw/setup/secrets" -d '{"key":"modal_token_secret","value":"as-3UaN8SNtjJfHeJeVWtqlP7"}'
```
**Systemic**: Need token rotation detection or health check for sandbox connectivity.

### ISSUE-7: completion_contract Mismatch (Root Cause of Publish Failures)

**Severity**: Critical
**Stage**: quality_review finalization
**Root cause**: CurationJob entity spec defaults `completion_contract` to `"legacy-json-v1"`. The `build_session_message` WASM defaults it to `"typed-v1"` when building the session. The `finalize_spawned_session` WASM reads the **entity field** (not the session), so it sees `"legacy-json-v1"` and takes the legacy path — which does NOT call `verify_quality_reviewed_languages()` or `Publish`.
**Impact**: All 10 quality_review jobs from batch 1 completed successfully (agents did full sandbox work) but none of the languages were published. The finalize WASM silently took the wrong code path.
**Fix**: Pass `completion_contract: "typed-v1"` explicitly in the Configure action params when creating jobs.
**Code locations**:
- Entity default: `curation_job.ioa.toml` → `completion_contract` initial = `"legacy-json-v1"`
- WASM default: `build_session_message/src/lib.rs:544-546` → defaults to `"typed-v1"`
- Finalize check: `finalize_spawned_session/src/lib.rs:160` → `if completion_contract == "typed-v1"`
**Suggestion**: Either change the entity default to `"typed-v1"`, or have `finalize_spawned_session` always use the typed path regardless of the field value.

### ISSUE-8: Publish 409 from UnderReview After Revise

**Severity**: Medium
**Stage**: quality_review finalization
**Error**: `Action 'Publish' not valid from state 'UnderReview'` (HTTP 409)
**Context**: After languages were manually published and then Revise'd back to UnderReview, the `Revise` action resets boolean flags (`has_valid_design_md`, `design_md_verified`, `quality_review_passed`). The finalize WASM called `MarkQualityPassed` then `Publish`, but the guards on `Publish` require ALL boolean flags to be true. If the agent didn't re-set the reset flags, the Publish guard fails.
**Fix**: New quality_review agents must re-verify all artifacts before calling CompleteQualityReview.

### ISSUE-9: Session Completed But Job Stuck at Running

**Severity**: Medium
**Stage**: quality_review
**Error**: Session completed (413 events) but CurationJob remained in Running state.
**Root cause**: The `finalize_spawned_session` WASM was either not triggered or failed silently without transitioning the job.
**Impact**: 1 job stuck; required manual Fail + replacement job.

### ISSUE-10: provider_response_applier Missing Fields

**Severity**: Low (transient)
**Stage**: source_search
**Error**: `provider_response_applier: missing prepared_context_file_id or provider_response_file_id`
**Root cause**: Race condition or partial state in the session turn pipeline.
**Impact**: 1 job failure.

### ISSUE-11: Synthesis Without Output

**Severity**: Low
**Stage**: synthesize
**Error**: `synthesis completed without any design_language_ids`
**Root cause**: Agent completed synthesis work but didn't produce any DesignLanguage entities (likely an agent reasoning failure).
**Impact**: 1 job failure. The retry (synth-wabi-sabi-v2) succeeded.

### ISSUE-12: Legitimate Quality Gate (storybook-page-turn)

**Severity**: N/A (working as designed)
**Stage**: quality_review
**Error**: `embodiment too small for full-page review; signature typography not present; CSS token motif missing; table component absent; DESIGN.md projection incomplete`
**Root cause**: The embodiment genuinely didn't meet quality standards.
**Impact**: Correct behavior — the quality review agent found real issues. The embodiment needs actual improvement before it can pass review.

## Verification Results
| Step | Expected | Actual | Status |
|------|----------|--------|--------|
| 5 source_search queries complete | All 5 produce DesignSources | All 5 completed after retries | PASS |
| Synthesize creates languages | 14 DesignLanguages produced | 14 produced (some after retries) | PASS |
| quality_review agents publish all | 14/14 Published | 5/14 Published, 9 in batch 3 pending | PENDING |
| completion_contract: typed-v1 | Finalize takes typed path | Batch 3 uses typed-v1 | PENDING |

## What Worked
- Source search agents produced high-quality design sources (29-34 sources per query)
- Synthesize agents created 14 distinct design languages with full specs
- Quality review agents did thorough sandbox work: DESIGN.md generation, lint, Playwright screenshots
- Modal sandbox integration works well after token fix
- Staggered retry strategy (create new jobs after failures) is effective
- `finalize_spawned_session` typed-v1 path correctly publishes when all flags are set

## What Didn't Work
- completion_contract defaulting broke the entire first quality_review batch silently
- Modal token staleness had no health check or warning
- Agent `import json` omission is a recurring cross-job-type bug
- Session timeout (80 polls) is too short for deep research sessions
- No automatic retry for transient 503 errors

## Limitations
- Local SQLite backend (no concurrent write protection for high-throughput)
- Single TemperPaw server (no horizontal scaling)
- Agent model (gpt-5.5) intermittently produces code with missing imports
- 9 quality_review jobs running simultaneously (may hit Codex rate limits)

## What Still Doesn't Work
- Batch 3 (9 quality_review jobs) in progress — outcome pending
- If any fail, may need batch 4

## Artifacts
- 14 DesignLanguages in local SQLite at `~/.local/share/temperpaw/paw.db`
- 80+ CurationJobs across source_search, synthesize, quality_review, organize_taxonomy
- Agent sessions with full event traces in Sessions entity
