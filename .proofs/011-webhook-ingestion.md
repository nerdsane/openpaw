# Proof Report: 011 — Webhook Ingestion

## Date

2026-03-27

## Branch / Commit

- Branch: `feat/openpaw-self-heal-loop-codex`
- Commit: working tree implementation

## Vision Target

This proof targets the `.vision` gap:

- `Webhook alert ingestion | ❌ Placeholder | webhooks.rs is empty`

The concrete claim being tested is: OpenPaw can accept a real external webhook, translate it into governed Temper state, and do so safely with signature checks and duplicate suppression.

## What Was Done

- Added a real `POST /webhooks/ingest` endpoint in [`crates/openpaw/src/webhooks.rs`](/Users/seshendranalla/Development/openpaw-codex/crates/openpaw/src/webhooks.rs)
- Wired webhook routing into the daemon in [`crates/openpaw/src/startup.rs`](/Users/seshendranalla/Development/openpaw-codex/crates/openpaw/src/startup.rs)
- Added `WEBHOOK_SECRET` support in [`crates/openpaw/src/config.rs`](/Users/seshendranalla/Development/openpaw-codex/crates/openpaw/src/config.rs)
- Added the autonomous proof driver [`scripts/prove_webhook_ingestion.py`](/Users/seshendranalla/Development/openpaw-codex/scripts/prove_webhook_ingestion.py)

## Flow Diagram

```text
synthetic webhook sender
        |
        v
POST /webhooks/ingest
        |
        +--> verify HMAC if WEBHOOK_SECRET is configured
        |
        +--> resolve/create Monitor
        |
        +--> open AlertCycle unless payload is duplicate
        |
        +--> for pull_request.merged:
                resolve WorkCycle and approve it
```

## What Was Proven

- A real HTTP webhook can create governed `Monitor` and `AlertCycle` state.
- A duplicate webhook payload does not create a second `AlertCycle`.
- A bad signature is rejected with `401`.
- A synthetic `pull_request.merged` payload can advance a reviewing `WorkCycle` to completion.

## How It Works

- The webhook handler canonicalizes the incoming JSON payload so deduplication is based on stable content rather than formatting differences.
- For alert-like events, it resolves a `Monitor` by `dd_monitor_id` or creates one, emits `Monitor.AlertFired`, and opens an `AlertCycle`.
- For GitHub merge events, it resolves the target `WorkCycle` by explicit `work_cycle_id` or PR URL and dispatches the harness approval action.
- Internal state changes are done through the same OData surface the rest of the platform uses, so this is not a side-channel bypass around Temper governance.

## Verification Flow

1. Start the daemon with `WEBHOOK_SECRET=test-webhook-secret cargo run -p openpaw`
2. Run `python3 scripts/prove_webhook_ingestion.py --secret test-webhook-secret`
3. The script:
   - creates a `ProjectHarness` and `Monitor`
   - POSTs a synthetic alert webhook
   - verifies `AlertCycle` creation and `alert_count` increment
   - creates a reviewing `WorkCycle`
   - POSTs a synthetic `pull_request.merged` webhook and verifies the `WorkCycle` completes
   - POSTs a bad HMAC signature and expects `401`
   - POSTs the same alert body again and verifies no duplicate `AlertCycle` is created

## Verification Results

- Local phase 1 proof succeeded on `2026-03-27` with `WEBHOOK_SECRET=test-webhook-secret`.
- `scripts/prove_webhook_ingestion.py` created:
  - `ProjectHarness`: `019d3296-c87f-7511-848d-3ce3c90bbd64`
  - `Monitor`: `019d3296-c890-7f30-9920-da911a0df64b`
  - `AlertCycle`: `019d3296-c8f4-75d0-bbf5-ad1987f0190a`
  - merge-verification `WorkCycle`: `019d3296-c977-7cb0-8715-e51da4d3d802`
- The alert webhook returned `alert_opened`.
- The GitHub merged webhook returned `work_cycle_completed`.
- The bad HMAC request returned `401` with `webhook signature mismatch`.
- Reposting the same alert body returned `duplicate_alert` and reused the existing `AlertCycle`.

Exact script summary:

```json
{
  "project_harness_id": "019d3296-c87f-7511-848d-3ce3c90bbd64",
  "monitor_id": "019d3296-c890-7f30-9920-da911a0df64b",
  "alert_cycle_id": "019d3296-c8f4-75d0-bbf5-ad1987f0190a",
  "merge_work_cycle_id": "019d3296-c977-7cb0-8715-e51da4d3d802",
  "alert_response": {
    "accepted": true,
    "outcome": "alert_opened",
    "duplicate": false
  },
  "merge_response": {
    "accepted": true,
    "outcome": "work_cycle_completed"
  },
  "bad_signature_response": {
    "accepted": false,
    "error": "webhook signature mismatch"
  },
  "duplicate_response": {
    "accepted": true,
    "outcome": "duplicate_alert",
    "duplicate": true
  }
}
```

## Honest Assessment Against Vision

- Proven:
  - Webhook ingress is no longer a placeholder.
  - The ingress path writes real governed state and enforces basic safety checks.
- Not proven here:
  - A live Logfire or GitHub sender reaching the endpoint over the public internet.
  - End-to-end remediation after ingestion. That belongs to later proofs.
- Known limitation:
  - This proof uses synthetic payloads, so it proves the platform contract, not third-party webhook compatibility edge cases.

## What Worked

- Real external HTTP ingress is now translated back into governed OData actions.
- Duplicate alert payloads are detected before a new `AlertCycle` is opened.
- GitHub merge payloads can complete a reviewing `WorkCycle`.
- Optional HMAC verification is supported through `WEBHOOK_SECRET`.

## Limitations

- GitHub merge handling assumes the webhook provides a `work_cycle_id` or a resolvable PR URL.
- The proof uses synthetic payloads, not a live Logfire or GitHub webhook sender.

## Artifacts

- [`crates/openpaw/src/webhooks.rs`](/Users/seshendranalla/Development/openpaw-codex/crates/openpaw/src/webhooks.rs)
- [`scripts/prove_webhook_ingestion.py`](/Users/seshendranalla/Development/openpaw-codex/scripts/prove_webhook_ingestion.py)
- [`docs/adrs/0003-demo-vision-implementation.md`](/Users/seshendranalla/Development/openpaw-codex/docs/adrs/0003-demo-vision-implementation.md)
