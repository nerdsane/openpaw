# Proof Report: 007 — Comprehensive Status vs Vision

## Date
2026-03-26

## Branch
`feat/openpaw-self-heal-loop-claude` — 23 commits

---

## WHAT WORKS (verified with curl, documented with artifacts)

### 1. Daemon boots and installs all OS apps
- Single binary embeds temper-platform via cargo git dependency
- 23 entity types across 6 OS apps, 16 WASM modules
- Turso local SQLite for persistence
- Boots in ~10s, serves OData at `/tdata`
- **VERIFIED**: `cargo run` → entity CRUD works

### 2. Souls bootstrap automatically
- 3 souls created on first boot: Paw, Developer, Scout
- Content uploaded to TemperFS Files
- Published to Active status
- Soul content loaded by llm_caller for personality
- **VERIFIED**: Paw responds with "I'm Paw, your AI project manager"

### 3. Discord Gateway connects
- Bot goes online on Discord
- Channel entity in Connected state
- AgentRoute entity created with soul_id + agent_config
- Webhook listener started for reply delivery
- **VERIFIED**: Gateway URL resolved, Channel.Connected transition logged
- **NOT VERIFIED**: Actually sending a DM and receiving a reply (needs human)

### 4. Auto soul binding + tools configuration
- Retry loop (10 attempts, 2s delay) finds AgentRoute after Discord creates it
- Sets both soul_id (Paw) and full agent_config (7 tools, max_turns 100)
- No manual curl needed — fully automatic on boot
- **VERIFIED**: "Set soul 'Paw' on AgentRoute" in logs, tools confirmed in agent state

### 5. Paw creates entities autonomously via OData tools
- `temper_create` → creates ProjectHarness, AlertCycle, Issue, WorkCycle
- `temper_action` → Configure, Activate, Open, DiagnoseReal, BeginPlanning
- `temper_list` → queries entities
- `save_memory` → persists knowledge across sessions
- **VERIFIED**: Paw creates full self-healing setup in 12 turns (49s):
  - 1 ProjectHarness (Active, deep-sci-fi)
  - 1 AlertCycle (Triaging, Logfire alert)
  - 1 Issue (Backlog)
  - 1 WorkCycle (Planning)

### 6. Session continuity
- Follow-up messages to same thread_id create new agent via Resume
- Previous agent's session context (session_file_id, workspace_id) passed to new agent
- **VERIFIED**: Message 1 → Paw creates entities. Message 2 → Paw creates WorkCycle referencing msg1 context.

### 7. Developer agent executes in local sandbox
- Local sandbox (Python HTTP server) auto-starts when SANDBOX_URL points to localhost
- bash, read, write tools execute real commands on host machine
- Developer agent clones repos, reads code, writes files
- **VERIFIED**: 28-turn investigation of deep-sci-fi (120s):
  - Clones repo via git
  - Reads proposals.py, database.py, models.py
  - Identifies 6 real issues (connection pool, N+1 queries, etc.)
  - Writes diagnosis.md

### 8. Entity state machines
- All entity types have correct state transitions verified via curl:
  - ProjectHarness: Created → Configure → Active → Archived
  - WorkCycle: Planning → Planned → InProgress → Testing → Reviewing → Complete
  - AlertCycle: Created → Open → Triaging → Fixed/Tuned/Escalated
  - Agent: Created → Provisioning → Thinking ↔ Executing → Completed
- **VERIFIED**: Full lifecycle for each entity type

### 9. Content-per-file session architecture (ADR-0003)
- Every conversation turn's content stored as separate TemperFS File
- Session tree is structural manifest only (references, no inline content)
- Files are immutable, never deleted
- **VERIFIED**: 30+ content files created during 28-turn investigation

---

## WHAT DOES NOT WORK (verified as broken or not implemented)

### 1. Discord DM end-to-end
- **Status**: NOT TESTED
- Gateway connects, entities created, but no human has sent a DM
- reply delivery via webhook (send_reply WASM → POST /reply → Discord REST) untested
- **Blocker**: Needs human to send a DM to the bot

### 2. E2B sandbox command execution
- **Status**: BROKEN
- E2B sandbox provisions successfully (sandbox_id, sandbox_url created via API)
- Command execution fails: temper-wasm `connect_call` sends `application/json` but E2B envd requires `application/connect+json`
- **Fix applied** in local temper checkout (via `[patch]` directive) but NOT pushed to remote
- **Also**: E2B envd request body format may need envelope framing (Connect protocol)
- The local sandbox works as a full workaround

### 3. Developer agent pushing PRs to GitHub
- **Status**: NOT TESTED
- Git credential injection implemented (GITHUB_TOKEN auto-injected for git/gh commands in tool_runner)
- Agent can clone repos (verified)
- Agent can commit locally (not tested)
- Agent pushing + creating PR (not tested)
- **Blocker**: Need to test `git push` + `gh pr create` in sandbox

### 4. Scout agent querying real Logfire alerts
- **Status**: NOT TESTED
- `logfire_query` tool exists in both llm_caller (definition) and tool_runner (execution)
- `logfire_read_token` seeded in vault from .env
- Scout soul has Logfire query instructions
- **But**: No test of Scout agent actually querying Logfire and triaging a real alert
- The self-healing demo uses Paw manually creating AlertCycles, not real Logfire webhooks

### 5. Webhook-driven alert flow (Logfire → Scout → Developer)
- **Status**: NOT IMPLEMENTED
- `webhooks.rs` in daemon is a placeholder
- No `POST /webhooks/ingest` endpoint
- No Logfire webhook configuration
- The vision: Logfire fires alert → webhook hits Open Paw → Scout triages → Developer fixes
- **Current**: Paw manually creates AlertCycles via temper_create tool

### 6. Cedar authorization enforcement
- **Status**: DISABLED
- All Cedar policies are permissive (permit all)
- No role separation (planner ≠ approver, implementer ≠ reviewer)
- No tenant isolation enforcement
- **Vision**: Cedar governs who can do what, prevents unauthorized actions

### 7. Server restart resilience
- **Status**: NOT TESTED
- Turso persistence stores trajectories and blobs
- Entity state recovery from Turso on restart not tested
- Souls may need re-bootstrapping after restart (or may persist — unclear)
- **Vision**: Stop daemon, restart, all entities and conversations resume

### 8. Context compaction for long conversations
- **Status**: NOT TESTED
- Compaction spec exists (NeedsCompaction → Compacting → back to Thinking)
- context_compactor WASM exists and was updated for content-per-file
- But compaction hasn't been triggered in any test (conversations end before hitting token limit)
- **Vision**: Long conversations get summarized, context window managed automatically

### 9. Multi-agent collaboration (Scout + Developer)
- **Status**: NOT TESTED
- Paw can create entities (ProjectHarness, AlertCycle, Issue)
- Developer agent can investigate code in sandbox
- But: Paw has NOT been tested spawning a Developer agent (via spawn_agent tool)
- And: Scout agent has NOT been tested triaging alerts and assigning to Developer
- **Vision**: Scout detects → creates Issue → Developer picks up → fixes → PR

### 10. Paw spawning Developer agents
- **Status**: NOT TESTED
- `spawn_agent` tool is in Paw's tools_enabled list
- The tool definition exists in llm_caller
- But: No test of Paw actually calling spawn_agent to create a Developer
- The Developer agent tests were done via direct curl, not Paw-initiated

---

## SETUP EXPERIENCE (how someone deploys Open Paw today)

### What you need:
1. Clone `nerdsane/openpaw` (private repo)
2. Create `.env` file with credentials:
   ```
   ANTHROPIC_API_KEY=sk-ant-oat-...  (OAuth token)
   DISCORD_BOT_TOKEN=...
   E2B_API_KEY=...  (optional, local sandbox works)
   GITHUB_TOKEN=ghp_...
   LOGFIRE_READ_TOKEN=...
   ```
3. `cargo run` (or `SANDBOX_URL=http://127.0.0.1:3478 cargo run` for local sandbox)
4. Wait ~10s for boot
5. DM the bot on Discord, or use curl:
   ```
   curl -X POST http://localhost:3467/tdata/Channels('{id}')/OpenPaw.Channel.ReceiveMessage \
     -H "x-tenant-id: default" -H "x-temper-principal-kind: admin" \
     -H "content-type: application/json" \
     -d '{"message_id":"msg-1","author_id":"user","thread_id":"user","content":"..."}'
   ```

### What auto-configures:
- 3 souls bootstrapped (Paw, Developer, Scout)
- Discord Gateway connected
- AgentRoute bound to Paw soul with full tool set
- Local sandbox auto-started (if SANDBOX_URL set)
- Secrets seeded in vault

### What needs manual setup:
- Nothing (if .env is filled in and SANDBOX_URL is set)
- Without SANDBOX_URL: E2B sandbox used (but command execution broken)

---

## SANDBOX DEFINITION

### Current state:
- **Local sandbox**: Python HTTP server (`local_sandbox.py`) running on host
  - Endpoints: `/v1/processes/run` (bash), `/v1/fs/file` (read/write)
  - Workdir: `/tmp/paw-workspace`
  - Has access to everything the host has: git, node, python, npm, etc.
  - No isolation — runs with host user privileges
  - No image or container — just a Python process

- **E2B sandbox**: Cloud VM provisioned via E2B API
  - Template: "base" (Ubuntu)
  - Provisions: sandbox_id + sandbox_url returned
  - Commands: NOT WORKING (Connect protocol content-type issue)

### What's NOT defined:
- No "computer spec" or image definition for what the sandbox should have
- No dependency installation (npm, pip) as part of provisioning
- No git credential pre-injection during sandbox setup
- No project-specific environment setup
- The Developer agent just clones and hopes git/node/python are available

### Vision vs reality:
- **Vision**: Developer gets a persistent cloud computer (Fly Sprites) with the project cloned, deps installed, git configured, monitoring agents watching
- **Reality**: Developer gets a Python file server on the host machine with whatever tools happen to be installed

---

## HARNESS DEFINITION

### Current state:
- `paw-harness` OS app has 2 entity types: ProjectHarness, WorkCycle
- ProjectHarness stores: name, repo_url, tech_stack, branch_strategy, conventions, setup_script, monitoring_config
- WorkCycle enforces: Planning → Planned → InProgress → Testing → Reviewing → Complete
- **But**: The harness is generic, NOT deep-sci-fi specific
- No conventions defined (empty JSON array)
- No setup_script defined
- No monitoring_config populated
- WorkCycle states exist but Developer agent doesn't follow them (it just runs commands freely)

### What's NOT implemented:
- Harness doesn't enforce anything on the Developer agent — no gates, no checks
- Developer agent doesn't create WorkCycles before coding
- No test gate (must pass tests before review)
- No review gate (must have review before merge)
- No project-specific conventions (linting rules, commit format, etc.)
- deep-sci-fi's existing CLAUDE.md + hooks NOT converted to temper entities

### Vision vs reality:
- **Vision**: Harness is a temper app that enforces development workflow — the agent CAN'T skip steps because the state machine won't allow it
- **Reality**: Harness entities exist but are decorative — the Developer agent ignores them

---

## CI/CD + ALERT PIPELINE

### What works:
- AlertCycle entity exists with full state machine (Created → Triaging → Fixed/Tuned)
- Paw can create AlertCycles via temper_create
- Paw can diagnose alerts (DiagnoseReal, DiagnoseNoise)

### What does NOT work:
- **No Logfire webhook**: No `POST /webhooks/ingest` endpoint
- **No automatic alert detection**: Alerts are manually created by Paw, not triggered by Logfire
- **No monitor creation**: No Datadog/Logfire monitor auto-generation from code diffs
- **No PR creation**: Developer agent writes diagnosis but doesn't commit/push/PR
- **No merge/deploy**: No CD pipeline triggered by agent work
- **No closed loop**: Alert → Scout → Developer → PR → Merge → Deploy → Monitor is NOT working end-to-end

### Vision vs reality:
- **Vision**: PR merged → monitors auto-generated → alert fires → scout triages → developer fixes → PR pushed → human approves merge → monitors updated
- **Reality**: Human tells Paw to create entities → Paw creates them → Developer investigates code → writes local file → done

---

## FSYNC (Sandbox ↔ TemperFS)

### Current state:
- Fsync DISABLED for local sandbox (only runs for E2B)
- When enabled: enumerates all files in workdir, syncs text files to TemperFS
- Binary files (PNG, etc.) cause errors (`utf-8 codec can't decode`)
- With deep-sci-fi repo (~7000 files), fsync enumerates 408 files per turn

### What's NOT implemented:
- No selective fsync (only changed files)
- No binary file handling
- No .gitignore-aware exclusion
- No incremental sync (full enumeration every turn)

### Vision vs reality:
- **Vision**: Agent's workspace is transparently synced to TemperFS, survives sandbox restarts
- **Reality**: Fsync disabled for local, broken for E2B (Connect protocol), and would be slow anyway

---

## SUMMARY TABLE

| Feature | Vision | Reality | Gap |
|---------|--------|---------|-----|
| Daemon boots | ✅ | ✅ Works | None |
| Entity state machines | ✅ | ✅ All verified | None |
| Paw personality (Soul) | ✅ | ✅ Works | None |
| Paw creates entities | ✅ | ✅ 12 turns, autonomous | None |
| Developer reads code | ✅ | ✅ 28 turns, diagnosis written | None |
| Session continuity | ✅ | ✅ Resume works | None |
| Content-per-file session | ✅ | ✅ ADR-0003 implemented | None |
| Discord connected | ✅ | ⚠️ Gateway yes, DM untested | Human test needed |
| E2B sandbox | ✅ | ⚠️ Provisions, commands broken | Connect protocol fix |
| Local sandbox | ✅ | ✅ Works (no isolation) | No container/image |
| Git push + PR | ✅ | ❌ Not tested | Need to verify |
| Scout + Logfire | ✅ | ❌ Tool exists, not tested | Need real Logfire test |
| Webhook alerts | ✅ | ❌ Not implemented | Need /webhooks/ingest |
| Cedar enforcement | ✅ | ❌ All permissive | Need real policies |
| Harness enforcement | ✅ | ❌ Entities exist, not enforced | Wire into agent workflow |
| Paw spawns Developer | ✅ | ❌ Tool exists, not tested | Need spawn_agent test |
| Full closed loop | ✅ | ❌ Pieces work, not connected | Major integration work |
| Server restart | ✅ | ❌ Not tested | Need persistence test |
| Monitor auto-generation | ✅ | ❌ Not implemented | MonitorScan entity needed |
| Multi-tenant isolation | ✅ | ❌ Single tenant only | Need tenant scoping |
