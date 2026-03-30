# Proof Report: 022 — Soul Architecture & Runtime Crafting Infrastructure

## Date

2026-03-30

## Branch / Commit

`feat/openpaw-self-heal-loop-codex` / `c39a19cf`

## What Was Done

Restructured the soul system and built the infrastructure for Paw to craft and spawn project lead agents at runtime:

1. **Soul separation** — Split monolithic soul files into SOUL.md (identity), STYLE.md (voice), SKILL.md (operations) for human-facing agents; SKILL.md only for task agents (SWE/SRE)
2. **Agent hierarchy** — Human → Paw (chief of staff) → Project Lead (crafted on demand) → SWE/SRE (task-specific, no personality)
3. **Startup multi-file loading** — `startup.rs` concatenates SOUL+STYLE+SKILL before uploading to TemperFS as a single Soul entity
4. **Skill bootstrapping** — Project Lead SCHEMA.md and SKILL.md registered as scoped Skill entities at boot
5. **`file_upload` tool** — New WASM tool enabling agents to create TemperFS files at runtime
6. **Skill scope filtering** — `load_skills_block` now filters by `Scope eq 'global' or Scope eq '{soul_name}'`
7. **Crafting workflow** — Paw's SKILL.md documents the concrete tool-call sequence for runtime soul creation
8. **ADR-0006** — Architecture decision documented

## Verification Flow

### Static Verification

| Step | Method | Result |
|------|--------|--------|
| openpaw crate compilation | `cargo check` | PASS (1 pre-existing warning) |
| tool-runner WASM compilation | `cargo check` in tool_runner dir | PASS |
| llm-caller WASM compilation | `cargo check` in llm_caller dir | PASS (8 pre-existing warnings) |
| file_upload registered in `is_entity_tool` | Code review | PASS |
| file_upload required params | Code review | PASS — `&["name", "content"]` |
| file_upload tool definition in llm_caller | Code review | PASS — JSON schema with name, content, mime_type |

### Runtime Verification (Local Instance — Fresh DB)

Server started with `cargo run`, fresh SQLite DB at `~/.local/share/openpaw/paw.db`.

| Step | Expected | Actual | Status |
|------|----------|--------|--------|
| Soul bootstrap | 3 souls created: Paw, SWE, SRE | `Paw: Paw chief of staff agent [Active]`, `SWE: Software developer agent [Active]`, `SRE: Site reliability engineering agent [Active]` | PASS |
| Paw soul content concatenated | SOUL.md + STYLE.md + SKILL.md in one file | `grep "^# "` → `# Paw`, `# Paw — Voice & Style`, `# Paw — Operating Manual` — all three sections present | PASS |
| Paw soul content correct | Chief of staff identity, not project manager | Content starts with "I'm the chief of staff for your software operation" | PASS |
| Skill bootstrap | 2 skills with correct scope | `Project Lead Schema: scope=Paw [Active]`, `Project Lead Playbook: scope=project-lead [Active]` | PASS |
| Skill content readable | Schema content loads from TemperFS | First line: `# Project Lead — Soul Schema` | PASS |
| File upload → Soul create → Publish | Full lifecycle via OData API | File created (`019d410b-063c...`), Soul created (`019d410b-0680...`), Published: `Active` | PASS |
| Created soul content readable | TemperFS returns uploaded markdown | Content: `# Test Lead Soul` with worldview section | PASS |

### End-to-End Agent Crafting (Runtime — Tensorlake Sandbox)

| Step | Expected | Actual | Status |
|------|----------|--------|--------|
| Agent with file_upload tool | Agent provisions via Tensorlake, runs to completion | Agent `019d4120-a9df` completed in ~10s | PASS |
| file_upload WASM tool | Creates TemperFS file, returns file_id | File `019d4120-b9b5` created with correct markdown content | PASS |
| temper_create Soul via agent | Soul entity created with ContentFileId | Soul `VerifiedLead` created, Description: "Agent-crafted project lead" | PASS |
| Soul content readable | TemperFS returns uploaded markdown | Content: `# Verified Lead` with identity and worldview sections | PASS |
| OpenPaw.Publish | Soul transitions Draft → Active | Published successfully via `resolve_bound_action_name` (Souls → OpenPaw namespace) | PASS |
| Full E2E: craft + spawn child | Orchestrator creates soul, publishes, spawns child agent | Soul `E2ELead` created with content, child agent `019d4122-7ca` spawned. Agent ran out of turns before completing all steps — child was Created but not Configured. | PARTIAL |
| E2E soul content | Crafted content stored correctly | `# E2E Lead` with identity and worldview — matches exactly what was uploaded via file_upload | PASS |

### Lifecycle Verification Summary

The complete runtime-verified pipeline:
```
Agent (Paw soul) → file_upload (TemperFS) → temper_create (Soul entity) → OpenPaw.Publish → Active Soul → spawn_agent (child with crafted soul)
```

Every individual step works. The only gap: a single agent completing all 5 steps in sequence requires sufficient turn budget.

## What Worked

- Fresh DB boot creates all 3 souls with correct names ("Paw chief of staff agent") and concatenated content (SOUL+STYLE+SKILL)
- Both project-lead skills bootstrap with correct scope values (Paw, project-lead)
- `file_upload` WASM tool creates TemperFS files from within a running agent
- `temper_create` creates Soul entities with the uploaded ContentFileId
- `OpenPaw.Publish` transitions souls from Draft → Active
- Soul content is readable from TemperFS via the ContentFileId reference
- Agents provision and complete via Tensorlake sandbox
- Child agent spawning works (agent was created, soul reference passed)

## What Didn't Work

- Agent turn budget (max_turns=10) was insufficient for the full 5-step E2E flow — agent used turns on file_upload + temper_create but didn't complete Publish + spawn_agent + wait in one run
- Child agent created but not configured due to parent running out of turns

## Limitations

- **Turn budget**: The full crafting pipeline (file_upload → create → publish → spawn → wait) requires more than 5 turns. Recommend max_turns ≥ 15 for Paw when crafting leads.
- **Skill scope filtering**: Skills bootstrap with correct scope values. The OData query filtering was not tested through actual prompt assembly (would need to inspect the assembled system prompt of a scoped agent).

## What Still Doesn't Work

- A single agent completing all 5 crafting steps needs higher turn budget or the task should be broken into fewer steps

## Artifacts

- `docs/adrs/0006-soul-architecture.md` — architecture decision record
- `souls/paw/SOUL.md`, `souls/paw/STYLE.md`, `souls/paw/SKILL.md` — Paw's separated soul
- `souls/project-lead/SCHEMA.md` — dimensions for crafting project lead souls
- `souls/project-lead/SKILL.md` — shared operational playbook for all leads
- `souls/swe/SKILL.md`, `souls/sre/SKILL.md` — task agent playbooks
- `crates/openpaw/src/startup.rs` — multi-file soul loading + skill bootstrapping
- `os-apps/paw-agent/wasm/tool_runner/src/entity_tools.rs` — `file_upload` tool handler
- `os-apps/paw-agent/wasm/llm_caller/src/lib.rs` — `file_upload` tool def + scoped skill loading

## Architecture Diagram

```text
Human
  │
  ▼
┌─────────────────────────────────────────────┐
│  Paw (chief of staff)                       │
│  SOUL.md + STYLE.md + SKILL.md              │
│                                             │
│  Crafts project leads using:                │
│  1. read_entity (load SCHEMA.md)            │
│  2. Generate SOUL + STYLE content           │
│  3. file_upload → TemperFS                  │
│  4. temper_create Soul → Publish            │
│  5. spawn_agent with crafted soul_id        │
└────────────────┬────────────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────────────┐
│  Project Lead (crafted per-project)         │
│  Bespoke SOUL + STYLE + shared SKILL        │
│  Scoped skills: "Project Lead Playbook"     │
│                                             │
│  Manages task agents:                       │
│  - spawn_agent(SWE) for features            │
│  - spawn_agent(SRE) for infrastructure      │
│  - Teaches via scoped Skill entities        │
└───────┬─────────────────────┬───────────────┘
        │                     │
        ▼                     ▼
┌───────────────┐   ┌────────────────┐
│  SWE          │   │  SRE           │
│  SKILL.md     │   │  SKILL.md      │
│  only         │   │  only          │
│               │   │                │
│  No soul      │   │  No soul       │
│  No style     │   │  No style      │
│  No human     │   │  No human      │
│  interaction  │   │  interaction   │
│               │   │                │
│  Execute →    │   │  Investigate → │
│  Report via   │   │  Report via    │
│  entity state │   │  entity state  │
└───────────────┘   └────────────────┘
```
