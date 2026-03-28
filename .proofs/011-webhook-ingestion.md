# Proof Report: 011 — Webhook Ingestion Endpoint

## Date
2026-03-28

## Branch / Commit
`feat/openpaw-self-heal-cc` (worktree from `feat/openpaw-self-heal-loop-codex`)

## What Was Done

Implemented `POST /webhooks/ingest` endpoint that accepts external webhook payloads (Logfire, Datadog, GitHub) and dispatches them as entity actions via the internal OData API. The webhook handler follows the same pattern as the Discord transport — it's an OData client internally.

### Code Changes
- `crates/openpaw/src/webhooks.rs` — Full implementation replacing placeholder
- `crates/openpaw/src/config.rs` — Added `webhook_secret` field
- `crates/openpaw/src/startup.rs` — Wired `/webhooks` route into axum Router
- `scripts/prove_webhook_ingestion.py` — Proof script

## Verification Flow

1. Start daemon (`cargo run`)
2. Create ProjectHarness + Monitor via OData
3. POST synthetic Logfire payload to `/webhooks/ingest`
4. Verify AlertCycle created in Triaging state
5. Verify Monitor alert_count incremented
6. POST duplicate payload → verify no duplicate AlertCycle
7. POST unknown source → verify 400
8. POST GitHub PR event → verify 200 with message
9. POST missing monitor_id → verify 400

## Verification Results

| Step | Expected | Actual | Status |
|------|----------|--------|--------|
| Create prerequisites | Harness + Monitor Active | Created and activated | PASS |
| Logfire alert → AlertCycle | HTTP 201, alert_cycle_id returned | 201, `019d328d-12be-7442-8fbe-4c195b527792` | PASS |
| AlertCycle in Triaging | Status=Triaging, monitor_id set | Triaging, monitor_id=webhook-proof-monitor-* | PASS |
| Monitor alert_count | >= 1 | 1 | PASS |
| Duplicate prevention | HTTP 200, same alert_cycle_id | 200, same ID returned | PASS |
| Unknown source rejected | HTTP 400 | 400 | PASS |
| GitHub PR event | HTTP 200, handled | 200, "No matching WorkCycle" | PASS |
| Missing monitor_id | HTTP 400 | 400 | PASS |

## What Worked
- Webhook handler correctly translates external payloads to OData entity actions
- Duplicate prevention works (returns existing AlertCycle ID instead of creating new)
- Monitor.AlertFired correctly increments alert_count
- AlertCycle state machine transitions Created → Triaging correctly
- Bad payloads rejected with appropriate HTTP status codes

## What Didn't Work
- Initial attempt had field access issues (OData returns `status` lowercase at root, `Status` capitalized in `fields`). Fixed by checking both locations.

## Limitations
- HMAC signature verification is simple string comparison (not proper HMAC-SHA256). Sufficient for initial integration; should be upgraded for production.
- GitHub PR event handler only logs — doesn't transition WorkCycles yet (would need WorkCycle state machine to support PR-merge-triggered transitions).

## What Still Doesn't Work
- Scout is not auto-spawned on AlertCycle creation (Phase 2)
- No real Logfire/Datadog webhook format parsing (uses generic envelope format)

## Artifacts
- Proof script: `scripts/prove_webhook_ingestion.py`
- Monitor ID: `webhook-proof-monitor-20260328034639`
- AlertCycle ID: `019d328d-12be-7442-8fbe-4c195b527792`
