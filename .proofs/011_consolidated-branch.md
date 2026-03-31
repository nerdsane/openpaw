# Proof Report: 011 — Consolidated Branch Status

## Date
2026-03-27

## Branch
Integration worktree based on `feat/openpaw-self-heal-loop-codex`, ready to push back onto that branch.

## Scope
This proof covers the consolidated Codex base plus selected Claude improvements, the upstream Temper fix, architectural cleanup, and a fresh post-consolidation verification run.

---

## What Changed In This Consolidation

### 1. Temper fix moved upstream
- The E2B Connect protocol fix was committed in the Temper repo instead of being carried as vendored code:
  - `aa73ef3` `fix: use connect+json for envd protocol`
- Claude's Discord/session hardening changes were also reviewed, committed, and pushed upstream:
  - `fa7275b` `feat: harden discord channel routing and session recovery`
- Open Paw now depends on Temper via git dependencies on `feat/temper-claw`.
- Vendored Temper code is no longer part of Open Paw.

### 2. Open Paw runtime cleanup
- Removed vendored Temper overrides from `Cargo.toml`.
- Moved the default Paw agent configuration out of Rust string literals into [`config/paw_agent_config.json`](/Users/seshendranalla/Development/openpaw-codex-integration-20260327000903/config/paw_agent_config.json).
- Replaced the old startup sleep with an actual readiness handoff before bootstrap.
- Seeded the fallback `Paw` `AgentRoute` during startup before the Discord transport starts.
- Added a basic `POST /webhooks/alerts` ingestion path that opens an `AlertCycle` through OData.
- Added local sandbox health-checking and PID logging as a dev-only helper.
- Removed the duplicate `temper_api_url` state from [`agent.ioa.toml`](/Users/seshendranalla/Development/openpaw-codex-integration-20260327000903/os-apps/paw-agent/specs/agent.ioa.toml).
- Increased `blob_adapter` context buffer to `131072` bytes.

### 3. Cedar policy tightening
- `ProjectHarness.Archive` is no longer broadly allowed.
- `Monitor.Archive` is no longer broadly allowed.
- `WorkCycle.Fail`, `Agent.Fail`, and `Agent.Cancel` are restricted to system/owner-style principals.
- The four-eyes `WorkCycle.Approve` pattern remains intact.

### 4. Claude deltas adopted vs deferred
- Adopted:
  - Upstream Temper Connect content-type fix
  - Upstream Discord route/session recovery hardening
  - `blob_adapter` large-context buffer fix
  - Content-per-file session storage architecture
  - More action-oriented `Paw` soul language
- Deferred:
  - No remaining deferred storage changes from Claude; the main unresolved issue is still E2B execution/output fidelity

---

## What Works Right Now

### 1. Daemon boot, OS app install, souls, Cedar, and WASM registration
- `cargo build` succeeds against upstream Temper.
- `cargo run` succeeds from this branch.
- Startup restores persisted specs from Turso and re-installs Open Paw OS apps.
- Startup registers local WASM modules including:
  - `llm_caller`
  - `tool_runner`
  - `sandbox_provisioner`
  - `context_compactor`
  - `steering_checker`
  - `workspace_restorer`
  - `channel_connect`
  - `route_message`
  - `send_reply`
  - `blob_adapter`
- Recovery proof after a real daemon kill/restart showed the same runtime coming back cleanly.

Artifact:
- Restart log showed `Installed os-app 'paw-agent'`, `Installed os-app 'paw-channels'`, `Installed os-app 'paw-fs'`, `Installed os-app 'paw-harness'`, `Installed os-app 'paw-heal'`, followed by `Registered ... local WASM modules`.

### 2. Declarative fallback Paw route exists and survives restart
- Startup seeds a fallback `AgentRoute` for `Paw` before Discord transport boot.
- The route uses declarative config from `config/paw_agent_config.json`, not a Rust JSON literal.
- The same route remained present after restart.

Artifact:
- `AgentRoute` `019d2d93-1f63-7343-8370-14eb7d19d582`
- `soul_id = "Paw"`
- `agent_config` includes:
  - `model = claude-sonnet-4-6`
  - `provider = anthropic`
  - `tools_enabled = temper_create,temper_get,temper_list,temper_action,spawn_agent,save_memory,read_entity`

### 3. Curl-style multi-turn continuation works
- `Channel.ReceiveMessage` resumes same-thread context instead of starting fresh.
- The follow-up message produced a continuation agent that reused prior session context.
- This was re-proven on the consolidated branch.

Artifact:
- First reply: `REMEMBERED moon-biscuit-42`
- Second reply: `RECALL moon-biscuit-42`
- First agent: `019d2d94-2433-76e3-9632-f78a3e02949e`
- Continuation agent: `019d2d94-2b88-7b91-9fd1-8480704af1be`

### 4. Scout -> Developer self-heal loop works end to end
- The proof script created:
  - `ProjectHarness`
  - `Monitor`
  - `AlertCycle`
  - `Scout` agent
- The `Scout` agent spawned a `Developer` child, the `Developer` fixed the repo, validated it, pushed a branch, and opened a PR.
- The loop ended with `AlertCycle = Fixed`.
- Ownership in the proven flow is:
  - proof driver creates `ProjectHarness`, `Monitor`, and initial `AlertCycle`
  - `Scout` creates the `WorkCycle` and spawns the `Developer`
  - `Developer` reads those workflow entities and updates them, but does not create the harness itself
- The `ProjectHarness` is operational workflow state, not just decorative text:
  - it holds the repo URL, stack, and conventions used to scope remediation work
  - it is linked from the `WorkCycle` and `AlertCycle` path the agents read and update
  - it is not yet a hard-enforced execution substrate or machine image definition

Artifacts:
- `ProjectHarness`: `019d2d94-4399-7313-943c-ab16d026b14b`
- `Monitor`: `019d2d94-43aa-7d21-99c9-36f8c00074c6`
- `AlertCycle`: `019d2d94-43bb-7db2-8a3d-27bfe2e232af`
- `Scout`: `019d2d94-43bf-7192-a3d2-33e3a28c69a3`
- `Developer`: `019d2d94-bdd3-7be3-8bdb-f48c60346262`
- PR: `https://github.com/arni-labs/deep-sci-fi/pull/69`
- Final `AlertCycle` state after restart remained readable as `Fixed`
- A later rerun on the same branch also completed successfully by validating that the issue had already been fixed upstream, without opening a duplicate PR:
  - `ProjectHarness`: `019d2f65-f0a1-7440-a039-1c1a58574379`
  - `Monitor`: `019d2f65-f0b3-75a2-b590-a0c3b8679473`
  - `AlertCycle`: `019d2f65-f0c7-7e21-bd1e-bd58951df169`
  - `Scout`: `019d2f65-f0cc-7141-adb0-44547ce30bed`
  - `Developer`: `019d2f66-5c42-7f71-85e2-80101d53f554`
  - Validation commit URL returned by the loop: `https://github.com/arni-labs/deep-sci-fi/commit/34e7971428df11f0b60791561faf3bd35a7610ee`

### 5. Basic webhook ingestion now exists and works
- `POST /webhooks/alerts` now creates an `AlertCycle` and dispatches `Open`.
- This was verified both before and after restart.

Artifacts:
- Pre-restart webhook-created alert cycle: `019d2d93-4c83-7b03-aac3-e91ea07a38cc`
- Post-restart webhook-created alert cycle: `019d2d9d-59f4-7c41-8d7e-834ddb8c143b`
- Example response:

```json
{"ok":true,"alert_cycle_id":"019d2d9d-59f4-7c41-8d7e-834ddb8c143b","monitor_id":"","status":"Triaging"}
```

### 6. Cedar restrictions are live, not just files on disk
- After restart, a non-admin principal attempting to archive a `ProjectHarness` was denied.

Artifact:

```text
POST /tdata/ProjectHarnesses('019d2d94-4399-7313-943c-ab16d026b14b')/OpenPaw.ProjectHarness.Archive
principal: kind=user, id=proof-user
HTTP 403
AuthorizationDenied: no matching permit policy
```

### 7. Local sandbox path works for the proven remediation flow
- The local sandbox remains the strongest verified developer execution path.
- It supports `bash`, `read`, and `write`.
- Startup now health-checks local sandbox URLs before seeding them.

### 8. E2B provisioning works through upstream Temper
- A real E2B sandbox was provisioned on the consolidated branch via upstream Temper.
- This confirms the Connect content-type fix is on the correct side of the dependency boundary.

Artifacts:
- Agent: `019d2d98-8680-7ec1-a5ba-7ca32812bbbb`
- E2B sandbox id: `itudp0k58utmy3fcrx7hi`
- E2B sandbox url: `https://49983-itudp0k58utmy3fcrx7hi.e2b.app`
- Final agent status: `Completed`
- Final result included `CLONE_OK`

### 9. Content-per-file session storage is now merged and verified
- Session payloads are now stored as TemperFS `File` entities, while `session.jsonl` stores only structural references via `content_file_id`.
- This was verified with a focused local proof and two focused E2B proofs.

Artifacts:
- Local proof agent: `019d2f61-c2e6-7933-914e-06f6e58bbb27`
- Local proof session file: `019d2f61-c341-72e2-bbc5-11e1f4e307c8`
- Local proof content files included:
  - user message file
  - assistant `tool_use` file
  - user `tool_result` file containing `file-backed-proof`
  - final assistant file containing `DONE: file-backed-proof`
- E2B proof agent: `019d2f62-702f-7991-b6b3-480244b4c6c4`
- E2B proof session file: `019d2f62-7211-7863-948b-a94de62d4727`
- E2B manifest proof agent: `019d2f63-3ffd-7ed1-ad57-c0c2a2311001`
- E2B manifest proof session file: `019d2f63-4166-7fa0-b24a-e3489069cbcb`

---

## What Does Not Work, Or Is Not Strongly Proven Enough To Claim

### 1. E2B output capture is still not clean
- Although provisioning and the overall agent loop worked, the rerun showed empty `tool_result` content in the session file.
- A focused E2B proof still stored an empty `tool_result` content file even when the command was `printf e2b-file-backed-proof`.
- A second focused E2B proof created `note.txt` inside the E2B workdir, but the persisted `file_manifest` still came back as `{}`.
- That means the upstream Connect fix is in place, but this branch still has a remaining E2B evidence/capture issue.

Current claim boundary:
- I can honestly claim real E2B provisioning and end-to-end agent completion.
- I cannot honestly claim correct E2B stdout/stderr capture or trustworthy E2B fsync from the current tool path.

### 2. No real external alert provider is wired end to end
- `POST /webhooks/alerts` is basic intake only.
- There is no provider-specific verification, signature validation, or production connector for Logfire/Datadog webhooks.
- The self-heal proof still starts from a synthetic/manual alert payload, not a real external monitor push.

Current claim boundary:
- Manual or scripted alert intake works.
- Autonomous observability-driven intake is not yet proven.

### 3. Discord human-message flow is still not re-proven here
- Discord transport boots and rotates stale channel entities on startup.
- The route/session hardening is upstream in Temper.
- But this consolidation proof did not include a fresh human DM/reply round-trip.

Current claim boundary:
- Channel/session logic is proven through curl.
- A real Discord DM round-trip still needs a live human message test.

### 4. `paw-compute` vision is still mostly unimplemented
- There is still no real persistent `Computer` lifecycle in use.
- No proven provision/sleep/wake/checkpoint/destroy workflow exists for cloud computers.
- The active runtime still passes `sandbox_url` and `workdir` to agents rather than provisioning a first-class machine entity.

### 4.5. `ProjectHarness` is workflow context, not yet hard execution control
- The harness is real state that the proof driver, `Scout`, and `Developer` read and update around repo work.
- It carries repo metadata and conventions that shape prompts and approvals.
- It does not yet enforce tool allowlists, dependency policy, branch policy, or sandbox shape by itself.
- In that sense it is operational, but still lighter than the full "developer lives inside the harness" vision.

### 5. Local sandbox is intentionally dev-only and not isolated
- The local sandbox is a Python HTTP shim over the host machine.
- It is not a VM or container.
- It runs with host user privileges.
- It is useful for local verification, but it is not a production sandbox model.

### 6. CI/CD closure beyond PR creation is still missing
- The proven loop ends at branch push + pull request.
- No merge, deploy, rollback, or post-deploy verification loop is implemented or proven.

### 7. Compaction is built but not re-proven here
- `context_compactor` is present and loaded.
- This proof did not deliberately drive a conversation far enough to trigger compaction on the consolidated branch.

---

## Setup Experience Today

### What an operator needs
Create a real `.env` file, not a broken symlink, with at least:
- `ANTHROPIC_API_KEY`
- `GITHUB_TOKEN`

Optional but used by major flows:
- `E2B_API_KEY`
- `DISCORD_BOT_TOKEN`
- `LOGFIRE_READ_TOKEN`
- `LOGFIRE_WRITE_TOKEN`
- `PAW_TENANT` if you do not want the default tenant

### How to boot
From the repo root:

```bash
set -a
source .env
set +a
cargo run
```

### How operator curls are authorized
- Tenant header: `X-Tenant-Id: default`
- Admin operator header: `x-temper-principal-kind: admin`
- Content type: `application/json`

Example:

```bash
curl -H 'X-Tenant-Id: default' \
  -H 'x-temper-principal-kind: admin' \
  -H 'Accept: application/json' \
  http://127.0.0.1:3467/tdata/AgentRoutes
```

### What startup auto-configures
- OS app install/recovery
- Cedar policy recovery
- local WASM registration
- vault seeding for runtime secrets
- fallback `Paw` route creation
- Discord transport startup if `DISCORD_BOT_TOKEN` is present
- local sandbox startup only when a local `SANDBOX_URL` is actually being used

---

## Sandbox Definition Today

### Local sandbox
- Implementation: [`os-apps/paw-agent/sandbox/local_sandbox.py`](/Users/seshendranalla/Development/openpaw-codex-integration-20260327000903/os-apps/paw-agent/sandbox/local_sandbox.py)
- Shape: Python HTTP service
- Capabilities:
  - run shell commands
  - read files
  - write files
- Isolation:
  - none
  - runs on the host machine as the local user
- Startup behavior:
  - started only for local sandbox URLs
  - health-checked before use
  - documented in code as dev-only

### E2B sandbox
- Provisioned through `sandbox_provisioner`
- Accessed through Temper's envd Connect transport
- Current setup is effectively "give the agent a real remote shell + file API + working directory"
- GitHub token injection is available for authenticated git and `gh` workflows
- There is still no first-class machine image definition owned by Open Paw

### Fsync / workspace sync contract
- Sync is bounded, not a full mirror
- Current defaults from [`agent.ioa.toml`](/Users/seshendranalla/Development/openpaw-codex-integration-20260327000903/os-apps/paw-agent/specs/agent.ioa.toml):
  - `max_sync_file_bytes = 61440`
  - `max_sync_files = 64`
  - `sync_exclude = __pycache__,node_modules,.git,.next,dist,build,target,coverage,venv,.venv`
- This is enough for lightweight evidence and selected artifacts
- It is not a full workspace persistence layer

---

## Current Data Flow

```text
operator curl or Discord
        |
        v
Open Paw daemon
        |
        +--> startup installs OS apps, Cedar, WASM, souls, fallback Paw route
        |
        +--> /tdata OData surface
        |
        +--> /webhooks/alerts
                |
                v
            AlertCycle.Open
                |
                v
           Scout / Paw workflows
                |
                v
             Agent entity
        (llm_caller <-> tool_runner)
                |
                +--> OData entity tools
                |
                +--> TemperFS files / manifests / sessions
                |
                +--> sandbox_provisioner
                        |
                        +--> local sandbox (dev-only host shell)
                        |
                        +--> E2B remote sandbox
                |
                v
          repo clone / edit / validate / push / PR
                |
                v
      WorkCycle / AlertCycle transitions via OData
```

---

## Remaining Gaps Vs The Vision

### Architectural gaps
- No first-class persistent `Computer` workflow from `paw-compute`
- No provider-grade webhook adapters with verification and signatures
- No deploy/rollback loop after PR creation
- `ProjectHarness` is still primarily coordination/governance metadata, not a fully enforced execution envelope

### Operational gaps
- Discord DM round-trip not re-proven in this consolidation
- E2B output capture still needs another pass
- Compaction is loaded but not re-proven under long-session pressure

### Security and governance gaps
- Cedar is meaningfully better than before, but not yet exhaustively tightened
- Local sandbox remains intentionally unsafe outside development

---

## Bottom Line

This consolidated branch is materially better than either earlier state in three important ways:
- the E2B protocol fix now lives upstream in Temper instead of in vendored code
- Open Paw no longer vendors Temper
- startup/runtime structure is cleaner, more declarative, and less race-prone

The strongest end-to-end claim I can make today is:
- Open Paw boots cleanly from this branch
- recovers entities, Cedar policies, and WASM after restart
- supports continuing conversations through channel/session state
- runs a real synthetic-alert self-heal loop to a real GitHub PR
- and can provision a real E2B sandbox through upstream Temper

The most important unresolved gap is:
- E2B execution evidence is still weaker than it should be because output/file-manifest capture is incomplete even after the storage architecture fix
