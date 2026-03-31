# Proof Report: 023 — Deep-Sci-Fi Bespoke Agent Team + Harness

## Date
2026-03-31

## Branch / Commit
`feat/dsf-team-and-harness` / `6e8b9f5d`

## What Was Done

Designed and implemented a bespoke agent team and development harness for deep-sci-fi. This includes:

1. **Reference project** (`projects/deep-sci-fi/`) — 20 files: ADR, specs, soul, skills, policies
2. **Team**: Ren (INTP product lead, only soul), SWE, SRE, Design, Librarian
3. **Harness auto-injection**: `project_harness_id` on Agent + `load_harness_block()` in llm_caller
4. **Two-layer tool governance**: Cedar ToolHooks for bash CLIs + Temper-native API tools (Railway, Vercel)
5. **Rust cron trigger**: replaces WASM polling CronScheduler
6. **Governed sandbox spec**: Computer entity with tools_installed, credentials_scoped, network_allow (design only, spec changes reverted)

## Verification Flow

Ran the OpenPaw daemon from the worktree and verified features via OData API.

## Verification Results
| Step | Expected | Actual | Status |
|------|----------|--------|--------|
| 1. Daemon starts | All 8 apps load | 7 of 8 load (paw-compute skips — pre-existing issue) | PARTIAL |
| 2. ProjectHarness created | Active with full conventions | Active, repo_url + tech_stack + conventions populated | PASS |
| 3. Agent with project_harness_id | Field stored on entity | `project_harness_id: 019d45a9-...` confirmed | PASS |
| 4. Harness auto-injection | Conventions in assembled prompt | Code in place, compiles. Cannot verify end-to-end without sandbox (TL_API_KEY not set) | BLOCKED |
| 5. WorkCycle gates | Report* actions + guard enforcement | Reverted — IOA TOML guard format needs investigation | GAP |
| 6. Cedar ToolHook | `gh pr merge` blocked for SWE | Code in place, compiles. Cannot verify without running agent | BLOCKED |
| 7. Railway API tool | Governed deployment query | Code in place, compiles. Cannot verify without RAILWAY_TOKEN | BLOCKED |
| 8. Vercel API tool | Governed deployment query | Code in place, compiles. Cannot verify without VERCEL_TOKEN | BLOCKED |
| 9. Heal loop | Simulated alert → Fixed | Not tested (depends on sandbox for SRE agent) | BLOCKED |
| 10. Cron trigger | Fires on schedule | Rust code compiled and spawned at startup. No active CronJobs to trigger. | PARTIAL |
| 11. Computer spec | Governance fields accepted | Reverted — paw-compute bundle validation issue | GAP |

## What Worked
- All Rust code compiles (daemon + all WASM modules)
- Reference project structure: 20 files with ADR, specs, soul, skills, policies
- ProjectHarness creation with full conventions via OData API
- Agent entity stores `project_harness_id` field
- `load_harness_block()` function in llm_caller (code review: fetches harness, formats XML, injects into prompt)
- Cedar ToolHook extension: `evaluate_before_hooks()` now accepts `input` and `project_harness_id`, checks `cedar_enabled` and `command_pattern`
- Railway + Vercel API tool wrappers (code review: follow datadog.rs pattern, vault credentials, structured actions)
- Rust cron trigger (code review: follows webhook trigger pattern, tokio sleep loop, dispatches CronJob.Trigger)
- startup.rs: cron trigger spawned, RAILWAY_TOKEN + VERCEL_TOKEN in vault
- alert_opener: passes `project_harness_id` to SRE agent Configure
- paw-harness: WorkCycle spec loads and updates successfully (without gate additions)

## What Didn't Work
- WorkCycle gate pattern: The `is_true` guard only accepts a single field. Compound guards (`a == 'true' && b == 'true'`) are not supported by Temper's IOA parser. Need to investigate how to express multi-field guards.
- paw-compute bundle: Skipped on startup (pre-existing issue, not caused by our changes)

## Limitations
- **No sandbox**: TL_API_KEY not configured → agent provisioning fails → cannot test full agent loop (harness injection, tool governance, heal loop)
- **No external service tokens**: RAILWAY_TOKEN, VERCEL_TOKEN not set → cannot verify API tool wrappers against real services
- **IOA guard format**: Temper's guard expressions need study. The `is_true` primitive works for single bools, but multi-field gates need a different approach (possibly invariants or multiple single-field guards on sequential transitions)

## What Still Doesn't Work
1. **WorkCycle gate enforcement**: Gate fields exist in the reference project spec but not yet in the paw-harness os-app. Needs IOA format investigation.
2. **Computer governance fields**: Same issue — fields defined in reference project but paw-compute spec reverted.
3. **End-to-end agent testing**: Blocked by sandbox provisioning. All WASM code compiles but hasn't been execution-tested.
4. **Ren soul on Discord**: Not wired yet — this is Phase 4 (after verification).

## Artifacts
- Branch: `feat/dsf-team-and-harness`
- Reference project: `projects/deep-sci-fi/` (20 files)
- ADR: `projects/deep-sci-fi/adr/001-team-and-harness-design.md`
- Ren's soul: `projects/deep-sci-fi/souls/ren/SOUL.md` + `STYLE.md`
- 4 skills: swe-conventions, design-system, content-standards, sre-monitoring
- Cedar policies: autonomy + tool governance
- Rust cron trigger: `crates/paw-transport/src/cron/`
- API tools: `os-apps/paw-agent/wasm/tool_runner/src/railway.rs` + `vercel.rs`

## Architecture Diagram
```text
Human (Discord)
  |
  v
Paw (chief of staff) ──────────────────────────┐
  |                                              │
  v                                              │ (later: multiple projects)
Ren (product lead, INTP, bespoke soul)          │
  |                                              │
  ├── SWE (skills: swe-conventions)             │
  │     └── Harness gates enforced by platform  │
  │     └── Tools: bash(gh via Cedar), read/write/edit
  │                                              │
  ├── SRE (skills: sre-monitoring)              │
  │     └── Alert-driven (heal loop)            │
  │     └── Cron: monitor health scan every 6h  │
  │     └── Tools: temper CRUD, spawn_agent     │
  │                                              │
  ├── Design (skills: design-system)            │
  │     └── Reviews + produces UI work          │
  │                                              │
  └── Librarian (skills: content-standards)     │
        └── Observes platform content health    │
                                                 │
Tool Governance (two layers):                    │
  Layer 1: Cedar ToolHooks → bash commands       │
    └── "SWE can gh pr create, not gh pr merge" │
  Layer 2: Temper-native API tools               │
    └── railway_api (GraphQL, vault creds)      │
    └── vercel_api (REST, vault creds)          │
    └── datadog_query (existing)                │
                                                 │
Harness Auto-Injection:                          │
  Agent.project_harness_id → llm_caller          │
    └── load_harness_block() → <project_harness> │
    └── Prompt: soul → override → HARNESS → skills → memory
                                                 │
Cron Trigger (Rust, no polling):                 │
  tokio::time::sleep_until()                     │
    └── CronJob.Trigger → cron_trigger WASM     │
    └── Replaces: CronScheduler polling loop    │
```
