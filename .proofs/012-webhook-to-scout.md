# Proof Report: 012 — Webhook-Triggered Scout Auto-Spawn

## Date
2026-03-28

## Branch / Commit
`feat/openpaw-self-heal-cc`

## What Was Done

Extended the webhook ingestion endpoint to automatically spawn a Scout agent when an AlertCycle is created from an incoming alert. The Scout is configured with the alert context, project harness details, and repo URL extracted from the webhook payload.

### Code Changes
- `crates/openpaw/src/webhooks.rs` — Added `spawn_scout_for_alert()` function that:
  - Finds the active Scout soul via OData query
  - Extracts `project_harness_id` and `repo_url` from alert payload
  - Creates an Agent entity, Configures it with Scout soul + alert context, Provisions it
  - Returns the Scout agent ID in the webhook response
- `scripts/prove_webhook_to_scout.py` — Proof script with --wait/--no-wait modes

## Verification Flow

1. Start daemon (`cargo run`)
2. Create ProjectHarness + Monitor via OData
3. POST webhook payload to `/webhooks/ingest`
4. Verify Scout agent was auto-spawned (status != Created, soul_id == Scout)
5. Optionally wait for Scout to complete and verify AlertCycle terminal state

## Verification Results

| Step | Expected | Actual | Status |
|------|----------|--------|--------|
| Create prerequisites | Harness + Monitor Active | Created and activated | PASS |
| Webhook auto-spawns Scout | HTTP 201, Scout ID in response | 201, Scout `019d3290-a244-7a93-bf12-130f0e395117` | PASS |
| Scout configured correctly | Status=Provisioning, soul_id=Scout | Provisioning, Scout | PASS |
| Scout completes triage | AlertCycle reaches terminal state | Scout Completed, AlertCycle Fixed, WorkCycle Complete | PASS |

## What Worked
- Webhook handler correctly finds the Scout soul via OData query
- Scout agent is created, configured with alert context, and provisioned automatically
- Alert payload fields (project_harness_id, repo_url) are passed through to Scout's task message
- Sandbox URL is correctly derived from API base URL

## Limitations
- Full Scout completion with LLM requires ANTHROPIC_API_KEY and significant time (~5-15min)
- Scout model is hardcoded to `claude-sonnet-4-20250514` — should eventually be configurable

## What Still Doesn't Work
- Paw orchestration (Phase 3)
- Proactive reporting after self-heal completes (Phase 6)

## Artifacts
- Proof script: `scripts/prove_webhook_to_scout.py`
- Scout Agent ID: `019d3290-a244-7a93-bf12-130f0e395117`
- AlertCycle ID: `019d3290-a23a-7b32-961f-cb5ada216f85`
