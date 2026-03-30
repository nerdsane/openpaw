# Proof Report: 022 — Soul Architecture & Runtime Crafting Infrastructure

## Date

2026-03-30

## Branch / Commit

`feat/openpaw-self-heal-loop-codex` / `d799db46`

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

### Static Verification (Performed)

| Step | Method | Result |
|------|--------|--------|
| openpaw crate compilation | `cargo check -p openpaw` | Pass (1 pre-existing warning: unused `webhook_secret` field) |
| tool-runner WASM compilation | `cargo check` in tool_runner dir | Pass (clean) |
| llm-caller WASM compilation | `cargo check` in llm_caller dir | Pass (8 pre-existing warnings, no new ones) |
| file_upload tool registered in `is_entity_tool` | Code review | `"file_upload"` added to match list |
| file_upload required params registered | Code review | `"file_upload" => &["name", "content"]` in validate_required_params |
| file_upload tool definition in llm_caller | Code review | JSON schema with name (required), content (required), mime_type (optional) |
| Startup soul paths updated | Code review | Paw: 3 files concatenated; SWE: 1 file; SRE: 1 file |
| bootstrap_skill function | Code review | Follows bootstrap_soul pattern: create File → upload content → create Skill → Register |
| load_skills_block scope filtering | Code review | Queries with `(Scope eq 'global' or Scope eq '{soul_name}')`, with two-query fallback |
| No "project manager" references | `grep "project manager"` | Only in startup.rs description, now reads "chief of staff" |
| No "Kit" references | `grep "Kit" souls/` | One leftover fixed → "leads" |

### Runtime Verification (NOT Performed — No Running Instance)

| Step | Expected | Actual | Status |
|------|----------|--------|--------|
| Boot with new soul paths | Souls bootstrap without error, logs show "Soul 'Paw' ready", "Soul 'SWE' ready", "Soul 'SRE' ready" | Not tested | PENDING |
| Soul content contains SOUL+STYLE+SKILL | GET Souls('Paw') → fetch content file → contains all three sections | Not tested | PENDING |
| Skill bootstrap | "Project Lead Schema" and "Project Lead Playbook" skills created with correct scope | Not tested | PENDING |
| file_upload tool | Agent with file_upload tool can create TemperFS file, returns file_id | Not tested | PENDING |
| Skill scope filtering | Paw sees "Project Lead Schema" (scope: Paw); SWE does NOT | Not tested | PENDING |
| End-to-end crafting | Paw generates soul → file_upload → temper_create Soul → Publish → spawn_agent → lead runs | Not tested | PENDING |

## What Worked

- All three crates compile without new errors
- The `file_upload` implementation reuses the proven `create_tool_content_file` pattern (POST Files + PUT $value)
- Skill scope filtering has a fallback path if Temper's OData doesn't support parenthesized OR
- `bootstrap_soul` signature change from `path: &str` to `paths: &[&str]` is backward-compatible (single-file souls just pass a 1-element slice)
- `bootstrap_skill` follows the exact same File → Entity → Action pattern as `bootstrap_soul`

## What Didn't Work

- Could not perform runtime verification — no Temper server running locally

## Limitations

- **No running Temper instance available** for end-to-end testing. All runtime verification steps are PENDING.
- **OData OR support unknown** — the skill scope filter uses `(Scope eq 'global' or Scope eq '{name}')`. If Temper's OData doesn't support this, the fallback two-query path will activate, but this hasn't been tested.
- **Soul content size** — concatenating SOUL+STYLE+SKILL for Paw produces a large prompt section (~300 lines). Context window impact not measured.

## What Still Doesn't Work

- Runtime soul crafting is infrastructure-ready but untested end-to-end
- The old soul file paths in any external references (e.g., documentation mentioning `souls/developer.md`) may be stale
- `Developer` soul name in startup was changed to `SWE` — any existing Soul entities named "Developer" in a running system would need migration or the name kept as "Developer" for backward compatibility

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
