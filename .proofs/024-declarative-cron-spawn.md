# Proof Report: 024 — Declarative CronJob → Session Spawning

## Date
2026-04-02

## Branch / Commit
- Temper: `main` @ `b425a8d5` — `feat: add copy_fields to spawn effect`
- OpenPaw: `main` @ `35b8f931` — `feat: declarative CronJob → Session spawning via platform spawn effect`

## What Was Done
Replaced the imperative `cron_trigger` WASM module (183 lines, 3 HTTP calls) with declarative platform effects. The CronJob now spawns Sessions via the Temper `spawn` effect with `copy_fields`, and Sessions auto-provision via a zero-delay `schedule` effect on Configure.

### Changes Made

**Temper platform** (8 files):
- Added `copy_fields: Option<Vec<String>>` to spawn effect across full pipeline (types → parser → translate → lint → JIT → builder → effects → dispatch)
- Parent entity fields are copied into child's initial_action params at spawn time

**OpenPaw** (10 files):
- Session.Configure now auto-provisions via `schedule(Provision, delay=0)`
- CronJob.Trigger fires `cron_compute_next` WASM (cron parsing + template substitution)
- CronJob.TriggerComplete spawns Session declaratively with `copy_fields` + `schedule_at` for next run
- Consolidated two WASM modules (cron_activate + cron_trigger) into one (cron_compute_next)
- Deleted cron_trigger WASM (183 lines)
- ADR-0011 documents the design

### Fixes During Implementation
1. **cron_activate WASM was never compiled** — missing from build.sh. Added it.
2. **`SystemTime::now()` panics in wasm32-unknown-unknown** — no clock available. Replaced with `Context::get_time_millis()` from the Temper WASM SDK host function.
3. **WASM config is `BTreeMap<String, String>`, not `serde_json::Value`** — fixed config access pattern.
4. **Daemon caches WASM in database** — needed clean rebuild + restart to pick up new module hash.

## Verification Flow
1. Start OpenPaw daemon with new WASM and specs
2. Create CronJob for DSF SRE agent with 2-minute schedule
3. Configure with system_prompt, user_message_template, model, tools
4. Activate → watch cron_compute_next WASM compute first next_run_at
5. Wait for platform timer to fire Trigger
6. Verify Session spawned with correct copied fields
7. Verify Session auto-provisions and runs to completion
8. Wait for second trigger to confirm self-scheduling loop

## Verification Results
| Step | Expected | Actual | Status |
|------|----------|--------|--------|
| CronJob.Activate | cron_compute_next WASM computes next_run_at | ActivateComplete with next_run_at=2026-04-02T15:34:10Z | PASS |
| Platform timer | Trigger fires at next_run_at | Trigger fired at ~15:34:10Z, run_count=1 | PASS |
| Session spawn | TriggerComplete spawns Session with copied fields | Session 019d4ed4-9bd4 created with model=claude-sonnet-4-6, soul_id=SRE, tools=temper_get,temper_list,read | PASS |
| Session auto-provision | Configure → Provision → Thinking chain | Session reached Thinking state automatically | PASS |
| Session completion | Session runs LLM and completes | Session status=Completed | PASS |
| Self-scheduling | TriggerComplete schedules next Trigger | next_run_at=2026-04-02T15:36:09Z set | PASS |
| Second trigger | run_count=2 after second cycle | run_count=2 confirmed at 15:36:12Z | PASS |
| Second Session | New Session spawned | Session 019d4ed6-6cbf created | PASS |

## What Worked
- Declarative spawn with copy_fields correctly propagates CronJob fields to Session.Configure
- Session auto-provision via schedule(Provision, delay=0) works seamlessly
- Template substitution ({{run_count}}, {{last_result}}) in WASM before spawn
- Self-scheduling loop via schedule_at confirmed across 2 consecutive trigger cycles
- One WASM module handles both activate and trigger modes via integration config

## What Didn't Work
- Initial attempt: cron_activate WASM was never compiled (missing from build.sh) — fixed by adding to build.sh
- SystemTime::now() panics in wasm32-unknown-unknown — fixed by using SDK host function
- Stale WASM cache in database caused old binary to run even after rebuild — fixed by clean rebuild + daemon restart

## Limitations
- Cron parser only supports simple interval patterns (*/N, 0 */N, etc.), not full cron expressions
- Template substitution is limited to {{run_count}}, {{last_result}}, {{now}}
- Session auto-provision means callers that did Configure+Provision separately will get a guard rejection on the redundant Provision call

## What Still Doesn't Work
- No max_runs enforcement — CronJob needs a guard on Trigger to check run_count < max_runs

## Artifacts
- Proof script: `scripts/prove_cron_scheduling.py`
- ADR: `docs/adrs/0011-declarative-cron-spawn.md`
- CronJob entity: `019d4ed2-cae0-7472-8cda-23137e19ecfe`
- Session 1: `019d4ed4-9bd4-7f70-9f17-b7fa667a8668` (Completed)
- Session 2: `019d4ed6-6cbf-70d3-87b3-10310fb87a0a`

## Architecture Diagram
```text
CronJob                              Platform                          Session
   │                                    │                                 │
   │──Activate──▶                       │                                 │
   │  trigger: cron_compute_next        │                                 │
   │  (WASM: parse cron → next_run_at)  │                                 │
   │◀─ActivateComplete(next_run_at)─────│                                 │
   │  effect: schedule_at → Trigger     │                                 │
   │                                    │                                 │
   │ ─ ─ ─ (platform timer fires) ─ ─ ─│                                 │
   │                                    │                                 │
   │──Trigger──▶                        │                                 │
   │  effects:                          │                                 │
   │   increment run_count              │                                 │
   │   trigger: cron_compute_next       │                                 │
   │   (WASM: next_run_at + template)   │                                 │
   │◀─TriggerComplete──────────────────▶│                                 │
   │  effects:                          │                                 │
   │   spawn Session + Configure ──────▶│──Create + Configure(fields)───▶│
   │     copy_fields propagates:        │  schedule(Provision, delay=0)   │
   │     system_prompt, model, tools,   │──Provision──────────────────────▶│
   │     soul_id, user_message          │                                 │ (running)
   │   schedule_at → next Trigger       │                                 │
   │                                    │                                 │
   │ ─ ─ ─ (next timer fires) ─ ─ ─ ─ ─│                                 │
   └─── repeats ────────────────────────┘                                 │
```
