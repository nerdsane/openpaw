# ADR-0014: App Structure — Agents, Skills, and Seed Data

## Status

Accepted

## Context

The current split between `souls/` directory (read by Rust startup code) and `os-apps/` (installed by platform) means agent identities require a hardcoded bootstrap hack. `spawn_soul_bootstrap` in `startup.rs` reads soul files from a filesystem path, uploads them to TemperFS, and creates Soul entities — all outside the app installation flow.

This creates three problems:

1. **"Skill" is overloaded** to mean three different things: agent operational instructions (SKILL.md in souls/), reusable knowledge injected into agent prompts (Skill entities), and app documentation (skill.md in reference projects). Each has different semantics, different consumers, and different lifecycle.

2. **Apps can't ship their own agent definitions.** An app like `dsf-team` defines 7 agent roles, but agent identity files live in `souls/` while app specs live in `os-apps/`. There's no way for `install_os_app()` to discover and bootstrap agent definitions because they live outside the app boundary.

3. **Bootstrap hack breaks self-containment.** The platform startup code hardcodes knowledge of which soul files exist and how to upload them. Adding a new agent requires touching Rust code, not just adding files to an app.

## Decision

### App manifest and documentation

Every app has:
- **`app.toml`** — Manifest: name, description, version, dependencies
- **`APP.md`** (optional) — Human/agent-readable documentation of what the app does, its entity types, and how to use it

### Agent definitions within apps

Apps may contain an `agents/` directory with one subdirectory per agent:

```
os-apps/paw-agent/agents/paw/
  SOUL.md       # WHO — identity, worldview, opinions (optional)
  STYLE.md      # HOW — communication voice and tone (optional)
  AGENT.md      # WHAT — operational instructions (always present)
```

- **SOUL.md + STYLE.md** define personality. Not all agents have these — task agents (SWE, SRE, probe) get only AGENT.md.
- **AGENT.md** contains operational instructions: tools, workflows, entity interactions. This replaces the overloaded SKILL.md filename for agent instructions.
- Files within an agent directory are concatenated alphabetically. The platform is filename-agnostic — it reads all `.md` files in the directory.

### Soul entity = personality only

The Soul entity stores personality content (SOUL.md + STYLE.md). Not all agents have souls. The Agent entity gets an `instructions_file_id` field for AGENT.md content, separate from `soul_id`.

This amends ADR-0006: the original three-file structure (SOUL.md, STYLE.md, SKILL.md) is preserved but SKILL.md is renamed to AGENT.md and loaded independently of the soul.

### Skills as directory-per-skill

Apps may contain a `skills/` directory:

```
os-apps/paw-agent/skills/research-first-planning/
  SKILL.md      # Skill content injected into agent prompts
```

This follows Anthropic's Agent Skills convention: one directory per skill with a `SKILL.md` file and optional companion files. Skill loading is no longer gated on `soul_id`.

### Seed data

Apps may contain `seed-data/*.toml` files using `[[instance]]` blocks for initial entity creation during app installation.

### Installation discovery

`install_os_app()` discovers and bootstraps all of these:
1. Reads `app.toml` for metadata and dependency resolution
2. Loads specs from `specs/`
3. Loads Cedar policies from `policies/`
4. Uploads agent `.md` files from `agents/` and creates Agent/Soul entities
5. Uploads skill content from `skills/` and creates Skill entities
6. Processes seed data from `seed-data/`
7. Loads WASM modules from `wasm/`

The `spawn_soul_bootstrap` function in `startup.rs` is removed.

## Amends

- **ADR-0006** (Soul Architecture) — Separates personality (Soul entity with SOUL.md + STYLE.md) from operational instructions (Agent entity with AGENT.md). The original three-file concatenation is replaced by independent loading of personality and instructions.

## Consequences

### Positive

- Apps are fully self-contained: agents, skills, policies, specs, seed data, and WASM all ship together
- No bootstrap hacks — `install_os_app()` handles everything
- "Skill" is no longer overloaded — AGENT.md for instructions, SKILL.md for injectable knowledge, APP.md for app docs
- Reference projects (dsf-team, dsf-harness) can define their own agents and skills
- Adding a new agent means adding files to an app directory, not touching Rust code

### Negative

- Migration needed: existing soul content must be reorganized from `souls/` into `os-apps/paw-agent/agents/`
- `install_os_app()` becomes more complex (more directories to discover)
- Existing references to `souls/` paths in documentation and scripts must be updated
