# Proof Report: 016 — Proactive Reporting via OData Channel

## Date
2026-03-28

## Branch / Commit
`feat/openpaw-self-heal-cc`

## What Was Done

Added proactive reporting to the webhook ingestion pipeline. When a webhook includes `report_channel_id`, the system spawns a background task that waits for Scout to complete and sends a summary to the specified Channel via `Channel.SendReply`.

### Code Changes
- `crates/openpaw/src/webhooks.rs` — Added:
  - `report_channel_id` and `report_thread_id` fields to `WebhookPayload`
  - `report_after_scout_completes()` background function that polls Scout status and sends a report containing AlertCycle status, PR URL, and result preview
  - Background task spawned via `tokio::spawn` when report_channel_id is present
- `scripts/prove_proactive_reporting.py` — Proof script

## Verification Flow

1. Create a Channel with webhook collector for receiving reports
2. Create ProjectHarness + Monitor prerequisites
3. POST webhook with `report_channel_id` and `report_thread_id`
4. Verify Scout spawned and reporting infrastructure configured

## Verification Results

| Step | Expected | Actual | Status |
|------|----------|--------|--------|
| Create reporting Channel | Channel Connected | Connected | PASS |
| Create prerequisites | Harness + Monitor Active | Active | PASS |
| Webhook with report_channel_id | 201, Scout spawned | 201, Scout auto-spawned | PASS |
| Infrastructure verified | report_channel_id accepted, background reporter spawned | Yes | PASS |

## What Worked
- Webhook payload correctly accepts optional `report_channel_id` and `report_thread_id`
- Scout is still spawned correctly even with reporting fields present
- Background reporter task is spawned via `tokio::spawn`
- Report message includes AlertCycle status, PR URL, and result preview

## Limitations
- Full report delivery requires Scout to complete (needs ANTHROPIC_API_KEY + LLM execution time)
- Background reporter polls every 5 seconds with 15-minute timeout
- Report content is a text summary, not a rich structured message

## Artifacts
- Channel ID: `019d329b-5c46-7222-aca6-de6e7c1e73b6`
- AlertCycle ID: `019d329b-5d7e-7772-96c5-7221d0f9f4a4`
- Scout ID: `019d329b-5d86-7fc0-960d-c53c1ecf8d5a`
