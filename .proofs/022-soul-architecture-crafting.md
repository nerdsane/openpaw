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

### End-to-End Agent Crafting (Partial)

| Step | Expected | Actual | Status |
|------|----------|--------|--------|
| Agent create + configure | Agent transitions to Configured | Agent created, but sandbox provisioner tries Tensorlake (external) instead of local — fails with timeout | BLOCKED |
| Root cause | — | Tensorlake API key is configured in secrets vault, overriding local sandbox. Agent lifecycle requires sandbox even for non-code tasks. | — |

The file_upload → Soul → Publish lifecycle was verified via direct OData API calls (the same HTTP calls the WASM tool makes). The WASM tool handler uses identical `POST /tdata/Files` + `PUT $value` pattern.

## What Worked

- Fresh DB boot creates all 3 souls with correct names and concatenated content
- Both project-lead skills bootstrap with correct scope values
- Full soul crafting lifecycle (file upload → create → publish → read) works via OData
- Content files are correctly stored and readable from TemperFS
- `bootstrap_soul` handles multi-file concatenation (Paw: 3 files → 1 content file)
- `bootstrap_skill` creates and registers skills with scope metadata

## What Didn't Work

- Agent spawn blocked by Tensorlake sandbox provisioner timeout — agents require sandbox even for pure entity-tool tasks
- Could not verify the `file_upload` WASM tool through an actual running agent (would need local sandbox or Tensorlake credentials)

## Limitations

- **Sandbox dependency**: All agents require sandbox provisioning, even for non-code tasks (file_upload, temper_create). This blocks pure-orchestration agents like Paw in environments without sandbox access.
- **Skill scope filtering**: Verified that skills bootstrap with correct `scope` field values. The OData query filtering (`Scope eq 'global' or Scope eq '{soul_name}'`) was not tested through an actual agent prompt assembly (blocked by sandbox).

## What Still Doesn't Work

- Agents cannot run locally without either local Python sandbox or Tensorlake credentials
- The WASM `file_upload` tool was verified by code review + equivalent OData calls, not through an actual agent invocation

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
