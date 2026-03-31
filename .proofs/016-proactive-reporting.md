# Proof Report: 016 — Proactive Reporting

## Date

2026-03-28

## Branch / Commit

- Branch: `feat/openpaw-self-heal-loop-codex`
- Commit: working tree implementation

## Vision Target

This proof targets the `.vision` gap:

- `Paw proactive reporting | ❌ Not implemented`

The specific claim is that the system should not only remediate or fail internally, but also report the outcome back through the conversation channel without a human polling OData.

## What Was Done

- Added a background proactive-reporting path in [`crates/openpaw/src/webhooks.rs`](/Users/seshendranalla/Development/openpaw-codex/crates/openpaw/src/webhooks.rs) that waits for SRE completion and dispatches `Paw.Channel.SendReply`
- Added the proof driver [`scripts/prove_proactive_reporting.py`](/Users/seshendranalla/Development/openpaw-codex/scripts/prove_proactive_reporting.py)

## Flow Diagram

```text
alert webhook with reply routing
    |
    v
SRE auto-spawn
    |
    v
terminal AlertCycle state
    |
    v
build summary
    |
    v
Paw.Channel.SendReply
    |
    v
webhook-backed proof collector receives summary
```

## What Was Proven

- Reply routing context can travel in the webhook payload.
- The system can wait for the remediation attempt to finish and then emit a summary without a human asking for it.
- The proactive reply still works on the failure path, which is important for operational transparency.
- The proactive-reporting webhook path now creates its own monitor-scoped `Issue` and `WorkCycle` instead of reusing an older issue from another monitor.

## Verification Flow

1. Start the daemon with the normal self-heal credentials
2. Run `python3 scripts/prove_proactive_reporting.py`
3. The script:
   - creates a webhook-backed `Channel`
   - triggers an alert webhook with explicit reply routing context
   - waits for a proactive channel reply containing the summary

## Verification Results

- Failure-path proactive reply proof, executed earlier with `WEBHOOK_SECRET=test-webhook-secret`:
  - `python3 scripts/prove_proactive_reporting.py --secret test-webhook-secret --timeout-secs 120`
  - recorded IDs:
    - `channel_id`: `019d32a0-29f8-70d1-b303-8a0f415d2148`
    - `route_id`: `019d32a0-2b1f-7171-b89d-6cdb3dd167ad`
    - `alert_cycle_id`: `019d32a0-2b47-7a52-8a14-20c5ca000a6c`
  - result:
    - webhook alert triggered a SRE auto-spawn
    - SRE failed immediately because `ANTHROPIC_API_KEY` was unresolved in that environment
    - the alert convergence watcher escalated the `AlertCycle` to `Failed`
    - a proactive channel reply was still delivered back through `Paw.Channel.SendReply`
  - reply excerpt:
    - `Open Paw self-heal update`
    - `AlertCycle: 019d32a0-2b47-7a52-8a14-20c5ca000a6c (Failed)`
    - `Diagnosis: SRE agent 019d32a0-2b4b-71c0-bca4-1f1414cdbcd7 ended in Failed: provider=anthropic api key is unresolved secret template: '{secret:anthropic_api_key}'. set tenant secret and retry`
- Corrective run after loading real credentials and tightening monitor-scoped dedupe:
  - `channel_id`: `019d348e-803b-7f80-a66a-0265548a1052`
  - `project_harness_id`: `019d348e-8151-7eb2-91a2-87348f7aaecf`
  - `monitor_id`: `019d348e-8162-7a02-ad4e-df8316b11443`
  - `alert_cycle_id`: `019d348e-816f-7001-b483-c583d6e1f421`
  - `sre_agent_id`: `019d348e-8173-7261-9f6c-5d63089a2294`
  - `issue_id`: `019d348e-f0c3-7b02-bc94-6cad5c0d4c4b`
  - `work_cycle_id`: `019d348f-0f21-7303-9ef4-6cb979b09479`
  - `developer_agent_id`: `019d348f-56a2-7dd3-9ac2-cf7aaf8ce8a8`
  - result:
    - the proactive-reporting path created a fresh `Issue` for the fresh `Monitor`
    - the `WorkCycle` advanced and a `Developer` child was spawned with `max_turns = 80`
    - this corrected run was still in progress when the daemon was cycled to patch bounded reproduction, so it does not replace the earlier terminal reply proof

## How It Works

- The webhook handler resolves a `ReportTarget` from explicit payload fields such as `reply_channel_entity_id` and `reply_thread_id`.
- After SRE reaches a terminal state, the watcher reads the final `AlertCycle`, latest `WorkCycle`, and latest linked `Issue`.
- It composes a plain-text summary and dispatches `Paw.Channel.SendReply` so the message exits through the same governed channel surface that Discord will later use.

## Honest Assessment Against Vision

- Proven:
  - Proactive reporting is no longer only an idea in the vision doc.
  - The failure case is reported explicitly instead of disappearing into internal logs.
  - The proactive-reporting webhook path now creates monitor-scoped PM state instead of broad issue reuse across monitors.
- Not proven by this report:
  - A successful PR-producing remediation followed by a proactive success summary.
  - A corrected post-patch run reaching terminal state and emitting a new reply after the bounded `npm ci` handoff change.
  - A real Discord delivery path.
- Still below vision:
  - This proves the channel-side reporting mechanism and its corrected issue-scoping behavior, not the full human-facing Discord experience.

## Artifacts

- [`crates/openpaw/src/webhooks.rs`](/Users/seshendranalla/Development/openpaw-codex/crates/openpaw/src/webhooks.rs)
- [`scripts/prove_proactive_reporting.py`](/Users/seshendranalla/Development/openpaw-codex/scripts/prove_proactive_reporting.py)
