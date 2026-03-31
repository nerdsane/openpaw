# Proof Report: 018 — Full Demo Loop

## Date

2026-03-27

## Branch / Commit

- Branch: `feat/openpaw-self-heal-loop-codex`
- Commit: working tree implementation

## Vision Target

This is the capstone proof against the full `.vision/001_openpaw_target_vision.md` scenario, especially:

- Human says: `Manage deep-sci-fi for me`
- Paw sets up the project
- monitors fire
- SRE triages
- Developer fixes and opens a PR
- Paw reports back proactively

## What Was Done

- Implemented the code and proof harnesses needed for the full demo chain:
  - Paw-managed project setup
  - webhook alert ingress
  - SRE auto-spawn
  - PM issue creation
  - Developer remediation flow
  - proactive channel reporting

## Flow Diagram

```text
Human on Discord
    |
    v
Paw
    |
    +--> ProjectHarness / Developer / Monitors
    |
synthetic or real alert
    |
    v
SRE
    |
    v
Developer
    |
    v
PR / remediation artifact
    |
    v
Paw proactive report to human
```

## What This Proof Must Ultimately Demonstrate

- The cloud-deployed service behaves like the experience promised in `.vision`.
- The human can kick off management from Discord instead of developer-only infrastructure interfaces.
- The full alert-to-remediation loop works as one coherent product, not as isolated subsystem tests.

## Verification Flow

1. Start the daemon with all demo credentials
2. Send “manage deep-sci-fi” through Discord
3. Trigger a synthetic alert webhook for the managed project
4. Wait for SRE -> Developer -> PR completion
5. Verify the proactive summary arrives back through Discord

## Verification Results

- Not executed in this environment.
- This phase requires both external credentials and a human-operated Discord interaction.

## Honest Assessment Against Vision

- Proven by earlier subsystem work:
  - Webhook ingress exists.
  - SRE auto-spawn exists.
  - PM issue creation path exists.
  - Proactive channel replies exist.
- Not proven by this report:
  - That Paw setup, alert remediation, and proactive reporting all chain together in one real Discord-driven session.
  - That the human-facing interaction quality matches the vision doc.
- Remaining human boundary:
  - This proof should not claim success until a real Discord conversation and a real end-to-end transcript are captured.

## Artifacts

- [`docs/adrs/0003-demo-vision-implementation.md`](/Users/seshendranalla/Development/openpaw-codex/docs/adrs/0003-demo-vision-implementation.md)
- [`crates/openpaw/src/webhooks.rs`](/Users/seshendranalla/Development/openpaw-codex/crates/openpaw/src/webhooks.rs)
- [`scripts/prove_paw_orchestration.py`](/Users/seshendranalla/Development/openpaw-codex/scripts/prove_paw_orchestration.py)
- [`scripts/prove_webhook_to_sre.py`](/Users/seshendranalla/Development/openpaw-codex/scripts/prove_webhook_to_sre.py)
- [`scripts/prove_pm_integration.py`](/Users/seshendranalla/Development/openpaw-codex/scripts/prove_pm_integration.py)
- [`scripts/prove_proactive_reporting.py`](/Users/seshendranalla/Development/openpaw-codex/scripts/prove_proactive_reporting.py)
