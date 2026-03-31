# Proof Report: 019 - Modal Full Platform Loop

## Date

2026-03-30

## Branch / Commit

- Branch: `feat/openpaw-self-heal-loop-codex`
- Commit: working tree implementation

## Vision Target

This proof targets the plan requirement to manually emulate a human talking to Paw and then watch the full loop complete on Modal:

- Human says: `Manage deep-sci-fi for me`
- Paw sets up the managed project
- a Datadog-shaped alert arrives
- SRE triages and spawns a Developer
- the remediation loop runs on Modal
- Open Paw reports back proactively into the same thread

## What Was Changed Before The Proof

- Fixed the Modal-backed sandbox file sync path in [tool_runner/src/lib.rs](/Users/seshendranalla/Development/openpaw-codex/os-apps/paw-agent/wasm/tool_runner/src/lib.rs) by replacing the platform-specific `find` + `stat` enumeration with a portable Python walker. This removed the bogus `fsync skip ... read failed (HTTP 404)` churn seen in earlier runs.
- Tightened the Developer repair guidance in [developer.md](/Users/seshendranalla/Development/openpaw-codex/souls/developer.md), [sre.md](/Users/seshendranalla/Development/openpaw-codex/souls/sre.md), and [webhooks.rs](/Users/seshendranalla/Development/openpaw-codex/crates/openpaw/src/webhooks.rs) so the live loop moves directly from a bounded `npm ci` failure to a bounded lockfile repair instead of drifting into repo archaeology.

## Verification Flow

1. Started an isolated daemon on `http://127.0.0.1:3867` with real credentials from `.env` plus a run-specific `WEBHOOK_SECRET`.
2. Ran [`scripts/prove_full_platform_loop.py`](/Users/seshendranalla/Development/openpaw-codex/scripts/prove_full_platform_loop.py) against that daemon:
   - `python3 scripts/prove_full_platform_loop.py --base-url http://127.0.0.1:3867 --secret proof-platform-secret-3867 --setup-timeout-secs 420 --alert-timeout-secs 1800`
3. The proof driver emulated a human by sending `Manage deep-sci-fi for me...` through a webhook-backed Channel thread instead of calling workflow entities directly.
4. The proof driver waited for Paw's setup reply, then posted a Datadog-shaped alert payload to `/webhooks/ingest`.
5. The proof driver waited for:
   - `AlertCycle` terminal state
   - terminal `SRE` completion
   - a proactive reply back into the same thread
6. The proof driver asserted that the actual agent sandbox URLs contained `modal.host`.

## Verification Results

- Base URL: `http://127.0.0.1:3867`
- Thread ID: `thread-20260330150753`
- Channel ID: `019d3f49-7a23-76a2-88f6-33e537b62fb9`
- Route ID: `019d3f49-7b34-7c92-9298-3822c6f3c1f4`
- ProjectHarness: `019d3f49-9aea-7bf3-8420-0d7d5c43958a`
- Monitor: `019d3f49-fa1b-7ac1-9bf3-9ba1132f4f05`
- AlertCycle: `019d3f49-fa29-7f52-9561-9139b1619914` -> `Fixed`
- WorkCycle: `019d3f4a-9a35-70b0-a394-feebf3ab1b8a` -> `Complete`
- Issue: `019d3f4a-7337-75f3-8bb0-a2952eb7279b`
- Paw agent: `019d3f49-7b63-76f1-85ce-16bc0b552b53`
- SRE agent: `019d3f49-fa2d-7840-9b6d-d556ed2c0ced` -> `Completed`
- Developer agent: `019d3f4a-f7ae-7f23-8409-292607a9f014` -> `Completed`
- Modal sandbox URL observed on Paw, SRE, and Developer:
  - `https://ta-01kmzmjb3j45557hmgjg26626m-3877-hz6ryrzd5trgxp3ksn5ow4s8i.w.modal.host`
- Remediation artifact recorded by the loop:
  - `https://github.com/arni-labs/deep-sci-fi/commit/6a1de00`

## What Was Proven

- The human-emulation path worked end to end. Paw received a channel message, created the managed-project state, and replied in-thread with setup confirmation.
- The webhook ingestion path accepted a Datadog-shaped alert payload, created or updated the linked monitor state, opened an `AlertCycle`, and auto-spawned `SRE`.
- The live remediation loop ran on Modal-backed sandboxes, not just the local sandbox path. Paw, SRE, and Developer all carried the same `w.modal.host` sandbox URL in live entity state.
- `SRE` created and managed workflow state correctly:
  - issue creation and triage
  - work cycle creation
  - developer spawn
  - `BeginTesting`
  - `PassTests`
  - `Approve`
  - `HealComplete`
- The Developer loop executed real validation commands inside the Modal-backed environment and concluded that the alert condition was already fixed upstream by commit `6a1de00`.
- The loop closed back to the human thread with a proactive summary message beginning with `Open Paw self-heal update`.

## Human-Facing Transcript Excerpts

Initial human-emulation message:

```text
Manage deep-sci-fi for me. For this proof run, do only the minimal managed-project setup: create or reuse the harness and monitoring metadata, then reply once setup is ready. Do not start an exploratory developer investigation before alerts arrive. The repo is https://github.com/arni-labs/deep-sci-fi.git.
```

Paw setup reply excerpt:

```text
## Setup Complete

I've successfully set up minimal managed-project structure for deep-sci-fi:

ProjectHarness 019d3f49-9aea-7bf3-8420-0d7d5c43958a (Active)
MonitorScan 019d3f49-cc6e-7ce0-9dd7-3ce3aa623c25 (Created)
```

Proactive reply excerpt:

```text
Open Paw self-heal update
AlertCycle: 019d3f49-fa29-7f52-9561-9139b1619914 (Fixed)
SRE: 019d3f49-fa2d-7840-9b6d-d556ed2c0ced
WorkCycle: 019d3f4a-9a35-70b0-a394-feebf3ab1b8a (Complete)
Issue: 019d3f4a-7337-75f3-8bb0-a2952eb7279b
Repo: https://github.com/arni-labs/deep-sci-fi.git
```

## Supporting Evidence

- `WorkCycle` authoritative terminal event:
  - `Approve` at `2026-03-30T15:13:44.462687Z`
- `AlertCycle` authoritative terminal event:
  - `HealComplete` at `2026-03-30T15:13:16.603872Z`
- `SRE` authoritative terminal event:
  - `FinalizeResult` at `2026-03-30T15:14:03.821992Z`
- `Developer` authoritative terminal event:
  - `FinalizeResult` at `2026-03-30T15:13:29.305597Z`

Run logs captured outside the repo:

- daemon log: `/tmp/openpaw-platform-proof-isolated-20260330110729.log`
- proof JSON: `/tmp/openpaw-proof-full-loop-isolated-20260330110729.log`

## Honest Assessment Against The Plan

- Proven:
  - the full human -> Paw -> webhook -> SRE -> Developer -> workflow closure -> proactive reply chain now completes in one run
  - the active loop is running on Modal-backed sandbox URLs
  - the result is durable in governed workflow entities, not just in transient agent text
- Not fully complete against the larger platform-upgrade plan:
  - repo-wide `E2B` cleanup is still unfinished, so this proof should not be read as "the entire codebase has no E2B path left"
  - the remediation artifact in this run is an existing commit URL, not a newly opened PR, because the alert condition had already been fixed upstream before the proof ran
- Quality gap still visible:
  - the proactive reply reached the human thread correctly, but its final `Diagnosis:` line still lagged the final entity state and reported `No diagnosis recorded yet.` The governed workflow converged correctly, but the thread summary can still be improved.

## Artifacts

- [webhooks.rs](/Users/seshendranalla/Development/openpaw-codex/crates/openpaw/src/webhooks.rs)
- [tool_runner/src/lib.rs](/Users/seshendranalla/Development/openpaw-codex/os-apps/paw-agent/wasm/tool_runner/src/lib.rs)
- [developer.md](/Users/seshendranalla/Development/openpaw-codex/souls/developer.md)
- [sre.md](/Users/seshendranalla/Development/openpaw-codex/souls/sre.md)
- [prove_full_platform_loop.py](/Users/seshendranalla/Development/openpaw-codex/scripts/prove_full_platform_loop.py)
- [openpaw_proof_support.py](/Users/seshendranalla/Development/openpaw-codex/scripts/openpaw_proof_support.py)
