# ADR-0004: Platform Upgrade — SRE Rename, Modal Sandboxes, Datadog Monitoring, CI/CD Closure

## Status

Accepted

## Context

Open Paw's self-heal loop is proven but has several gaps versus the vision:

1. **Scout naming** does not reflect the role. The triage agent acts as an SRE — it monitors, triages alerts, coordinates remediation, and tunes monitors. "Scout" undersells this.

2. **E2B sandboxes** were used for proof-of-concept but are not the long-term sandbox provider. Modal provides persistent containers with better governance integration via gRPC, and the team has Modal credentials ready.

3. **Logfire references** exist throughout the codebase, but deep-sci-fi (the demo project) does not use Logfire at all. It needs Datadog instrumentation set up from scratch, and all monitoring/alerting should flow through Datadog.

4. **Monitor bootstrap** is missing. The vision describes Ramp-style granular monitoring (~1 monitor per 75 lines), but no `MonitorScan` entity or automated monitor creation exists.

5. **CI/CD stops at PR creation.** The AlertCycle reaches "Fixed" when a PR is opened, but there is no merge, deployment detection, or post-deploy verification.

6. **Paw orchestration** has not been proven end-to-end. Paw exists as a soul but has never driven the full "manage deep-sci-fi" flow.

## Decisions

### 1. Rename Scout → SRE

The triage agent soul is renamed from "Scout" to "SRE" across the entire codebase — soul file, startup bootstrap, webhook handler, entity specs, proofs, docs, and scripts. The entity field `scout_agent_id` on AlertCycle becomes `sre_agent_id`. Existing Turso data is dropped and re-bootstrapped (dev branch only).

**Rationale**: "SRE" accurately describes the agent's function and aligns with industry terminology.

### 2. Replace E2B with Modal via WASM gRPC integration

Modal uses gRPC/protobuf. The Temper WASM SDK already provides `ctx.connect_call()` for Connect protocol (gRPC-over-HTTP/1.1). The sandbox provisioner WASM module is rewritten to call Modal's gRPC API directly.

If Modal's gRPC endpoint does not support Connect protocol natively, a `grpc_call` host function is added to Temper's `WasmHost` trait, keeping the call within the governed WASM execution pipeline.

**Rationale**: Keeping sandbox provisioning inside the WASM module preserves Cedar policy governance over Computer entity lifecycle actions. A Python bridge would bypass this governance layer.

**Config**: `MODAL_TOKEN_ID` + `MODAL_TOKEN_SECRET` replace `E2B_API_KEY`.

### 3. Switch from Logfire to Datadog

All Logfire references are removed. The Monitor entity's `logfire_query` field becomes `dd_query`. New config fields: `DD_API_KEY`, `DD_APP_KEY`, `DD_SITE` (defaults to `datadoghq.com`).

A new `datadog_query` WASM tool module is created so agents can query DD monitors, events, and metrics. Datadog has a REST API, so `ctx.http_call` works without any Temper host changes.

The webhook handler gains native Datadog webhook format detection and field mapping.

**Rationale**: deep-sci-fi will use Datadog. Building around DD first with a real project is more valuable than maintaining Logfire references for a hypothetical integration.

### 4. Add MonitorScan for Ramp-style monitor bootstrap

A new `MonitorScan` entity in paw-heal tracks automated monitor generation:
- `scan_type`: "bootstrap" (full codebase) or "pr_delta" (changed files only)
- Developer agents create DD monitors via the DD API during project bootstrap
- Each DD monitor is configured to webhook to OpenPaw's `/webhooks/ingest`
- Corresponding OpenPaw Monitor entities are created and linked by `dd_monitor_id`

**Rationale**: This is the core of the Ramp-style self-healing pattern described in the vision. Without it, monitors must be created manually.

### 5. Extend AlertCycle for CI/CD closure

AlertCycle gains new states: `Fixed` → `Merging` → `Deploying` → `Verifying` → `Resolved` (terminal).

After SRE/Developer produce a PR and AlertCycle reaches "Fixed":
1. Poll GitHub Checks API for the PR
2. When checks pass, merge via GitHub API
3. Detect deployment (deep-sci-fi deploys to Vercel + Railway on push to staging/main via GitHub)
4. Wait, then query DD for the original monitor status
5. If resolved: `AlertResolved`. If still firing: `AlertPersists` → Failed.

**Rationale**: The vision explicitly calls for "fully automatic observability-driven loop." Stopping at PR creation leaves the loop open.

### 6. Developer agent sets up DD instrumentation

Since deep-sci-fi has no Datadog integration today, the Developer agent is responsible for adding it:
- `ddtrace` (Python) / `dd-trace` (Node.js) library installation
- `DD_SERVICE`, `DD_ENV`, `DD_VERSION` configuration
- This happens during project bootstrap, before MonitorScan

**Rationale**: The Developer agent should be capable of setting up its own observability, not just consuming it. This matches the vision where agents bootstrap monitors across codebases they manage.

## Consequences

- All existing proof scripts and reports need updating for the Scout → SRE rename
- The WASM sandbox provisioner changes from REST (E2B) to gRPC (Modal), potentially requiring a new Temper host function
- Logfire integration code is removed entirely — if Logfire support is needed later, it would be re-added as a second provider
- AlertCycle's "Fixed" state is no longer terminal, which is a breaking change to the state machine
- The implementation agent must produce an end-to-end proof that the full autonomous loop works, not just compile checks
