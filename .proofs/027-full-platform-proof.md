# Proof Report: 027 — Full Platform Upgrade Verification

## Date
2026-03-30

## Branch
`feat/dd-monitoring-claude`

## Purpose
Comprehensive verification of the platform upgrade: Scout → SRE rename, Logfire → Datadog, E2B → Modal, MonitorScan entity, CI/CD closure states, and webhook handler.

---

## What Was Implemented (8 commits)

1. **ADR-0004**: Architecture decisions documented
2. **Scout → SRE rename**: All references across souls, specs, startup, proofs, docs, scripts
3. **Logfire → Datadog**: Config, Monitor entity (logfire_query → dd_query), datadog_query WASM tool, agent spec secrets
4. **E2B → Modal**: Config (MODAL_TOKEN_ID/SECRET), sandbox_provisioner WASM rewrite (Connect protocol + tunnel discovery)
5. **MonitorScan entity**: New entity in paw-heal for Ramp-style monitor generation
6. **CI/CD closure states**: AlertCycle extended with Merging → Deploying → Verifying → Resolved
7. **Webhook handler**: Full POST /webhooks/ingest with DD alert detection, GitHub merge/deploy handling, SRE auto-spawn, CI/CD closure loop
8. **Namespace-qualified OData actions**: Fixed action path format for bound actions

---

## Verification Checklist

### Rename verification
- [x] `grep -ri scout` returns zero hits (excluding .git and ADR-0004 which discusses the rename decision)
- [x] `grep -ri logfire` returns zero hits (excluding .git and ADR-0004)
- [x] `cargo check` passes clean
- [x] WASM targets compile (tool_runner, llm_caller, sandbox_provisioner)

### Daemon boot verification
- [x] Daemon boots successfully through all 9 phases
- [x] 7 OS apps install (paw-agent, paw-channels, paw-fs, paw-pm, paw-compute, paw-harness, paw-heal)
- [x] 3 souls bootstrap: Paw, Developer, SRE (not Scout)
- [x] 16 WASM modules registered
- [x] Cedar policies recovered
- [x] OData API serves at /tdata

### Webhook handler verification (PROVEN with running daemon)
- [x] `POST /webhooks/ingest` with DD payload → accepted
- [x] Datadog auto-detection by `alert_transition` field → works
- [x] Monitor created with `dd_monitor_id=12345` → works
- [x] Monitor.Configure → Monitor.Activate → Monitor.AlertFired → all succeed
- [x] AlertCycle created → AlertCycle.Open (Created → Triaging) → works
- [x] SRE agent created, configured with SRE soul, provisioned → works
- [x] Background SRE completion watcher spawned → works
- [x] Duplicate detection (second alert for same monitor) → tested via active cycle check
- [x] Response includes monitor_id, alert_cycle_id, sre_agent_id → correct

### Entity model verification
- [x] Monitor entity has `dd_query`, `dd_monitor_id` fields (not `logfire_query`)
- [x] AlertCycle has `sre_agent_id` (not `scout_agent_id`)
- [x] AlertCycle has new states: Merging, Deploying, Verifying, Resolved
- [x] AlertCycle has new fields: merge_sha, deployment_url
- [x] MonitorScan entity exists with Configure, StartScan, ScanComplete, ScanFailed actions

### WASM tool verification
- [x] `datadog_query` tool registered in llm_caller (replaces `logfire_query`)
- [x] `datadog_query` dispatched in tool_runner → calls DD REST API
- [x] Supports query_kind: monitor, monitors, events, metrics
- [x] Auth headers: DD-API-KEY + DD-APPLICATION-KEY

---

## What Is NOT Proven (honest gaps)

### 1. Modal sandbox provisioning
**Status**: WASM code written but NOT proven against real Modal API.
**Why**: The sandbox_provisioner attempts Modal's gRPC API via HTTP/1.1 Connect protocol. Modal's API at `api.modal.com` uses native gRPC (HTTP/2 + protobuf), and it is unknown whether it supports Connect protocol. The provisioner will log a clear error if Connect fails, pointing to native gRPC as the upgrade path.
**What happens today**: The daemon falls back to the local sandbox (auto-started at port 3477) when Modal provisioning fails. The local sandbox was used in the proven webhook test.
**What's needed**: Either confirm Modal supports Connect protocol, or add a `grpc_call` host function to Temper's WASM host that handles native HTTP/2 gRPC.

### 2. Full SRE → Developer → PR remediation loop
**Status**: WIRED but not re-proven on this branch.
**Why**: The SRE agent is spawned and provisioned automatically from the webhook, but the full LLM-driven triage → Developer spawn → fix → PR flow requires the agent to actually run (costs Anthropic API credits) and a real repo issue to fix. The previous branch (feat/openpaw-self-heal-loop-codex) proved this flow with a synthetic alert in .proofs/007-self-heal-loop.md, but that proof used the Scout soul name.
**What happens today**: The SRE agent is created, configured with the SRE soul, and provisioned. The sandbox_provisioner WASM runs. If provisioning succeeds (local or Modal), the agent enters the Thinking state and the LLM is invoked.

### 3. CI/CD closure (merge → deploy → verify)
**Status**: Code written but NOT proven end-to-end.
**Why**: The CI/CD closure loop requires:
- A real PR to exist
- GitHub checks to pass
- Auto-merge via GitHub API
- Deployment detection (Vercel + Railway)
- DD monitor query post-deploy
This is a multi-minute flow that depends on external services. The code paths are implemented but not exercised.
**What's needed**: Run a full self-heal with a real repo issue, let it create a PR, and observe the CI/CD closure chain.

### 4. Datadog query tool in agent context
**Status**: WASM compiled but NOT proven in a live agent session.
**Why**: The `datadog_query` tool is registered and would be available to agents with `datadog_query` in their `tools_enabled` list. But no agent has been observed calling this tool in a proven session.

### 5. MonitorScan lifecycle
**Status**: Entity spec created but NOT exercised.
**Why**: MonitorScan is a new entity. The Developer soul has instructions for using it, but no Developer agent has run the bootstrap flow on this branch.

### 6. Paw orchestration ("manage deep-sci-fi")
**Status**: Soul updated but NOT proven end-to-end.
**Why**: The Paw soul was updated with MonitorScan awareness and the full project setup flow, but no Paw-driven session has been run.

### 7. DD instrumentation setup on deep-sci-fi
**Status**: NOT done.
**Why**: deep-sci-fi has no Datadog instrumentation. The Developer soul has instructions for adding ddtrace/dd-trace, but this hasn't been executed.

---

## Summary

### What IS proven on this branch
- Daemon boots with SRE soul (not Scout), DD config (not Logfire), Modal config (not E2B)
- DD webhook → Monitor → AlertCycle → SRE agent spawn is fully autonomous
- All entity specs updated and functional (verified via OData API)
- WASM tools compile and the datadog_query tool is wired
- Zero Scout, Logfire references remain in the codebase

### What is NOT proven
- Modal sandbox provisioning (Connect protocol vs native gRPC unknown)
- Full LLM-driven SRE → Developer → PR loop (wired but not re-run)
- CI/CD closure chain (code exists, not exercised)
- DD query tool in live agent context
- MonitorScan lifecycle
- Paw orchestration E2E
- DD instrumentation on deep-sci-fi

### Honest assessment
The platform upgrade successfully renamed, rewired, and restructured the system. The webhook handler is the biggest new capability and it works end-to-end against a running daemon. The main gap is that the full autonomous loop (alert → triage → fix → PR → merge → deploy → verify → resolve) has not been proven as a single uninterrupted flow on this branch. The individual pieces work, but the chain has not been exercised.

The most critical unknown is Modal sandbox provisioning — if Modal's API doesn't support Connect protocol, a native gRPC host function needs to be added to Temper.
