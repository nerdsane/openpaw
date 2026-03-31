# Proof Report: 021 — Temper-Native Self-Heal Loop Refactor

## Date
2026-03-30

## Branch / Commit
`feat/openpaw-self-heal-loop-codex` (pre-commit)

## What Was Done

Refactored the self-heal loop from ~1400 lines of hardcoded Rust orchestration in `webhooks.rs` to Temper-native architecture:

1. **Webhook processing is now an auditable entity** (WebhookEvent: Created -> Validating -> Routing -> Processed/Rejected)
2. **Orchestration moved to WASM integrations** on AlertCycle state transitions
3. **Agents self-report** outcomes via temper_action (no watchers)
4. **Webhook trigger** follows ONE-entity-ONE-action rule
5. **Guardrails** prevent non-Temper-native implementations: ADR-0005, agents.md, Architect Reviewer agent, CLAUDE.md rules

## Architecture Diagram

```
                        EXTERNAL WORLD
                    +-------------------------+
                    |  Datadog / PagerDuty /   |
                    |  GitHub / Custom         |
                    +----------+--------------+
                               | POST /triggers/webhook/{route_key}
                               v
                +------------------------------------+
                |  WEBHOOK TRIGGER                    |
                |  crates/paw-transport/src/webhook/  |   ~80 lines Rust
                |  ONE entity, ONE action             |   (was ~1400 lines)
                +----------+-------------------------+
                           | WebhookEvent.Received
                           v
    +----------------------------------------------------------+
    |              TEMPER ENTITY + WASM LAYER                   |
    |         (specs + WASM integrations + Cedar)               |
    +----------------------------------------------------------+

    WebhookEvent                        WebhookRoute
    ~~~~~~~~~~~~                        ~~~~~~~~~~~~
    Created                             (config entity)
      | Received                        route_key, source_type,
      v                                 target_entity, target_action
    Validating                          webhook_secret
      | WASM: validate_webhook
      v
    Routing
      | WASM: route_webhook
      v
    Processed ----> AlertCycle.Open dispatched


    AlertCycle          SRE Agent            Developer Agent
    ~~~~~~~~~~          ~~~~~~~~~            ~~~~~~~~~~~~~~~
    Created
      | Open
      | WASM: alert_opener -----> Agent(SRE) spawned
      v                            |
    Triaging                       | LLM triage
      |                            |
      | <-- temper_action ------   | HealComplete / TuneComplete / Escalate
      v
    Fixed
      | WASM: cicd_initiator -> BeginMerge
      v
    Merging
      | WASM: cicd_merger (self-loop: poll GitHub, squash merge)
      v
    Deploying
      | WASM: deployment_tracker (self-loop: poll GitHub deployments)
      v
    Verifying
      | WASM: alert_verifier (self-loop: poll DD/monitoring API)
      v
    Resolved ---> WASM: heal_reporter -> Channel summary
```

## Files Created

| File | Purpose |
|------|---------|
| `docs/adrs/0005-temper-native-orchestration.md` | ADR documenting the decision |
| `agents.md` | Temper-native guide for all agents |
| `.claude/agents/temper-native-reviewer.md` | Architect Reviewer agent definition |
| `scripts/check-temper-native.sh` | PreCommit hook script |
| `.claude/settings.json` | Claude Code hook config |
| `crates/paw-transport/src/webhook/mod.rs` | Webhook trigger module |
| `crates/paw-transport/src/webhook/trigger.rs` | Webhook trigger (~80 lines) |
| `os-apps/paw-ingest/specs/webhook_event.ioa.toml` | WebhookEvent entity spec |
| `os-apps/paw-ingest/specs/webhook_route.ioa.toml` | WebhookRoute entity spec |
| `os-apps/paw-ingest/specs/model.csdl.xml` | CSDL model for OpenPaw.Ingest |
| `os-apps/paw-ingest/policies/webhook.cedar` | Cedar permits |
| `os-apps/paw-ingest/wasm/validate_webhook/` | HMAC validation WASM |
| `os-apps/paw-ingest/wasm/route_webhook/` | Payload normalize + route WASM |
| `os-apps/paw-ingest/wasm/process_webhook/` | Dispatch target action WASM |
| `os-apps/paw-heal/wasm/alert_opener/` | Spawn SRE on AlertCycle.Open |
| `os-apps/paw-heal/wasm/cicd_initiator/` | Fixed -> BeginMerge gate |
| `os-apps/paw-heal/wasm/cicd_merger/` | GitHub PR merge WASM |
| `os-apps/paw-heal/wasm/deployment_tracker/` | GitHub deployment polling WASM |
| `os-apps/paw-heal/wasm/alert_verifier/` | DD monitor verification WASM |
| `os-apps/paw-heal/wasm/heal_reporter/` | Terminal state Channel reporting |

## Files Modified

| File | Change |
|------|--------|
| `CLAUDE.md` | Added Temper-Native Rule section |
| `souls/developer.md` | Added Architecture section referencing agents.md |
| `os-apps/paw-heal/specs/alert_cycle.ioa.toml` | Added self-loop actions, integrations, report fields |
| `os-apps/paw-heal/policies/alert_cycle.cedar` | Added new action permits |
| `crates/paw-transport/src/lib.rs` | Added `pub mod webhook` |
| `crates/openpaw/src/startup.rs` | Added paw-ingest to os-apps, webhook trigger spawn, removed webhook router |
| `crates/openpaw/src/main.rs` | Removed `mod webhooks` |

## Files Removed

| File | Reason |
|------|--------|
| `crates/openpaw/src/webhooks.rs` | Renamed to `_webhooks_deprecated.rs`. ~1400 lines of hardcoded orchestration replaced by 9 WASM modules + 1 thin trigger |

## Verification Results (Pre-Build)

| Step | Expected | Actual | Status |
|------|----------|--------|--------|
| WebhookEvent spec follows Channel pattern | States + actions + integrations + invariants | Created with Received->Validating->Routing->Processed + 3 WASM integrations | CREATED |
| WebhookRoute spec is config entity | Active/Disabled states, Register/Update actions | Created with route_key, source_type, target config | CREATED |
| AlertCycle has self-loop actions | CheckMergeReady, CheckDeployment, CheckAlertResolution | Added as internal actions from/to same state | CREATED |
| AlertCycle has integration declarations | 6 integrations wired to actions | spawn_sre, begin_cicd, check_merge_ready, check_deployment, check_alert_resolution, report_outcome | CREATED |
| Open action removes sre_agent_id | WASM sets it, not webhook code | Params: monitor_id, alert_payload, report_channel_entity_id, report_thread_id | VERIFIED |
| Webhook trigger is ONE-ONE | Creates one entity, dispatches one action | trigger.rs: create WebhookEvent -> dispatch Received -> return | VERIFIED |
| webhooks.rs disconnected | No mod/use references in compilation | main.rs: mod webhooks removed, startup.rs: webhook router removed | VERIFIED |
| ADR-0005 written | Documents decision and consequences | Created with Context, Decision (4 sections), Consequences | CREATED |
| agents.md written | Entity-first rule, trigger boundary, anti-patterns | Created with tables, examples, reference links | CREATED |
| Architect Reviewer agent | .claude/agents/ with review rules and PASS/FAIL format | Created at .claude/agents/temper-native-reviewer.md | CREATED |
| PreCommit hook configured | .claude/settings.json with hook command | Created with scripts/check-temper-native.sh reference | CREATED |

## Vision Gap Assessment (Post-Refactor)

| Capability | Pre-Refactor | Post-Refactor | Notes |
|---|---|---|---|
| Single binary deploys | Done | Done | No change |
| OS apps install at boot (8 apps now) | 7 apps | 8 apps (added paw-ingest) | Webhook processing now entity-based |
| Paw, Developer, SRE souls | Done | Done | Developer soul updated with agents.md reference |
| OData API for all entities | Done | Done | WebhookEvent + WebhookRoute added |
| Webhook alert ingestion | Hardcoded Rust | Temper-native | WebhookEvent entity with full audit trail |
| SRE -> Developer -> PR (self-heal) | Manually triggered | WASM-driven | alert_opener spawns SRE, SRE self-reports |
| Full CI/CD closure | Not implemented | WASM modules | cicd_merger, deployment_tracker, alert_verifier |
| Proactive reporting | Hardcoded in Rust | WASM (heal_reporter) | Reports on terminal AlertCycle states |
| Architecture enforcement | None | 4 layers | ADR, agents.md, Architect Reviewer hook, soul governance |
| Webhook audit trail | None | WebhookEvent entity | Every webhook traceable to entities it spawned |

## What Still Needs End-to-End Verification

- [ ] Compile all WASM modules (requires wasm32-unknown-unknown target)
- [ ] Start platform with paw-ingest os-app
- [ ] Create WebhookRoute for DD alerts via OData
- [ ] POST webhook -> WebhookEvent transitions -> AlertCycle created
- [ ] SRE spawned by WASM, triages, dispatches HealComplete
- [ ] CI/CD WASM chain: Fixed -> Merging -> Deploying -> Verifying -> Resolved
- [ ] heal_reporter sends Channel summary
- [ ] PreCommit hook blocks non-Temper-native code

## What Still Doesn't Work (Vision Gaps)

| Capability | Status | What's Needed |
|---|---|---|
| Discord end-to-end | Not re-proven | Fresh proof on this branch |
| Paw orchestrates full flow | Not proven | Paw needs to drive the demo scenario |
| Monitor generation (bootstrap) | Partial | MonitorScan automation incomplete |
| Persistent governed sandbox (Fly) | Specs only | Computer WASM modules needed |
| Autonomous slider | Not started | Cedar policy adjustment mechanism |
| Evolution Agent | Not started | No soul or detection logic |
| Agent-created Temper apps | Not started | Platform capability needed |
| Crash/restart recovery | Wired | Not proven under pressure |

## Artifacts

- ADR-0005: `docs/adrs/0005-temper-native-orchestration.md`
- Agent guide: `agents.md`
- Reviewer agent: `.claude/agents/temper-native-reviewer.md`
- Deprecated reference: `crates/openpaw/src/_webhooks_deprecated.rs`
