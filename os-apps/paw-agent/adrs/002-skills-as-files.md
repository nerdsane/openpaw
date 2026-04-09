# ADR-002: Skills as TemperFS Files

**Status:** Accepted
**Scope:** entity-types, integrations
**Author:** claude-code
**Date:** 2026-04-08

## Context

The Skill entity system has 4 overlapping scoping dimensions (`scope`, `agent_filter`,
`skill_ids` CSV, `project_id`) and a fragile `content_file_id` indirection where
deterministic IDs collide across apps. Agents on Discord cannot reliably access skill
content — 3 "Temper App Creation" entries all resolved to wrong content because the
File entity backing them was overwritten or stale. The 120-line `load_skills_block` in
`llm_caller` WASM is opaque and hard to debug.

OpenClaw and Hermes Agent both use filesystem-based skill storage with agentskills.io
alignment (directory + SKILL.md). Hermes adds progressive disclosure (L0 catalog,
L1 full content, L2 companion files). Both support agent self-creation of skills.

## Decision

Eliminate the Skill entity type. Skills become plain Files in TemperFS at conventional
paths. Path encodes scope:

- `/skills/{name}/SKILL.md` — tenant-scoped (all agents)
- `/projects/{pid}/skills/{name}/SKILL.md` — project-scoped
- `/agents/{aid}/skills/{name}/SKILL.md` — agent-scoped

SKILL.md uses YAML frontmatter (`---`) aligned with agentskills.io:

```yaml
---
name: skill-name
description: One-line description
scope: global
---
```

Progressive disclosure:
- **L0** — name + description injected into system prompt at session start (~50-100 tokens per skill)
- **L1** — full SKILL.md loaded on demand via `temper.load_skill(name)`
- **L2** — companion files in same directory, accessed naturally via `temper.read(path)`

Agents self-create skills via `temper.create_skill(name, content, scope)` which handles
directory creation and path routing.

Scope filtering uses the `scope` field in YAML frontmatter:
- `"global"` or empty — loaded for all agents in the tenant
- Agent/soul name — loaded only when agent name or soul name matches (case-insensitive)

## Consequences

### Positive
- Single source of truth: the file IS the skill, no entity-to-file indirection
- Path encodes scope visibly and debuggably
- Self-creation is 1 API call, not 3
- L2 companion files come free (co-located in directory)
- Aligns with agentskills.io, portable with OpenClaw/Hermes ecosystems
- No ID collisions — paths are unique by definition
- Dramatically simpler llm_caller (~30 lines replaces ~120)

### Negative
- Must query multiple path prefixes to assemble all skills for an agent
- Metadata (description) parsed from frontmatter, not queryable via OData fields
- Requires TemperFS to be installed (acceptable — it is a core dependency)
- Existing Skill entities become orphaned (harmless, deprecated over time)
