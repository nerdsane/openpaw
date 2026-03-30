# Proof Report: 012 — Webhook to SRE Auto-Spawn

## Date

2026-03-28

## Branch / Commit

- Branch: `feat/openpaw-self-heal-loop-codex`
- Commit: working tree implementation

## Vision Target

This proof targets the `.vision` row:

- `SRE → Developer → PR (self-heal) | ✅ Proven | Manually triggered with synthetic alert`

Specifically, this phase proves the missing trigger path: the loop should begin from a webhook-created `AlertCycle`, not only from a manual OData setup.

## What Was Done

- Extended webhook ingestion so real alert payloads can create/configure/provision a SRE agent automatically when project context is available
- Added the autonomous proof driver [`scripts/prove_webhook_to_sre.py`](/Users/seshendranalla/Development/openpaw-codex/scripts/prove_webhook_to_sre.py)

## Flow Diagram

```text
alert webhook
    |
    v
/webhooks/ingest
    |
    v
AlertCycle.Open
    |
    v
auto-spawn SRE
    |
    +--> classify / create PM state / possibly spawn Developer
    |
    v
terminal AlertCycle state
```

## What Was Proven

- A webhook can auto-create and provision a SRE agent.
- The auto-spawned SRE can create PM state from the webhook path itself: a fresh monitor-scoped `Issue`, a `WorkCycle`, and a governed `Developer` child.
- The webhook handoff now provisions that `Developer` with `max_turns = 80` and an explicit bounded lockfile-recovery path.
- If SRE fails before closing the loop, the platform still converges the `AlertCycle` to `Failed` instead of leaving it stuck in `Triaging`.

## Verification Flow

1. Start the daemon with the normal self-heal credentials plus `WEBHOOK_SECRET` if signature checking is enabled
2. Run `python3 scripts/prove_webhook_to_sre.py`
3. The script:
   - creates a `ProjectHarness`
   - POSTs a synthetic alert webhook
   - waits for the auto-spawned SRE agent to reach a terminal state
   - verifies the `AlertCycle` reaches `Fixed`, `Tuned`, or `Failed`
   - captures any child `Developer` agents and `WorkCycle`s

## Verification Results

- Failure-path convergence proof, executed earlier with `WEBHOOK_SECRET=test-webhook-secret`:
  - `python3 scripts/prove_webhook_to_sre.py --secret test-webhook-secret --timeout-ms 60000`
  - recorded IDs:
    - `project_harness_id`: `019d32a0-0da8-7482-9bfc-4a3f021d2b50`
    - `monitor_id`: `019d32a0-0de2-7eb0-889b-0c2d2a057335`
    - `alert_cycle_id`: `019d32a0-0e0b-7cf2-8061-1c051f544200`
    - `sre_agent_id`: `019d32a0-0e50-70a0-a573-3dddf7baa668`
  - result:
    - webhook alert created a real `Monitor`
    - webhook alert auto-created and provisioned a SRE `Agent`
    - SRE reached terminal `Failed`
    - the convergence watcher escalated the `AlertCycle` to terminal `Failed`
- Corrective run after loading real credentials and tightening the SRE prompt:
  - `project_harness_id`: `019d348e-8033-7121-aa32-7da0ad61a9f9`
  - `monitor_id`: `019d348e-808b-74e1-807f-0318474ff64e`
  - `alert_cycle_id`: `019d348e-80c4-7d51-a058-5198665825e7`
  - `sre_agent_id`: `019d348e-80dd-7e10-9257-f2a0f6ed3550`
  - `issue_id`: `019d348e-e3f6-75e2-9c63-51ae35d8cf04`
  - `work_cycle_id`: `019d348f-0db1-77d0-87e5-26f745ef00ab`
  - `developer_agent_id`: `019d348f-601d-7dc1-8932-88628aab323f`
  - result:
    - the webhook path created a fresh `Issue` for the fresh `Monitor` instead of reusing an older alert's issue
    - the `WorkCycle` advanced to `InProgress`
    - the `Developer` child was spawned with `max_turns = 80`
- Targeted bounded-reproduction verification on the latest build:
  - `project_harness_id`: `019d3491-73bb-7802-95d0-4ce99ad53e96`
  - `alert_cycle_id`: `019d3491-744a-7550-bf2a-c1e8b32c7112`
  - `sre_agent_id`: `019d3491-7462-78d2-b5b0-0ef0ff3fbc9e`
  - `issue_id`: `019d3491-f24c-7431-a0a8-e16b2dbbc642`
  - `work_cycle_id`: `019d3492-18dd-79f1-acbe-ddc1c989c456`
  - `developer_agent_id`: `019d3492-5dfb-7a02-969b-147e28c0324c`
  - verified directly from the spawned `Developer` prompt:
    - `max_turns = 80`
    - reproduction uses `timeout 120 npm ci`
    - fallback uses `timeout 120 npm install --package-lock-only --ignore-scripts --no-fund --no-audit`

## What Worked

- Webhook-created alert cycles now attempt SRE auto-spawn instead of stopping at record creation.
- The webhook path passes concrete workflow IDs and remediation context into SRE.
- The webhook path now creates monitor-scoped PM issues instead of collapsing multiple monitors onto one active issue.
- The `Developer` child now receives a materially stronger remediation brief: 80 turns, bounded reproduction, and bounded lockfile refresh.
- Failed SRE startups still converge the `AlertCycle` to `Failed` automatically with a diagnosis copied from the agent failure.

## How It Works

- The webhook path resolves the `ProjectHarness` context, creates/configures an `Agent` with the `SRE` soul, and dispatches `OpenPaw.Provision`.
- A background watcher waits for the SRE agent to hit a terminal state.
- If SRE reaches `Failed` or `Cancelled` while the `AlertCycle` is still `Triaging`, the watcher dispatches `AlertCycle.Escalate` with the agent failure message as diagnosis.

## Limitations

- Successful end-to-end remediation still depends on external model and GitHub credentials.
- If project context is missing from the webhook payload and no matching harness can be found, the alert is still opened but SRE auto-spawn is skipped.

## Honest Assessment Against Vision

- Proven:
  - Webhook-triggered SRE startup works.
  - The webhook path can create PM-visible remediation state: `Issue`, `WorkCycle`, and `Developer`.
  - Monitor-scoped issue dedupe now behaves correctly on fresh runs.
  - The system still fails closed for the alert state machine instead of hanging forever on a dead SRE.
- Not proven by this report:
  - A successful SRE → Developer → PR chain from the webhook path with real external credentials on this branch.
  - A webhook-path run reaching terminal `Fixed` after the new bounded `npm ci` handoff.
- Still below vision:
  - This is now a stronger trigger-and-handoff proof, but it is still not the full self-heal demo promised in `.vision`.

## Artifacts

- [`crates/openpaw/src/webhooks.rs`](/Users/seshendranalla/Development/openpaw-codex/crates/openpaw/src/webhooks.rs)
- [`scripts/prove_webhook_to_sre.py`](/Users/seshendranalla/Development/openpaw-codex/scripts/prove_webhook_to_sre.py)
