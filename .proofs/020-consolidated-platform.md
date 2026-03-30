# Proof Report: 020 - Consolidated Platform

## Date

2026-03-30

## Branch / Commit

- Branch: `feat/openpaw-self-heal-loop-codex`
- Base commit at start of this proof: `ab3a356f`
- Proof scope includes additional local changes in:
  - [startup.rs](/Users/seshendranalla/Development/openpaw-codex/crates/openpaw/src/startup.rs)
  - [alert_cycle.cedar](/Users/seshendranalla/Development/openpaw-codex/os-apps/paw-heal/policies/alert_cycle.cedar)
  - [001_openpaw_target_vision.md](/Users/seshendranalla/Development/openpaw-codex/.vision/001_openpaw_target_vision.md)

## Consolidation Scope

This proof covers the principal-level consolidation requested after comparing this branch with `origin/feat/dd-monitoring-claude` at `e6377efa`.

The consolidation work included:

- Porting the Modal bridge sandbox provisioner pattern into [sandbox_provisioner/src/lib.rs](/Users/seshendranalla/Development/openpaw-codex/os-apps/paw-agent/wasm/sandbox_provisioner/src/lib.rs) using [modal_sandbox.py](/Users/seshendranalla/Development/openpaw-codex/os-apps/paw-agent/sandbox/modal_sandbox.py)
- Porting the CI/CD closure flow into [webhooks.rs](/Users/seshendranalla/Development/openpaw-codex/crates/openpaw/src/webhooks.rs):
  - GitHub checks polling
  - squash merge
  - deployment polling
  - post-deploy Datadog verification
- Expanding Datadog recovery handling so recovery webhooks can resolve active cycles beyond the narrow happy path
- Merging stronger Datadog instrumentation/bootstrap guidance into [developer.md](/Users/seshendranalla/Development/openpaw-codex/souls/developer.md)
- Removing the remaining E2B runtime paths from:
  - [config.rs](/Users/seshendranalla/Development/openpaw-codex/crates/openpaw/src/config.rs)
  - [startup.rs](/Users/seshendranalla/Development/openpaw-codex/crates/openpaw/src/startup.rs)
  - [sandbox_provisioner/src/lib.rs](/Users/seshendranalla/Development/openpaw-codex/os-apps/paw-agent/wasm/sandbox_provisioner/src/lib.rs)
  - [tool_runner/src/lib.rs](/Users/seshendranalla/Development/openpaw-codex/os-apps/paw-agent/wasm/tool_runner/src/lib.rs)
  - [workspace_restorer/src/lib.rs](/Users/seshendranalla/Development/openpaw-codex/os-apps/paw-agent/wasm/workspace_restorer/src/lib.rs)
- Decomposing the `tool_runner` monolith into separate Rust modules:
  - [datadog.rs](/Users/seshendranalla/Development/openpaw-codex/os-apps/paw-agent/wasm/tool_runner/src/datadog.rs)
  - [entity_tools.rs](/Users/seshendranalla/Development/openpaw-codex/os-apps/paw-agent/wasm/tool_runner/src/entity_tools.rs)
- Fixing architectural gaps:
  - `dd_site` is now a required `String` with default `datadoghq.com`
  - startup now clears stale persisted `sandbox_url` values when Modal bridge mode is active
  - alert-cycle Cedar policy now authorizes `BeginMerge`, `MergeComplete`, `DeployDetected`, `AlertResolved`, and `AlertPersists`
- Cleaning the final stale repo-wide `E2B` references from [001_openpaw_target_vision.md](/Users/seshendranalla/Development/openpaw-codex/.vision/001_openpaw_target_vision.md)

## Architectural Notes

- I did not split `datadog_query` and entity tools into separate WASM crates in this pass.
- Instead, I moved them into separate Rust modules inside `tool_runner` and kept the dispatcher thin.
- Reason: the current platform runtime does not yet provide a clean cross-WASM delegation mechanism for one tool integration to invoke another governed WASM module as a first-class tool boundary.
- This keeps the code separated into independent compilation units now, while preserving a straightforward path to future extraction into standalone WASM integrations.

Audits requested in the task that did not require code changes:

- `agent.ioa.toml` does not currently contain a duplicate `temper_api_url`
- `model.csdl.xml` already exposes `Pause` and `Archive` on `Monitor`

## Pre-flight Checks

Executed from repo root:

```bash
cargo check
cargo build --target wasm32-unknown-unknown --release --manifest-path os-apps/paw-agent/wasm/sandbox_provisioner/Cargo.toml
cargo build --target wasm32-unknown-unknown --release --manifest-path os-apps/paw-agent/wasm/tool_runner/Cargo.toml
cargo build --target wasm32-unknown-unknown --release --manifest-path os-apps/paw-agent/wasm/workspace_restorer/Cargo.toml
cargo build
rg -n --hidden -g '!target' -g '!.git' -g '!.proofs' 'e2b|E2B' .
rg -n --hidden -g '!target' -g '!.git' -g '!.proofs' 'logfire|Logfire|scout|Scout' .
```

Results:

- `cargo check` -> passed
- release build for `sandbox_provisioner` -> passed
- release build for `tool_runner` -> passed
- release build for `workspace_restorer` -> passed
- `cargo build` -> passed
- `rg ... 'e2b|E2B'` -> clean after updating stale vision text
- `rg ... 'logfire|Logfire|scout|Scout'` -> clean

Important runtime packaging note:

- The daemon loads WASM directly from each module's `target/wasm32-unknown-unknown/release/` directory via [find_wasm_binary()](/Users/seshendranalla/Development/openpaw-codex/crates/openpaw/src/startup.rs#L991)
- No extra manual copy step was required for the running daemon to consume the rebuilt modules

## Boot Verification

Daemon booted successfully with:

```bash
WEBHOOK_SECRET=proof-secret cargo run
```

Observed startup log lines:

```text
2026-03-30T18:14:55.039816Z  INFO openpaw::startup: Phase 1: Initializing storage...
2026-03-30T18:14:55.044986Z  INFO openpaw::startup: Phase 2: Building spec registry...
2026-03-30T18:14:55.045002Z  INFO openpaw::startup: Phase 3: Loading OS apps from ./os-apps/...
2026-03-30T18:14:55.087444Z  INFO openpaw::startup: Phase 4: Assembling platform state...
2026-03-30T18:15:01.615166Z  INFO openpaw::startup: Modal bridge: http://127.0.0.1:3478
2026-03-30T18:15:01.616205Z  INFO openpaw::startup: Phase 6: Installing Paw OS apps...
```

The source labels confirm the full nine-phase boot sequence:

```text
Phase 1: Initializing storage
Phase 2: Building spec registry
Phase 3: Loading OS apps
Phase 4: Assembling platform state
Phase 5: Configuring secrets vault
Phase 6: Installing Paw OS apps
Phase 7: Recovery
Phase 8: Bootstrap complete
Phase 9: Starting server
```

The live process also showed the key rebuilt modules being loaded:

```text
module=sandbox_provisioner
module=tool_runner
module=workspace_restorer
```

Service document verification:

```bash
curl -i \
  -H 'Accept: application/json' \
  -H 'X-Tenant-Id: default' \
  -H 'X-Temper-Principal-Kind: admin' \
  http://127.0.0.1:3467/tdata
```

Response:

```text
HTTP 200
{"@odata.context":"$metadata","value":[ ... "MonitorScans" ... "Monitors" ... "ProjectHarnesses" ... "WorkCycles" ... ]}
```

Active souls at boot:

```json
{"Id":"019d3fea-1474-70c2-9c7c-82199e48da11","Name":"Paw","Status":"Active"}
{"Id":"019d3fea-1499-7300-951d-8b0023b6720b","Name":"Developer","Status":"Active"}
{"Id":"019d3fea-14ba-7d63-abb0-ad3838ca1abe","Name":"SRE","Status":"Active"}
```

## Entity Lifecycle Verification

Verified via OData with tenant headers and admin principal headers.

Created and exercised:

- `ProjectHarness` -> `019d3feb-ace4-7da0-9827-516bb09974f9`
- `Monitor` -> `019d3feb-acf4-71f3-8543-e9bff63b883e`
- `AlertCycle` -> `019d3feb-ad05-76f1-8bf3-5f6d6de67eaa`
- `WorkCycle` -> `019d3feb-ad0e-7820-85ab-00c0e9b737f7`

Verified outcomes:

- monitor could be created, configured, and activated
- `AlertFired` created or linked an `AlertCycle`
- `AlertCycle` opened and progressed to `Triaging`
- `ProjectHarness` and `WorkCycle` state machines were writable through OData actions

## Webhook Verification

### Datadog-format alert ingestion

Verified:

- POST `/webhooks/ingest` with a Datadog-shaped payload -> accepted
- duplicate POST of the same payload -> deduped
- invalid HMAC with `WEBHOOK_SECRET=proof-secret` -> rejected with `401`

Bad-HMAC response:

```json
{"accepted":false,"error":"webhook signature mismatch"}
```

Concrete monitor from webhook run:

- Monitor: `019d3ff6-5516-79f1-aa3c-e01858c0e0db`
- AlertCycle: `019d3ff6-5539-79c0-b640-8b15ce96fd35`

Monitor snapshot:

```json
{
  "Id":"019d3ff6-5516-79f1-aa3c-e01858c0e0db",
  "Status":"Active",
  "dd_query":"sum(last_5m):avg:ci.install.failures{service:deep-sci-fi-platform} > 0",
  "dd_monitor_id":"dd-proof-20260330181641",
  "alert_count":1
}
```

### Datadog recovery handling

I hit a real bug during this proof:

- recovery webhooks initially failed with `AuthorizationDenied`
- root cause: Cedar policy allowed `HealComplete` and `TuneComplete` but not the new closure actions and recovery actions

After updating [alert_cycle.cedar](/Users/seshendranalla/Development/openpaw-codex/os-apps/paw-heal/policies/alert_cycle.cedar), recovery succeeded.

Concrete recovery artifact:

- AlertCycle: `019d3fee-688c-71e0-8cc1-00cebd60ee1d`
- final state after recovery webhook: `Resolved`

## Modal Sandbox Verification

### Bridge health

```bash
curl -sS http://127.0.0.1:3478/health
```

Response:

```json
{"status":"ok","provider":"modal","bridge_url":"http://127.0.0.1:3478","sandboxes":5}
```

### Direct bridge provisioning

Verified direct Modal sandbox creation through the local bridge API.

Representative artifact:

- sandbox id: `sb-g9rjT3YPHHzbvXtwJh4Y3I`
- returned `sandbox_url`: real `*.w.modal.host`

### Agent-driven provisioning and bash execution

Verified that an agent provisions a Modal sandbox and executes a command inside it.

Concrete artifact:

- Agent: `019d3ff5-9440-7b23-9d86-bb60dd8ab8c7`
- sandbox URL: real `*.w.modal.host`
- bash result: `The current working directory is /tmp/modal-bash-proof`

## Self-Heal Loop Verification

## Historical terminal proof on this branch

The earlier proof in [019-modal-full-platform-loop.md](/Users/seshendranalla/Development/openpaw-codex/.proofs/019-modal-full-platform-loop.md) already demonstrated a terminal human -> Paw -> webhook -> SRE -> Developer -> workflow closure -> proactive reply loop on this branch.

That proof reached terminal entities:

- AlertCycle `019d3f49-fa29-7f52-9561-9139b1619914` -> `Fixed`
- WorkCycle `019d3f4a-9a35-70b0-a394-feebf3ab1b8a` -> `Complete`
- SRE `019d3f49-fa2d-7840-9b6d-d556ed2c0ced` -> `Completed`
- Developer `019d3f4a-f7ae-7f23-8409-292607a9f014` -> `Completed`

## Fresh post-consolidation rerun

I reran the human-emulation path after the consolidation changes.

Human message:

```text
Manage deep-sci-fi for me. For this proof run, do only the minimal managed-project setup: create or reuse the harness and monitoring metadata, then reply once setup is ready. Do not start an exploratory developer investigation before alerts arrive. The repo is https://github.com/arni-labs/deep-sci-fi.git.
```

Observed artifacts from the rerun:

- Paw: `019d3ff5-e14c-7f92-a022-6de53056e590` -> `Completed`
- SRE: `019d3ff6-553e-7ef0-94dd-30e94d7cd01f` -> `Completed`
- Developer: `019d3ff7-6413-73c3-992f-c9ba894543b8` -> `Cancelled`
- AlertCycle: `019d3ff6-5539-79c0-b640-8b15ce96fd35` -> `Fixed`
- WorkCycle: `019d3ff7-1939-7fb2-80f1-6d2bcd5e3161` -> `Complete`
- Issue: `019d3ff6-e66e-76f3-ab82-e986988283d1` -> `Triage`

Important observation:

- Paw, SRE, and Developer all used real Modal-hosted sandbox URLs during this rerun
- the stale `http://127.0.0.1:3478` sandbox short-circuit seen earlier no longer appeared after the startup fix

Representative agent sandbox URLs:

```text
Paw:       https://ta-01kmzzbrxayyhqv69aqqxcc8ng-8080-etkcn3yokegamawkfryu1dts4.w.modal.host
Developer: https://ta-01kmzzcnr710fdrf8vzyma4xnd-8080-x56cz81xh9js7dgfn95ijmev4.w.modal.host
```

Developer activity observed live from entity state and daemon logs:

- cloned `https://github.com/arni-labs/deep-sci-fi.git`
- reproduced the `npm ci` failure path
- executed bounded lockfile repair flow
- ran additional git and npm commands inside the Modal sandbox

Governing workflow evidence from the rerun:

- `Approve` moved `WorkCycle 019d3ff7-1939-7fb2-80f1-6d2bcd5e3161` to `Complete` at `2026-03-30T18:23:07.607211Z`
- `HealComplete` moved `AlertCycle 019d3ff6-5539-79c0-b640-8b15ce96fd35` to `Fixed` at `2026-03-30T18:23:14.625388Z`
- `FinalizeResult` moved `SRE 019d3ff6-553e-7ef0-94dd-30e94d7cd01f` to `Completed` at `2026-03-30T18:24:46.367165Z`
- the linked issue description was updated to record the automated lockfile repair and fixed status
- SRE final summary explicitly recorded:
  - `ALERT_CYCLE_STATUS=Fixed`
  - `WORK_CYCLE_STATUS=Complete`
  - `PR_URL=` (empty, not captured in this rerun)

Current proof cutoff assessment:

- The rerun clearly progressed past setup, triage, issue creation, work-cycle creation, Developer spawn, sandbox provisioning, and active bash execution in Modal.
- The governed workflow entities reached successful terminal outcomes for the remediation path itself: `AlertCycle=Fixed`, `WorkCycle=Complete`, and `SRE=Completed`.
- The Developer agent did not end in `Completed`; it was `Cancelled` after the workflow converged.
- This rerun is strong evidence that the post-consolidation remediation path still closes successfully on governed entities while using Modal sandboxes.
- The remaining gap in this rerun is not state-machine closure; it is artifact completeness. The SRE summary explicitly says the fix completed but `PR_URL` was not captured.

## Current Architecture Diagram

```text
Human / Channel Thread
        |
        v
 /webhooks/ingest
        |
        v
   Monitor (Datadog metadata)
        |
        v
   AlertCycle ------------------------------+
        |                                   |
        v                                   |
      SRE agent                             |
        |                                   |
        +--> Issue + WorkCycle              |
        |                                   |
        +--> Developer agent                |
                |                           |
                v                           |
        Modal sandbox via local bridge      |
                |                           |
                +--> repo clone / bash / fix|
                |                           |
                +--> PR URL ----------------+
                                            |
                                            v
                             CI/CD closure in webhooks.rs
                           (checks -> merge -> deploy -> DD verify)
                                            |
                                            v
                                AlertResolved / AlertPersists
                                            |
                                            v
                              proactive reply back to thread
```

## Honest Assessment

- Proven by execution in this proof:
  - repo-wide E2B removal is complete
  - build and release compilation succeed
  - daemon boots and serves OData
  - core entities can be created and driven through OData
  - Datadog-style ingest, dedup, and HMAC rejection work
  - Datadog recovery now works after the Cedar fix
  - Modal bridge health, direct provisioning, and agent bash execution work
- Proven by prior executed proof on the same branch:
  - the end-to-end human -> Paw -> SRE -> Developer -> workflow closure loop can reach terminal success
- Proven again in this fresh rerun:
  - the post-consolidation self-heal loop reached `AlertCycle=Fixed`, `WorkCycle=Complete`, and `SRE=Completed` with real Modal execution
- Still imperfect in this fresh rerun:
  - the Developer agent ended `Cancelled` rather than `Completed`
  - no `PR_URL` was captured, so CI/CD closure from a remediation PR was not exercised in this run
- Code-reviewed rather than separately runtime-proven in this proof:
  - CI/CD closure watcher logic in [webhooks.rs](/Users/seshendranalla/Development/openpaw-codex/crates/openpaw/src/webhooks.rs) for merge -> deployment -> Datadog resolution
  - the modularized `tool_runner` decomposition as an architectural improvement, since it is still shipping as one WASM binary rather than independent governed modules

## Files Central To This Consolidation

- [config.rs](/Users/seshendranalla/Development/openpaw-codex/crates/openpaw/src/config.rs)
- [startup.rs](/Users/seshendranalla/Development/openpaw-codex/crates/openpaw/src/startup.rs)
- [webhooks.rs](/Users/seshendranalla/Development/openpaw-codex/crates/openpaw/src/webhooks.rs)
- [agent.ioa.toml](/Users/seshendranalla/Development/openpaw-codex/os-apps/paw-agent/specs/agent.ioa.toml)
- [sandbox_provisioner/src/lib.rs](/Users/seshendranalla/Development/openpaw-codex/os-apps/paw-agent/wasm/sandbox_provisioner/src/lib.rs)
- [tool_runner/src/lib.rs](/Users/seshendranalla/Development/openpaw-codex/os-apps/paw-agent/wasm/tool_runner/src/lib.rs)
- [tool_runner/src/datadog.rs](/Users/seshendranalla/Development/openpaw-codex/os-apps/paw-agent/wasm/tool_runner/src/datadog.rs)
- [tool_runner/src/entity_tools.rs](/Users/seshendranalla/Development/openpaw-codex/os-apps/paw-agent/wasm/tool_runner/src/entity_tools.rs)
- [workspace_restorer/src/lib.rs](/Users/seshendranalla/Development/openpaw-codex/os-apps/paw-agent/wasm/workspace_restorer/src/lib.rs)
- [modal_sandbox.py](/Users/seshendranalla/Development/openpaw-codex/os-apps/paw-agent/sandbox/modal_sandbox.py)
- [alert_cycle.cedar](/Users/seshendranalla/Development/openpaw-codex/os-apps/paw-heal/policies/alert_cycle.cedar)
- [developer.md](/Users/seshendranalla/Development/openpaw-codex/souls/developer.md)
