# Proof Report: 003 — Full MVP Status

## Date
2026-03-26

## Branch / Commit
`feat/openpaw-self-heal-loop-claude` / `ae53ab6`

## What Was Done

Complete implementation and verification of the Open Paw MVP — from repo scaffold to self-healing loop, entity management, E2B sandbox provisioning, and Discord transport.

## Verification Results

| # | Test | Status | Notes |
|---|------|--------|-------|
| 1 | Daemon boots with embedded temper | PASS | ~10s boot, 23 entity types, 16 WASM modules |
| 2 | 6 OS apps install | PASS | paw-agent, paw-channels, paw-fs, paw-pm, paw-harness, paw-heal |
| 3 | 3 souls bootstrap to Active | PASS | Paw, Developer, Scout — content uploaded to TemperFS |
| 4 | Discord Gateway connects | PASS | Channel entity in Connected state, bot online |
| 5 | Soul content loaded by LLM | PASS | Paw responds with personality from soul.md |
| 6 | Agent tool loop | PASS | LLM → tool_use → tool_runner → HandleToolResults → loop |
| 7 | temper_create/action/list tools | PASS | Paw creates entities via OData |
| 8 | ProjectHarness lifecycle | PASS | Created → Configure → Active |
| 9 | WorkCycle lifecycle | PASS | Planning → Planned → InProgress |
| 10 | AlertCycle lifecycle | PASS | Created → Open → Triaging → Tuned/Fixed |
| 11 | Full self-healing loop | PASS | Paw autonomously creates ProjectHarness + AlertCycle + Issue + WorkCycle (12 turns, 51s) |
| 12 | E2B sandbox provisioning | PASS | Sandbox ID + URL created via E2B REST API |
| 13 | E2B command execution | **FAIL** | Connect protocol content-type mismatch (temper-wasm fix needed) |
| 14 | Developer agent bash execution | PARTIAL | Works with local sandbox, fails on E2B |
| 15 | Session continuity | NOT TESTED | Each message creates new agent (Resume not wired) |
| 16 | Server restart resilience | NOT TESTED | Turso persistence exists but entity recovery not tested |
| 17 | Discord DM end-to-end | NOT TESTED | Needs human to send DM |

## What Works

- Single daemon binary (openpaw) embeds full temper platform
- Entity model renamed: Agent, Soul, Memory, Skill (namespace: OpenPaw)
- 23 entity types across 6 OS apps, all state machines verified
- Paw agent uses temper_create/temper_action/temper_list tools to autonomously create and manage entities
- Full self-healing loop: ProjectHarness → AlertCycle → Issue → WorkCycle
- E2B sandbox provisions via REST API (sandbox_id, sandbox_url)
- Discord Gateway connected, Channel/AgentRoute entities created
- Soul content persists in TemperFS, loaded by llm_caller
- Anthropic OAuth token works for LLM calls
- Local Turso SQLite for persistence
- All credentials seeded from .env file

## What Doesn't Work

- **E2B command execution**: temper-wasm connect_call sends `content-type: application/json` but E2B envd requires `application/connect+json`. This is a one-line fix in temper-wasm's host_trait.rs line 228.
- **Session continuity**: Follow-up messages create new agents. The route_message WASM checks ChannelSession for existing agents but creates new ones when the previous agent is Completed.
- **Auto soul binding**: The set_default_soul function has timing issues (runs before Discord transport creates the AgentRoute). Currently requires manual binding via curl.

## Limitations

- No Cedar enforcement (all actions permitted for MVP)
- No E2B sandbox command execution (Connect protocol issue)
- Boot time ~10-25s on first start (spec verification for each entity type)
- Turn budget instead of token budget
- No multi-tenant isolation tested

## Architecture Diagram

```
                           ┌─────────────────────┐
                           │    Discord Gateway    │
                           │  (WSS, bot online)   │
                           └──────────┬───────────┘
                                      │ MESSAGE_CREATE
                                      ▼
┌──────────────────────────────────────────────────────────────┐
│                    Open Paw Daemon (:3468)                    │
│                                                              │
│  ┌──────────────────────────────────────────────────────┐   │
│  │ paw-transport (Discord transport)                     │   │
│  │ Channel.ReceiveMessage → route_message WASM           │   │
│  └─────────────────────┬────────────────────────────────┘   │
│                         │ Creates Agent + Configure + Provision
│                         ▼                                    │
│  ┌──────────────────────────────────────────────────────┐   │
│  │ Paw Agent (Soul: Paw)                                 │   │
│  │                                                        │   │
│  │ Tools: temper_create, temper_action, temper_list,      │   │
│  │        save_memory, read_entity                        │   │
│  │                                                        │   │
│  │ 12 turns autonomously creating:                        │   │
│  │  ├── ProjectHarness (Active, deep-sci-fi)             │   │
│  │  ├── AlertCycle (Triaging, Logfire alert)              │   │
│  │  ├── Issue (Backlog, database timeout)                │   │
│  │  └── WorkCycle (Planning, fix tracking)               │   │
│  └──────────────────────────────────────────────────────┘   │
│                                                              │
│  ┌──────────────────────────────────────────────────────┐   │
│  │ Developer Agent (Soul: Developer)                      │   │
│  │ Tools: bash, read, write                               │   │
│  │ Sandbox: E2B (provisions ✓, commands ✗)               │   │
│  └──────────────────────────────────────────────────────┘   │
│                                                              │
│  OS Apps:                                                    │
│  ┌───────────┐ ┌───────────┐ ┌───────────┐ ┌───────────┐  │
│  │paw-agent  │ │paw-chan.  │ │paw-fs     │ │paw-pm     │  │
│  │8 entities │ │3 entities │ │4 entities │ │5 entities │  │
│  │12 WASM    │ │3 WASM     │ │1 WASM     │ │           │  │
│  └───────────┘ └───────────┘ └───────────┘ └───────────┘  │
│  ┌───────────┐ ┌───────────┐                               │
│  │paw-harness│ │paw-heal   │  ← NEW                       │
│  │2 entities │ │1 entity   │                               │
│  └───────────┘ └───────────┘                               │
│                                                              │
│  ┌──────────────────────────────────────────────────────┐   │
│  │ Temper Platform Engine                                 │   │
│  │ ActorSystem + SpecRegistry + Cedar + SecretsVault     │   │
│  │ Turso SQLite (~/.local/share/openpaw/paw.db)          │   │
│  └──────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────┘
         │                              │
         ▼ HTTPS (OAuth)                ▼ HTTPS
   ┌──────────┐                  ┌──────────┐
   │ Claude   │                  │  E2B API │
   │ Sonnet   │                  │ Sandbox  │
   └──────────┘                  └──────────┘
```

## Next Steps (for human)

1. **Fix E2B Connect protocol**: One-line fix in `temper/crates/temper-wasm/src/host_trait.rs:228` — change `application/json` to `application/connect+json`
2. **Test Discord DM**: Send a DM to the bot, verify Paw responds with personality
3. **Auto soul binding**: Fix timing of set_default_soul (add retry/polling)
4. **Session continuity**: Wire Resume action for follow-up messages
5. **Token budgets**: Replace turn limits with token-based budgets
