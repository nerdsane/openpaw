# ADR-002: Skills as TemperFS Files — Path-Based Scoping

**Status:** Accepted (supersedes original ADR-002)
**Scope:** entity-types, integrations, bootstrap
**Author:** claude-code
**Date:** 2026-04-09

## Context

The original skills-as-files design (2026-04-08) solved entity-to-file indirection and
ID collisions, but introduced four new problems:

1. **Kernel pollution**: `list_skills`/`load_skill`/`create_skill` were hardcoded in the
   Temper kernel dispatch layer. Agents already have `temper.read(path)`/`temper.write(path)`
   in their WASM sandbox — kernel methods were redundant and violated the temper-native rule.

2. **Scope by name-matching**: `skill_matches_scope()` in llm_caller matched a frontmatter
   `scope` field against agent/soul names. Fragile and wrong — two agents named "paw" in
   different apps would see each other's skills.

3. **Apps invisible at runtime**: Apps were an in-memory `BTreeMap<String, PathBuf>` with
   no entity representation. Agents couldn't discover "what did this app bring in?"

4. **Entity ID collisions**: Deterministic IDs like `app-soul-{name}` and
   `os-skill-file-{name}` collided across apps with same-named agents.

## Decision

### Path = Scope (no frontmatter)

Three scope levels determined purely by TemperFS path. No `scope` frontmatter field.

```
/system/skills/{name}/SKILL.md                → system-level (platform knowledge, all agents)
/agents/{agent-uuid}/skills/{name}/SKILL.md   → agent-scoped (from app bootstrap or runtime)
/projects/{project-id}/skills/{name}/SKILL.md → project-scoped (runtime, created by leads)
```

The llm_caller queries paths using IDs from the session context:
1. `/system/skills/` — always (platform knowledge)
2. `/agents/{session.agent_id}/skills/` — always (agent's own skills)
3. `/projects/{session.project_id}/skills/` — if project_id is set

Precedence on name collision: agent > project > system.

### SKILL.md Frontmatter

Only `name` and `description` in YAML frontmatter. No `scope` field.

```yaml
---
name: skill-name
description: One-line description
---
```

### Progressive Disclosure

- **L0**: Name + description listed as XML in system prompt (`<skill name="..." description="..." path="..." />`)
- **L1**: Agent loads full content via `temper.read(path)` (path from XML attribute)
- **L2**: Companion files in same directory, accessed via `temper.read(path)`

No kernel `load_skill` method. The `path` attribute in XML replaces the `file_id` attribute.

### System Skills

Platform operational knowledge that all agents need, regardless of source app:

```
/system/skills/platform-awareness/SKILL.md     → how to discover apps, entities, capabilities
/system/skills/temper-app-creation/SKILL.md    → how to create new apps
/system/skills/research-first-planning/SKILL.md → planning methodology
```

On disk (in paw-agent): `system/skills/{name}/SKILL.md`

### Agent-Scoped Skills

All app-bundled skills live under their agent's TemperFS directory:

```
/agents/{curator-uuid}/skills/sourcing/SKILL.md
/agents/{curator-uuid}/skills/synthesis/SKILL.md
```

On disk: `agents/{agent-name}/skills/{skill-name}/SKILL.md`

### App Entity

New `App` entity type tracks installed apps with provenance:

- Fields: `name`, `description`, `version`, `app_guide_file_id`
- States: `Installed`, `Archived`
- APP.md bootstrapped to `/apps/{app-name}/APP.md` in TemperFS
- `app_guide_file_id` points to the APP.md File entity

### Agent Source App Reference

Agent entity has `source_app_id` field linking to the App that installed it.

### Prefixed UUID Entity IDs

All entity IDs are prefixed UUIDs: `{type-prefix}-{uuid-v7}`. No concatenated
name-smashing IDs. Bootstrap idempotency via name-based lookups.

| Entity Type | Prefix |
|-------------|--------|
| App | `ap-` |
| Agent | `aj-` |
| Soul | `sl-` |
| Session | `ss-` |
| File | `fl-` |
| Directory | `dr-` |

### Kernel Separation

Removed from kernel dispatch: `list_skills`, `load_skill`, `create_skill`.
Agents use `temper.read(path)` and `temper.write(path, content)` instead.

## On-Disk Directory Structure

```
paw-agent/
  system/
    skills/
      platform-awareness/SKILL.md
      temper-app-creation/SKILL.md
      research-first-planning/SKILL.md
  agents/
    paw/
      AGENT.md
      SOUL.md
      STYLE.md
      skills/
        openpaw-agent/SKILL.md
        openpaw-lead/SKILL.md
        project-lead-schema/SKILL.md
        project-lead-playbook/SKILL.md
```

## Consequences

### Positive

- Path encodes scope — no fragile name-matching, no frontmatter scope
- No ID collisions — agent UUID in path makes skills globally unique
- Apps are first-class entities with provenance chain (App → Agent → Skills)
- Progressive disclosure via path attribute — agents use standard `temper.read(path)`
- System skills separated from agent skills — clear ownership
- No kernel skill methods — agents use the same read/write as everything else
- Prefixed UUIDs make entity type identifiable from ID alone

### Negative

- Must query 2-3 path prefixes to assemble all skills for an agent
- Agent UUID in paths is opaque to humans (mitigated: human names on disk)
- Metadata (description) parsed from frontmatter, not queryable via OData fields
- Requires TemperFS to be installed (acceptable — it is a core dependency)
