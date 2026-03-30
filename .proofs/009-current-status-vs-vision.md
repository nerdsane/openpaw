# Proof Report: 009 — Current Open Paw Status vs Vision

## Date
2026-03-26

## Branch
`feat/openpaw-self-heal-loop-codex`

## Purpose
This document is the strict status report for Open Paw as it exists on this branch.

It separates:
- `PROVEN`: I have direct proof artifacts or a concrete successful end-to-end run.
- `WIRED BUT NOT RE-PROVEN`: the code path exists and is plausibly wired, but I do not have a dedicated end-to-end proof artifact for it here.
- `NOT IMPLEMENTED / VISION ONLY`: the architecture or spec exists, but the working implementation is missing or unproven enough that it should not be claimed as working.

This report is based on:
- [.proofs/006-repo-clone-e2b.md](/Users/seshendranalla/Development/openpaw-codex/.proofs/006-repo-clone-e2b.md)
- [.proofs/007-self-heal-loop.md](/Users/seshendranalla/Development/openpaw-codex/.proofs/007-self-heal-loop.md)
- [.proofs/008-channel-continuation.md](/Users/seshendranalla/Development/openpaw-codex/.proofs/008-channel-continuation.md)
- the current code on this branch

## Executive Summary
Open Paw is real and usable today as a Temper-backed daemon with governed entities, Cedar policies, hot-loadable WASM integrations, agent souls, a curl-style channel/session flow, local-sandbox remediation, and one proven E2B clone flow.

The strongest proven path today is:

`curl/OData -> Channel/AgentRoute/ChannelSession or direct Agent API -> SRE/Developer agent loop -> local sandbox or E2B clone path -> repo work -> PR -> WorkCycle/AlertCycle updates`

The biggest gap versus the vision is that the fully automatic observability-driven loop is not actually autonomous yet. The proof used a synthetic alert payload and manual OData actions to open the cycle. Real external webhook ingestion is still a placeholder, `MonitorScan` is missing, and the `paw-compute` persistent computer story is still spec-only.

## What Is Proven Right Now

| Area | Status | Evidence | Notes |
|---|---|---|---|
| Open Paw daemon boots and serves the OData API | `PROVEN` | Proofs 006-008 all ran against `http://localhost:3467/tdata` | This includes OS-app installation, soul bootstrap, and WASM loading as part of the running daemon used for proofs. |
| Developer agent can provision a real E2B sandbox and clone `deep-sci-fi` | `PROVEN` | [.proofs/006-repo-clone-e2b.md](/Users/seshendranalla/Development/openpaw-codex/.proofs/006-repo-clone-e2b.md) | This specifically proved E2B clone, not the full SRE -> Developer repair loop on E2B. |
| Agent session tree survives tool execution without oversized context failure | `PROVEN` | Proof 006 | The compact marker stays on the entity; the full tool results stay in the session tree. |
| File sync from sandbox to Paw/Temper FS works in bounded form | `PROVEN` | Proof 006 | It is deliberately partial and best-effort; see limitations below. |
| Curl-style continuing conversation works through Channel + ChannelSession | `PROVEN` | [.proofs/008-channel-continuation.md](/Users/seshendranalla/Development/openpaw-codex/.proofs/008-channel-continuation.md) | Same thread did not start from blank context. A continuation agent reused the same session tree. |
| Synthetic self-heal loop can go from ProjectHarness + Monitor + AlertCycle to SRE + Developer + PR + WorkCycle complete + AlertCycle fixed | `PROVEN` | [.proofs/007-self-heal-loop.md](/Users/seshendranalla/Development/openpaw-codex/.proofs/007-self-heal-loop.md) | This was a real repo fix and PR, but the alert was synthetic and the sandbox was local. |
| GitHub push + PR creation from the remediation loop | `PROVEN` | Proof 007, PR `#68` | This proves the fix/push/PR part of the loop. |
| SRE and Developer souls are installed and used as governed agents | `PROVEN` | Proof 007 and startup bootstraps | The proof specifically exercised both souls. |

## What Is Wired But Not Strictly Proven Here

| Area | Status | Why it is not marked proven |
|---|---|---|
| Discord transport startup | `WIRED BUT NOT RE-PROVEN` | Startup will spawn Discord if `DISCORD_BOT_TOKEN` is present, but the proof path intentionally avoided Discord and used curl/webhook-style channels instead. |
| Datadog query tool inside agent tool runner | `WIRED BUT NOT RE-PROVEN` | `datadog_query` exists and reads `dd_api_key` plus `dd_app_key`, but this branch does not include a direct proof that an agent successfully queried real Datadog data and used it in a repair loop. |
| Agent compaction flow | `WIRED BUT NOT RE-PROVEN` | The `Compacting` state and `context_compactor` module exist, but I do not have a proof artifact here showing a long context actually compacted and resumed correctly. |
| Restart recovery of policies/WASM/specs/souls | `WIRED BUT NOT RE-PROVEN` | Startup restores registry data from Turso, reloads WASM, recovers Cedar policies, and refreshes souls, but there is no dedicated proof here of a crash/reboot with an in-flight workflow resuming correctly. |
| E2B sandbox use for more than clone | `WIRED BUT NOT RE-PROVEN` | E2B process execution is proven for clone, but not for the full SRE -> Developer -> validate -> push -> PR path. |

## What Is Not Implemented Or Still Vision-Only

| Area | Status | Evidence |
|---|---|---|
| Real external webhook ingestion for alerts and GitHub events | `NOT IMPLEMENTED` | [webhooks.rs](/Users/seshendranalla/Development/openpaw-codex/crates/openpaw/src/webhooks.rs) is still a placeholder. |
| Automatic Datadog/Datadog alert intake that opens cycles without manual OData actions | `NOT IMPLEMENTED` | Proof 007 manually called `Monitor.AlertFired` and `AlertCycle.Open` from the script. |
| `MonitorScan` entity mentioned in the architecture | `NOT IMPLEMENTED` | It appears in the docs, but there is no `MonitorScan` spec under `os-apps/paw-heal/specs/`. |
| Persistent Fly Sprite / cloud computer provisioning for developer agents | `NOT IMPLEMENTED` | `paw-compute` has only specs and policy files; there are no compute WASM modules in `os-apps/paw-compute/`. |
| Full autonomous CI/CD loop after PR creation | `NOT IMPLEMENTED` | There is no proof of PR merge, deploy, post-deploy verification, rollback, or GitHub webhook-driven closure. |
| A fully hardened machine-image-style developer environment | `NOT IMPLEMENTED` | Today the agent gets `sandbox_url` and `workdir`, not a proven persistent `Computer` lifecycle. |

## Operator Setup Experience Today

## What You Need
You need a gitignored `.env` with some subset of these variables:
- `ANTHROPIC_API_KEY`
- `E2B_API_KEY`
- `GITHUB_TOKEN`
- `LOGFIRE_READ_TOKEN`
- `LOGFIRE_WRITE_TOKEN`
- `DISCORD_BOT_TOKEN`
- `FLY_API_TOKEN`
- `TEMPER_API_KEY`
- `TEMPER_VAULT_KEY`
- `TURSO_URL`
- `TURSO_AUTH_TOKEN`
- `PORT`
- `PAW_TENANT`

## What Startup Does
On startup, Open Paw does this:
1. Creates or opens the Turso/libSQL store, defaulting to `~/.local/share/openpaw/paw.db`.
2. Restores the spec registry from Turso.
3. Configures the secrets vault.
4. Loads `.env` via `dotenv`.
5. Seeds configured secrets into the vault for both `default` and the active tenant.
6. Auto-starts the local sandbox if `os-apps/paw-agent/sandbox/local_sandbox.py` exists.
7. Auto-starts the local blob store if `BLOB_ENDPOINT` is not provided.
8. Installs the bundled OS apps:
   - `paw-agent`
   - `paw-channels`
   - `paw-fs`
   - `paw-pm`
   - `paw-compute`
   - `paw-harness`
   - `paw-heal`
9. Builds and registers local WASM modules if missing.
10. Recovers Cedar policies and installed skills from store.
11. Loads persisted WASM modules from store.
12. Bootstraps the `Paw`, `Developer`, and `SRE` souls from `souls/*.md`.
13. Starts Discord if `DISCORD_BOT_TOKEN` is present.
14. Serves the OData API.

## What This Means In Practice
The current setup experience is functional, but not polished:
- It is not a one-command installer for a fresh machine.
- It depends on the local machine having the right Rust, Python, and toolchain setup.
- The proof environment reused a real `.env` and a pre-existing developer machine, not a clean-room onboarding run.

## Secrets, Cedar, And Authorization Tracking

## What Exists
Open Paw is genuinely Cedar-governed at the entity/app level.

Relevant policy files include:
- [agent.cedar](/Users/seshendranalla/Development/openpaw-codex/os-apps/paw-agent/policies/agent.cedar)
- [channels.cedar](/Users/seshendranalla/Development/openpaw-codex/os-apps/paw-channels/policies/channels.cedar)
- [project_harness.cedar](/Users/seshendranalla/Development/openpaw-codex/os-apps/paw-harness/policies/project_harness.cedar)
- [work_cycle.cedar](/Users/seshendranalla/Development/openpaw-codex/os-apps/paw-harness/policies/work_cycle.cedar)
- [monitor.cedar](/Users/seshendranalla/Development/openpaw-codex/os-apps/paw-heal/policies/monitor.cedar)
- [alert_cycle.cedar](/Users/seshendranalla/Development/openpaw-codex/os-apps/paw-heal/policies/alert_cycle.cedar)

## How It Is Tracked
- Policies are installed as part of the OS-app bundle.
- Startup calls `recover_cedar_policies(...)` so the policies are reloaded from persistent storage on boot.
- Startup also persists OS-app verification state into Turso and marks registry verification as completed with a bootstrap-level summary.

## Important Limitation
That verification tracking is not the same as a deep proof that each policy was exercised end to end.

Today it means:
- the OS app was installed
- the registry records it as verified at bootstrap time
- Cedar policies were recovered on boot

It does not mean:
- every policy branch was tested
- every action/resource pair was authorization-audited
- harness/heal tenancy boundaries were deeply validated

## Current Policy Quality
The policy story is uneven:
- `paw-agent` and `paw-channels` have more explicit callback and module-scope permissions.
- `paw-harness` and `paw-heal` are currently broad and permissive. They mostly allow any principal to create/read/update those entities.

That is good enough for the current proofs, but it is not the final governance posture implied by the vision.

## Sandbox Definition: What The Agent Actually Gets

## Current Reality
Today, an agent is not given a rich persistent machine object in the proven flows.
It is given:
- `sandbox_url`
- `workdir`
- tool permissions via `tools_enabled`

That is the real execution contract today.

## Local Sandbox
The local sandbox is defined by [local_sandbox.py](/Users/seshendranalla/Development/openpaw-codex/os-apps/paw-agent/sandbox/local_sandbox.py).

It exposes:
- `GET /health`
- `GET /v1/fs/file?path=...`
- `PUT /v1/fs/file?path=...`
- `POST /v1/processes/run`

What that means:
- the agent can read files
- the agent can write files
- the agent can run shell commands
- the commands run directly on the host via `subprocess.run(..., shell=True)`

Important limitation:
- there is no isolation
- this is not a VM
- this is not containerized
- it uses the host filesystem and host shell
- command timeout is 60 seconds in the local sandbox implementation

## E2B Sandbox
The E2B path is defined by [sandbox_provisioner/src/lib.rs](/Users/seshendranalla/Development/openpaw-codex/os-apps/paw-agent/wasm/sandbox_provisioner/src/lib.rs).

Current behavior:
- if `sandbox_url` is explicitly provided, use it
- else if a configured sandbox URL exists, use that
- else if `e2b_api_key` exists, create an E2B sandbox through the REST API

Important details:
- it currently requests E2B with `"secure": false`
- Open Paw then talks directly to the envd URL
- this path was proven for clone in Proof 006

## What The Agent Can Do In The Sandbox
If the relevant tools are enabled, the agent can use:
- `read`
- `write`
- `edit`
- `bash`
- entity tools like `temper_get`, `temper_list`, `temper_action`, `temper_create`
- `spawn_agent`
- `read_entity`
- `datadog_query`

This is powerful, but it is not yet a strongly sandboxed or tightly capability-scoped developer computer.

## Vision Gap: Computer Entity
The architecture envisions a persistent `Computer` entity with:
- `provider`
- `cpu_cores`
- `memory_gb`
- `storage_gb`
- `base_image`
- `setup_script`
- checkpoint / sleep / wake / destroy lifecycle

That spec exists in [computer.ioa.toml](/Users/seshendranalla/Development/openpaw-codex/os-apps/paw-compute/specs/computer.ioa.toml).

What does not exist today:
- the compute WASM modules
- a proven provisioning flow
- a proof that Developer agents actually use `Computer` entities rather than ad hoc `sandbox_url`

So the persistent cloud-computer story is still design/spec work, not a working capability.

## Paw FS / Temper FS And Fsync: What Actually Happens

## What Is Created
When an agent is provisioned, `sandbox_provisioner` attempts to create:
- a workspace
- a conversation file
- a file manifest file
- a session file
- an initial session leaf

These IDs are then attached to the agent state.

## What The Tool Runner Does
After tool execution:
- full tool results are appended to the session tree if `session_file_id` exists
- the entity state keeps only a compact marker
- sandbox files are synced to Paw/Temper FS on a best-effort basis

## Proven Fsync Behavior
The repo clone proof proved that:
- synced files appear in the manifest
- at least some cloned repo files were uploaded
- the post-tool follow-up turn succeeded using the stored session tree

## Current Fsync Limits
The current defaults are:
- `max_sync_file_bytes = 61440`
- `max_sync_files = 64`
- `sync_exclude = __pycache__,node_modules,.git,.next,dist,build,target,coverage,venv,.venv`

So fsync is intentionally partial.

What that means:
- it is not a full mirror of the repo
- it may skip many files in large workspaces
- sync failures are non-fatal
- the manifest is evidence of some synced files, not a guarantee of full workspace fidelity

## Blob Storage Path
The `paw-fs` blob adapter is real and content-addressable:
- file content is uploaded by hash
- file metadata is tracked on the `File` entity
- the adapter can cache and fetch stream data

However, this is still a relatively low-level capability. The user-facing claim should be:

`Paw FS stores conversation/session/manifests and a bounded subset of workspace files.`

It should not be claimed as:

`Paw FS is a complete, authoritative mirror of the working sandbox filesystem.`

## Harness: Is It Deep-Sci-Fi Specific?

## The Model
No. The harness model is generic.

`ProjectHarness` fields:
- `repo_url`
- `tech_stack`
- `conventions`
- `last_activated_at`

`WorkCycle` fields include:
- `project_harness_id`
- `task_summary`
- `planner_id`
- `approver_id`
- `plan_summary`
- `test_summary`
- `pr_url`
- `error_message`
- `has_plan`
- `tests_passed`

## The Deep-Sci-Fi Instance
The proof created a deep-sci-fi-specific instance by configuring:
- `repo_url = https://github.com/arni-labs/deep-sci-fi.git`
- `tech_stack = Next.js frontend, Python backend`
- `conventions = Prefer focused fixes, validate in platform/, open PR with concrete reproduction notes.`

So:
- the harness entity type is generic
- the proof instance was specific to deep-sci-fi

## Self-Heal / CI-CD / Alerting: What Works And What Does Not

## What Works
This exact flow was proven:
1. Create `ProjectHarness`
2. Create `Monitor`
3. Create `AlertCycle`
4. Configure a `SRE`
5. Open the alert cycle
6. Fire the monitor alert manually
7. Provision the SRE
8. Let SRE spawn one Developer
9. Let Developer fix the repo
10. Validate with real commands
11. Push a branch
12. Open a PR
13. Mark `WorkCycle` complete
14. Mark `AlertCycle` fixed

This is real enough to claim:
- the model works
- the agent loop works
- the repo editing/push/PR path works
- the workflow entities can accurately reflect success

## What Does Not Work Automatically Yet
This exact flow was not proven and should not be claimed:
1. Real Datadog alert fires on its own
2. Open Paw automatically ingests that webhook/event
3. Open Paw automatically opens the `AlertCycle`
4. Open Paw automatically spawns the SRE from that external alert
5. The PR merges automatically
6. Deployment runs automatically
7. Post-deploy verification closes the loop automatically
8. GitHub or observability webhooks feed the final state back in

## The Honest Status Of The Alert Story
Today the alert loop is:
- modeled
- partly wired
- manually triggerable
- proven with a synthetic alert payload

It is not yet:
- externally event-driven end to end
- autonomously closed loop from production telemetry to recovery

## Important Gaps Compared To The Vision
- `webhooks.rs` is still a placeholder.
- `MonitorScan` is referenced in docs but does not exist in `paw-heal`.
- `dd_monitor_id` is still named for Datadog even though the narrative talks about Datadog.
- The proof used a synthetic alert query string, not a real live Datadog incident.

## Continuing Conversation: What Is Actually True
The curl-style conversation proof showed that same-thread follow-up messages do not start from blank context.

But the implementation detail matters:
- the second turn is usually a continuation agent, not the exact same original agent
- the continuity comes from `ChannelSession` rebinding plus reuse of the same `session_file_id`

That is a good and valid design.
It should be described as:

`thread continuity via ChannelSession + continuation agent + shared session tree`

not as:

`the same single agent process stays alive forever in the same thread`

## Important Limitations Compared To The Vision

## Platform / Setup Limitations
- Setup was not re-proved from a pristine machine.
- The environment still depends on an existing developer machine with Rust, Python, and credentials already available.
- If `TEMPER_VAULT_KEY` is missing, secrets are ephemeral and restart persistence is weaker.

## Governance Limitations
- Harness/heal policies are currently broad and permissive.
- OS-app verification tracking is bootstrap-level, not deep policy proof.
- I do not have a dedicated proof of restart/crash recovery for an in-flight agent workflow.

## Sandbox / Compute Limitations
- The proven self-heal flow used the local sandbox, which is not isolated.
- The local sandbox is host shell execution, not a VM.
- The full self-heal flow was not re-proved on E2B.
- The `Computer`/Fly Sprite persistent machine story is not implemented.

## FS / Persistence Limitations
- Fsync is intentionally partial and bounded.
- Paw FS is not yet a guaranteed full mirror of a large repo workspace.
- Sync failures are non-fatal, so a successful agent result does not imply a complete file mirror.

## Alerting / CI-CD Limitations
- No real webhook-driven alert intake yet.
- No automatic sre spawn from external observability.
- No automatic issue creation was part of the proven path.
- No automatic merge/deploy/rollback/post-deploy verification loop was proven.
- No GitHub webhook closeout loop was proven.

## Agent Runtime Limitations
- Compaction code exists but is not proven here.
- Restart recovery of in-flight agent execution is not proven here.
- Discord transport is wired, but the proof path intentionally bypassed it.

## Final Bottom Line
The current system is beyond a mock. It can:
- boot as a real daemon
- run governed agents
- continue threaded conversations
- clone and edit real repos
- push branches and open PRs
- update harness/heal workflow entities to reflect the repair

But it is not yet the full autonomous vision.

The most accurate single-sentence description is:

`Open Paw currently proves a governed, manually-triggered self-heal workflow with real agent execution and real GitHub PR output, but it does not yet prove fully automatic observability ingestion, persistent cloud computers, or a complete deployment feedback loop.`
