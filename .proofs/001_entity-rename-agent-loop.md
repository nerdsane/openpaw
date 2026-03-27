# Proof Report: 001 — Entity Rename + Agent Loop

## Date
2026-03-26

## Branch / Commit
`feat/openpaw-self-heal-loop-claude` / `1cd2449`

## What Was Done
- Renamed all entity types: TemperAgent→Agent, AgentSoul→Soul, AgentMemory→Memory, AgentSkill→Skill, ToolHook→Hook, HeartbeatMonitor→Heartbeat
- Changed CSDL namespace to `OpenPaw` across all OS apps
- Updated all WASM module source files with new entity names
- Recompiled all 16 WASM modules
- Added Turso local SQLite storage (was in-memory, blob store needed persistence)
- Added `blob_endpoint` secret to vault (was missing — root cause of all file upload failures)
- Added `dotenvy` for .env file loading
- Switched to multi-threaded tokio runtime (WASM host functions use `block_in_place`)
- Switched temper dependency from `main` to `feat/temper-claw` branch (WASM loading only exists there)
- Copied `temper-wasm-sdk` crate for WASM compilation

## Verification Flow

### Level 1: Platform Health
1. Boot daemon with `PORT=3468 ./target/debug/openpaw`
2. Check entity sets via `GET /tdata`
3. Check souls via `GET /tdata/Souls`
4. Count WASM modules loaded from startup logs

### Level 2: Blob Upload
1. Create a File entity
2. PUT content to `Files('{id}')/$value`
3. Verify 200/204 response

### Level 3: Agent Loop
1. POST to create Agent
2. POST Configure with "What is 2+2?"
3. POST Provision
4. Wait for Completed/Failed
5. GET agent state, check result

## Verification Results

| Step | Expected | Actual | Status |
|------|----------|--------|--------|
| L1: Entity sets | 20 types with renamed names | Agents, Souls, Memories, Skills, Hooks, Heartbeats, Channels, etc. | PASS |
| L1: WASM modules | 16 loaded | 16 loaded (log confirmed) | PASS |
| L1: Souls | 3 Active | Paw: Active, Developer: Active, Scout: Active | PASS |
| L2: Blob upload | 200/204 | 204 No Content (with Turso + blob_endpoint) | PASS |
| L3: Configure | 200 | 200 (via OpenPaw.Agent.Configure path) | PASS |
| L3: Provision | 200 | 200 | PASS |
| L3: LLM call | Anthropic API returns response | input=39, output=12, stop_reason=end_turn | PASS |
| L3: Agent status | Completed | Completed (RecordResult transition) | PASS |
| L3: Agent result | Contains "4" | "2+2 equals 4." | PASS |

## What Worked
- Entity rename across specs, CSDL, WASM, Cedar — all consistent
- Anthropic OAuth token auto-detected and used correctly
- Full state machine cycle: Created → Provisioning → Thinking → Completed
- Turso local SQLite for persistence
- blob_adapter WASM registered and functional with blob_endpoint secret

## What Didn't Work
- `paw-compute` OS app still fails to load ("not found in catalog") — IOA spec parsing issue, non-blocking
- Short form action path `/Configure` returns 405 — must use qualified `OpenPaw.Agent.Configure`
- Soul content upload during bootstrap still fails (blob_adapter works for agent but soul bootstrap runs before routes are fully ready — timing)

## Limitations
- No persistence across restarts (in-memory entity state, Turso only stores trajectories + blobs)
- No Cedar enforcement (all actions permitted)
- Single tenant only ("default")
- paw-compute not functional

## What Still Doesn't Work
- Discord transport (untested — next step)
- Soul content not uploaded to TemperFS (souls exist but content files are empty)
- E2B sandbox provisioning (not tested yet)
- Developer agent spawning (depends on above)

## Artifacts

### Agent creation and result
```
POST /tdata/Agents → 019d2ba1-952d-7c03-9f63-154b679b63c8
POST /tdata/Agents('{id}')/OpenPaw.Agent.Configure → 200
POST /tdata/Agents('{id}')/OpenPaw.Agent.Provision → 200
GET  /tdata/Agents('{id}') → {status: "Completed", result: "2+2 equals 4."}
```

### Startup log (key lines)
```
Phase 1: Storage: turso (file:/Users/seshendranalla/.local/share/openpaw/paw.db)
Phase 6: Installed paw-agent: 8 entities, wasm=[llm_caller, tool_runner, ...]
         Installed paw-channels: 3 entities, wasm=[channel_connect, route_message, send_reply]
         Installed paw-fs: 4 entities, wasm=[blob_adapter]
         Installed paw-pm: 5 entities
Phase 9: Open Paw listening on port 3468
Soul 'Paw' ready (Active)
Soul 'Developer' ready (Active)
Soul 'Scout' ready (Active)
llm_caller: calling Anthropic API, model=claude-sonnet-4-20250514, oauth=true
llm_caller: usage: input=39, output=12, stop_reason=end_turn
transition: RecordResult → Completed
```

## Architecture Diagram
```
┌─────────────────────────────────────────────┐
│           Open Paw Daemon (port 3468)       │
│                                             │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  │
│  │paw-agent │  │paw-chan  │  │ paw-fs   │  │
│  │8 entities│  │3 entities│  │4 entities│  │
│  │12 WASM   │  │3 WASM    │  │1 WASM    │  │
│  └────┬─────┘  └──────────┘  └────┬─────┘  │
│       │                           │         │
│  ┌────▼──────────────────────────▼────┐    │
│  │        Temper Platform Engine       │    │
│  │  (SpecRegistry, ActorSystem, Cedar) │    │
│  └────────────────┬───────────────────┘    │
│                   │                         │
│  ┌────────────────▼───────────────────┐    │
│  │     Turso (SQLite @ ~/.local/...)   │    │
│  │  trajectories + blobs + wasm cache  │    │
│  └─────────────────────────────────────┘    │
│                                             │
│  Secrets Vault: anthropic_api_key,          │
│    blob_endpoint, e2b_api_key, github_token │
└─────────────────────────────────────────────┘
         │
         ▼ HTTP (Anthropic API)
   ┌───────────┐
   │  Claude    │ ← "2+2 equals 4."
   └───────────┘
```
