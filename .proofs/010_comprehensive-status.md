# Proof Report: 010 — Comprehensive Status vs Vision

## Date
2026-03-26

## Branch
`feat/openpaw-self-heal-loop-codex` — 19 commits

---

## WHAT WORKS (verified with proof artifacts or direct end-to-end runs)

### 1. Daemon boots and installs the Open Paw OS apps
- Single daemon boots with Temper embedded and serves OData at `/tdata`
- Startup installs 7 OS apps:
  - `paw-agent`
  - `paw-channels`
  - `paw-fs`
  - `paw-pm`
  - `paw-compute`
  - `paw-harness`
  - `paw-heal`
- Startup restores specs from Turso, reloads WASM modules, and recovers Cedar policies
- Startup bootstraps 3 souls: `Paw`, `Developer`, `SRE`
- **VERIFIED**: Proofs 006-008 all ran successfully against `http://localhost:3467/tdata`

### 2. Developer agent can provision a real E2B sandbox and run a real clone flow
- `sandbox_provisioner` can provision through the E2B API when `E2B_API_KEY` is set
- `tool_runner` can execute bash in the E2B sandbox
- GitHub token injection works for HTTPS clone
- Session-tree continuation after tool execution works without the old oversized-context failure
- **VERIFIED**: [.proofs/006-repo-clone-e2b.md](/Users/seshendranalla/Development/openpaw-codex/.proofs/006-repo-clone-e2b.md)
- **Artifact**: Developer cloned `deep-sci-fi` in E2B and returned `CLONE_OK`

### 3. Curl-style continuing conversation works
- `Channel.ReceiveMessage` creates or resumes a `ChannelSession`
- Same-thread follow-up creates a continuation agent instead of starting from blank context
- The continuation agent reuses the same `session_file_id`
- `ChannelSession` rebinds to the continuation agent
- **VERIFIED**: [.proofs/008-channel-continuation.md](/Users/seshendranalla/Development/openpaw-codex/.proofs/008-channel-continuation.md)
- **Artifact**: Second message correctly recalled `moon-biscuit-42`

### 4. SRE -> Developer self-heal loop works in a manually-triggered form
- `ProjectHarness`, `Monitor`, `AlertCycle`, and `WorkCycle` all exist as working entities
- A `SRE` agent can read the workflow entities, spawn one `Developer` child, wait for it, and close the loop
- The `Developer` child can clone, edit, validate, push, and open a PR
- `WorkCycle` reaches `Complete`
- `AlertCycle` reaches `Fixed`
- **VERIFIED**: [.proofs/007-self-heal-loop.md](/Users/seshendranalla/Development/openpaw-codex/.proofs/007-self-heal-loop.md)
- **Artifact**: PR `https://github.com/arni-labs/deep-sci-fi/pull/68`

### 5. GitHub push + PR creation work in the proven remediation path
- The remediation proof did not stop at diagnosis
- It pushed a branch and created a real GitHub pull request
- **VERIFIED**: Proof 007
- **Important qualifier**: This was proven in the local sandbox path, not in E2B

### 6. Bounded sandbox -> Paw FS / TemperFS sync works
- Agent provisioning creates conversation/session/workspace storage
- `tool_runner` appends full tool results to the session tree
- The agent entity keeps only a compact marker instead of storing giant raw tool payloads
- Sandbox files sync back into the manifest and file entities on a best-effort basis
- **VERIFIED**: Proof 006
- **Important qualifier**: this is bounded, partial fsync, not a full workspace mirror

### 7. Cedar-governed entity/action model is real
- Cedar policy files exist for agent, channels, harness, heal, compute, and FS areas
- Startup recovers Cedar policies from persistent storage
- WASM callbacks rely on policy-governed actions like `SandboxReady`, `HandleToolResults`, `Resume`, `Open`, `HealComplete`
- **VERIFIED**: policy recovery and entity flows are present in the running system used for proofs
- **Important qualifier**: some policies are still broad/permissive; see limitations below

### 8. Local sandbox execution works as the main developer fallback
- Local sandbox auto-starts when the script exists and no explicit E2B-only default is forced
- Supports:
  - file read
  - file write
  - shell command execution
- **VERIFIED**: Proof 007 used local sandbox end to end for SRE and Developer

---

## WHAT DOES NOT WORK (broken, unimplemented, or not proven enough to claim)

### 1. Real external webhook ingestion for alerts
- **Status**: NOT IMPLEMENTED
- [webhooks.rs](/Users/seshendranalla/Development/openpaw-codex/crates/openpaw/src/webhooks.rs) is still a placeholder
- No proven `POST /webhooks/ingest` alert intake path
- No proven GitHub event ingestion path

### 2. Real observability-driven alert opening
- **Status**: NOT IMPLEMENTED
- The self-heal proof used a synthetic alert payload plus explicit OData actions
- The script manually called:
  - `Monitor.AlertFired`
  - `AlertCycle.Open`
- **Current**: the loop is manually triggerable
- **Not current**: true autonomous production-alert intake

### 3. `MonitorScan` from the architecture
- **Status**: NOT IMPLEMENTED
- The docs mention `MonitorScan`
- No `MonitorScan` spec exists under `os-apps/paw-heal/specs/`

### 4. Persistent cloud computers via `paw-compute`
- **Status**: NOT IMPLEMENTED
- `paw-compute` currently has specs and Cedar policy only
- There are no compute WASM modules for:
  - provision
  - checkpoint
  - sleep
  - wake
  - destroy
- The proven flows still use `sandbox_url`, not real `Computer` entities

### 5. Full autonomous CI/CD closure
- **Status**: NOT IMPLEMENTED
- No proof of:
  - PR merge
  - deploy
  - post-deploy verification
  - rollback
  - webhook-driven closure after deployment
- **Current**: Open Paw can produce a PR
- **Not current**: the whole deploy/observe/reclose loop

### 6. Discord DM end-to-end
- **Status**: NOT RE-PROVEN HERE
- Startup can spawn Discord transport if `DISCORD_BOT_TOKEN` is set
- The current proofs intentionally used curl/webhook-style channel flows instead of Discord
- **Current**: Discord is wired
- **Not current**: a documented DM -> agent -> reply proof on this branch

### 7. Full E2B self-heal loop
- **Status**: NOT PROVEN
- E2B clone is proven
- Full SRE -> Developer -> validate -> push -> PR on E2B is not separately proven here
- **Current**: E2B is good enough for clone and basic bash execution
- **Not current**: confidence that the whole remediation path is solid on E2B

### 8. Crash/restart resilience for in-flight work
- **Status**: NOT PROVEN
- Startup does restore registry data, reload WASM, and recover Cedar policies
- But there is no proof here of:
  - killing the daemon mid-workflow
  - restarting
  - resuming an in-flight agent/session/heal loop correctly

### 9. Agent compaction under real long-context pressure
- **Status**: NOT PROVEN
- `Compacting` state and `context_compactor` exist
- No proof here that a long conversation crossed the threshold, compacted, and resumed successfully

### 10. Paw soul driving the whole loop as the operator-facing orchestrator
- **Status**: NOT PROVEN
- `Paw` soul is bootstrapped and intended as the default project-manager soul
- But the proven artifacts on this branch focus on:
  - direct Agent API
  - channel continuation with a custom proof prompt
  - SRE/Developer self-heal
- **Current**: `Paw` exists and is wired
- **Not current**: a full proof showing Paw itself creates the harness/heal workflow and kicks off the repair

---

## SETUP EXPERIENCE (how someone sets up Open Paw today)

### What you need
1. Clone the repo and run from the worktree you want to test
2. Create a gitignored `.env` with the relevant credentials
3. Start the daemon
4. Use curl/OData or the proof scripts to drive it

### The useful environment variables
The current code reads these from `.env` if present:
```env
ANTHROPIC_API_KEY=
E2B_API_KEY=
GITHUB_TOKEN=
LOGFIRE_READ_TOKEN=
LOGFIRE_WRITE_TOKEN=
DISCORD_BOT_TOKEN=
FLY_API_TOKEN=
TEMPER_API_KEY=
TEMPER_VAULT_KEY=
TURSO_URL=
TURSO_AUTH_TOKEN=
PORT=3467
PAW_TENANT=default
```

### What auto-configures on startup
- Turso/libSQL storage
- registry restore from Turso
- secrets vault
- local sandbox auto-start, when applicable
- local blob store auto-start, when applicable
- OS-app installation
- local WASM build/register if needed
- Cedar recovery
- skill restore
- soul bootstrap for `Paw`, `Developer`, `SRE`
- optional Discord startup

### What is still rough
- This is not a polished one-command install story yet
- The proof environment reused a real machine with the needed toolchain already installed
- Clean-room onboarding from an empty laptop was not re-proved here

### How to drive it today
- Direct OData/curl
- Proof scripts:
  - [prove_self_heal_loop.py](/Users/seshendranalla/Development/openpaw-codex/scripts/prove_self_heal_loop.py)
  - [prove_channel_continuation.py](/Users/seshendranalla/Development/openpaw-codex/scripts/prove_channel_continuation.py)

---

## SANDBOX DEFINITION

### Current local sandbox
- Implemented by [local_sandbox.py](/Users/seshendranalla/Development/openpaw-codex/os-apps/paw-agent/sandbox/local_sandbox.py)
- Endpoints:
  - `GET /health`
  - `GET /v1/fs/file?path=...`
  - `PUT /v1/fs/file?path=...`
  - `POST /v1/processes/run`
- The local sandbox runs commands directly on the host with `subprocess.run(..., shell=True)`
- The local sandbox uses a working directory such as `/tmp/paw-sandbox`

### What the local sandbox really is
- It is not a VM
- It is not a container image
- It is not isolated
- It is a Python HTTP server exposing host file operations and host shell execution

### Current E2B sandbox
- Provisioned by [sandbox_provisioner/src/lib.rs](/Users/seshendranalla/Development/openpaw-codex/os-apps/paw-agent/wasm/sandbox_provisioner/src/lib.rs)
- Priority order:
  - explicit `sandbox_url`
  - configured `sandbox_url`
  - E2B API using `e2b_api_key`
- Current E2B create request uses `"secure": false`
- Open Paw then talks directly to the envd URL

### What the agent actually receives
In the proven flows, the agent effectively gets:
- `sandbox_url`
- `workdir`
- `tools_enabled`

That is the real contract today.

### What is not yet true
- No proven persistent machine image
- No proven project-prepared computer with dependencies already installed
- No proven `Computer` entity lifecycle in front of the Developer agent

### Vision vs reality
- **Vision**: persistent cloud developer computer with checkpoint/sleep/wake and project-aware setup
- **Reality**: local host-backed sandbox for the proven repair loop, plus one proven E2B clone path

---

## HARNESS DEFINITION

### Current state
`paw-harness` is generic, not hardcoded to deep-sci-fi.

It has:
- `ProjectHarness`
- `WorkCycle`

`ProjectHarness` stores:
- `repo_url`
- `tech_stack`
- `conventions`
- `last_activated_at`

`WorkCycle` tracks:
- plan
- work
- testing
- review
- completion/failure

### What the deep-sci-fi proof actually did
The proof created a deep-sci-fi-specific harness instance by configuring:
- `repo_url = https://github.com/arni-labs/deep-sci-fi.git`
- `tech_stack = Next.js frontend, Python backend`
- conventions describing focused fixes and validation in `platform/`

So:
- the harness model is generic
- the proof instance was deep-sci-fi-specific

### What is not yet implemented
- No hard enforcement that Developer must always obey harness state before acting
- No proof that Paw creates harnesses autonomously in the main operator flow
- No proof that conventions are deeply enforced as policy gates rather than prompt guidance

### Vision vs reality
- **Vision**: harness meaningfully governs the development workflow
- **Reality**: harness/work-cycle entities are real and were used in the self-heal proof, but governance is still relatively light

---

## CEDAR AUTHORIZATION

### What exists
The Cedar layer is real.

Relevant policy areas include:
- agent
- channels
- harness
- heal
- compute
- FS

### What is good
- `paw-agent` and `paw-channels` have explicit callback/module-related permissions
- The running daemon used Cedar-recovered policy state during the proven flows
- This is not “Cedar in theory only”

### What is still weak
- `paw-harness` and `paw-heal` policies are broad and permissive
- Current verification tracking is bootstrap-oriented, not a full authorization audit
- There is no proof here of nuanced multi-role separation like strict planner/approver/implementer boundaries across the whole system

### Vision vs reality
- **Vision**: Cedar is the firm governance boundary for who can do what
- **Reality**: Cedar is present and active, but some domains still need much tighter policy design and proof coverage

---

## FSYNC (Sandbox ↔ Paw FS / TemperFS)

### Current state
- Agent provisioning attempts to create:
  - workspace
  - conversation file
  - file manifest
  - session file
  - session leaf
- After tool execution:
  - full tool results go into the session tree
  - agent entity keeps a compact marker
  - sandbox files sync back on a best-effort basis

### Current limits
- `max_sync_file_bytes = 61440`
- `max_sync_files = 64`
- excludes:
  - `__pycache__`
  - `node_modules`
  - `.git`
  - `.next`
  - `dist`
  - `build`
  - `target`
  - `coverage`
  - `venv`
  - `.venv`

### What is proven
- Proof 006 confirmed manifest entries for synced repo files
- Proof 006 also confirmed the post-tool follow-up turn still worked using the stored session tree

### What is not implemented
- full workspace mirroring guarantee
- selective changed-files-only sync
- strong guarantee that large repo workspaces are fully represented in Paw FS after a turn

### Vision vs reality
- **Vision**: sandbox work transparently persists and survives environment churn
- **Reality**: bounded, partial, best-effort sync is real; complete workspace fidelity is not

---

## CI/CD + ALERT PIPELINE

### What works
- `Monitor` and `AlertCycle` entities exist and work
- `SRE` can triage and spawn a `Developer`
- `Developer` can fix code, validate, push, and open a PR
- `WorkCycle` and `AlertCycle` can be brought to successful terminal states
- **VERIFIED**: Proof 007

### What does not work automatically
- no real external alert webhook ingestion
- no automatic `AlertCycle` creation from production observability
- no `MonitorScan`
- no proven merge/deploy/post-deploy verification loop
- no proven GitHub-event-based closeout loop

### The honest current state
- **Vision**: observability system fires -> SRE triages -> Developer fixes -> PR -> human merge -> deploy -> monitoring updates and confirms recovery
- **Reality**: synthetic alert + manual OData kick-off -> SRE -> Developer -> PR

---

## SUMMARY TABLE

| Feature | Vision | Reality | Gap |
|---------|--------|---------|-----|
| Daemon boots | ✅ | ✅ Works | None |
| OS-app install + soul bootstrap | ✅ | ✅ Works | None |
| Curl/thread continuity | ✅ | ✅ Proven | None |
| E2B clone flow | ✅ | ✅ Proven | None for clone milestone |
| Local sandbox remediation | ✅ | ✅ Proven | No isolation |
| SRE -> Developer -> PR | ✅ | ✅ Proven with synthetic trigger | Still manually triggered |
| Git push + PR | ✅ | ✅ Proven in local sandbox | E2B remediation not proven |
| Paw FS session persistence | ✅ | ✅ Proven in bounded form | Partial fsync only |
| Discord connected | ✅ | ⚠️ Wired, not re-proven | Needs direct DM proof |
| Real Datadog query in healing loop | ✅ | ⚠️ Tool exists, not proven in loop | Need real query proof |
| Webhook alert ingestion | ✅ | ❌ Not implemented | Need real webhook path |
| `MonitorScan` | ✅ | ❌ Missing | Need spec + implementation |
| Persistent `Computer` entity for Developer | ✅ | ❌ Spec-only | Need compute WASM + proof |
| Cedar governance | ✅ | ⚠️ Real but uneven | Tighten harness/heal policy design |
| Crash/restart recovery | ✅ | ⚠️ Wired, not proven | Need restart proof |
| Context compaction | ✅ | ⚠️ Wired, not proven | Need long-context proof |
| Full autonomous closed loop | ✅ | ❌ Not there yet | Major integration work |
