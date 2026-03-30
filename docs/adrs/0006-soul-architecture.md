# ADR-0006: Soul Architecture — Identity, Voice, and Operations Separation

## Status

Accepted

## Context

OpenPaw's original agent souls were single markdown files that blended identity, communication style, tool documentation, workflows, and entity references into one document. This made it impossible to:

1. **Craft agent identities at runtime** — Paw (the chief of staff) needs to create project leads with bespoke personalities tailored to each project's stage, domain, and risk profile. A single monolithic soul file conflates what should be dynamic (personality) with what should be shared (operational playbook).

2. **Separate human-facing agents from task agents** — SWEs and SREs don't interact with humans. They don't need personality, voice, or communication style. They need precise operational instructions. Giving them a full soul wastes context and confuses their role.

3. **Teach agents project-specific knowledge** — Project leads accumulate knowledge about their project (conventions, failure patterns, shortcuts). There was no mechanism to encode this knowledge and inject it into future SWE/SRE agents.

The soul.md ecosystem (aaronjmars/soul.md, Soul Spec v0.5) provides a proven separation: SOUL.md (identity), STYLE.md (voice), SKILL.md (operations). We adapt this for OpenPaw's agent hierarchy.

## Decision

### Three-file soul structure for human-facing agents

Agents that communicate with humans (Paw, project leads) have three files:

- **SOUL.md** — WHO the agent is: identity, worldview, opinions, tradeoff style, tensions, boundaries
- **STYLE.md** — HOW the agent communicates: register, vocabulary, tone by situation, right/wrong voice examples
- **SKILL.md** — WHAT the agent does: tools, entities, workflows, operational procedures

At runtime, these are concatenated into a single `content_file_id` on the Soul entity. The separation exists in source for human readability and independent evolution.

### SKILL.md only for task agents

SWEs and SREs get a single SKILL.md — their operational playbook. No SOUL.md, no STYLE.md. They receive instructions from their project lead, execute, and report results through entity state transitions. "No prose. No personality. Just results."

### Agent hierarchy: Human → Paw → Project Lead → SWE/SRE

- **Paw** is the chief of staff. Talks to humans. Manages through project leads.
- **Project leads** are crafted on demand by Paw — one per project, with a bespoke soul tailored to the project's stage, domain, stack, and needs. They own their project end-to-end and fan out to SWEs and SREs.
- **SWEs** handle feature work: code, tests, commits, PRs. Task-specific, lead-managed.
- **SREs** handle infrastructure: alerts, scaling, performance, incident response. Task-specific, lead-managed.

Paw does NOT spawn SWEs or SREs directly. Paw works through project leads.

### Paw crafts project lead souls at runtime — no templates

Project leads are not picked from a roster. Paw creates them by:

1. Reading `souls/project-lead/SCHEMA.md` — the dimensions to fill (identity, sensibility, stage posture, domain fluency, tradeoff style, worldview, tensions, boundaries, voice)
2. Generating SOUL.md + STYLE.md content tailored to the specific project
3. Appending the shared `souls/project-lead/SKILL.md` operational playbook
4. Uploading the combined content to TemperFS via `file_upload`
5. Creating and publishing a Soul entity
6. Spawning an agent with that soul

The crafted soul can evolve — if the project's stage changes, Paw can update the lead's soul.

### Project leads teach SWEs/SREs through scoped Skill entities

Project leads accumulate knowledge about their project. They encode this as `Skill` entities with a `scope` field that limits which agents see them. Skills are injected into the system prompt at agent spawn time, filtered by scope matching the agent's soul.

Teaching triggers:
- After a failed first attempt (what instruction would have prevented it?)
- After a slow success (what shortcut should future agents know?)
- After the lead handles something directly (capture the context)
- When the codebase changes (update skills before next spawn)

### `file_upload` tool enables runtime soul crafting

A new `file_upload` tool allows agents to create TemperFS files with generated content. This is the missing link that enables Paw to craft souls at runtime — previously only the Rust startup code could upload files.

### Skill scope filtering

`load_skills_block` filters skills by `scope` field: agents see global skills plus skills scoped to their soul name. This enables project-specific teaching without polluting other agents' contexts.

## Consequences

### Positive

- Paw can create project leads tailored to any project, stage, or domain without code changes
- Task agents (SWE/SRE) get focused operational instructions without personality overhead
- Project leads build institutional knowledge that makes every subsequent agent more effective
- Soul source files are human-readable and independently evolvable
- The architecture mirrors real org structure: executive → project lead → specialist

### Negative

- Concatenating three files at boot adds a small complexity to startup
- Runtime soul crafting depends on Paw's judgment — a poorly crafted soul produces a poorly calibrated lead
- Skill scope filtering adds an OData query parameter — minor performance impact

### Neutral

- Existing Soul entity spec unchanged (single `content_file_id`) — no migration needed
- Cedar policies unchanged — WASM calls already run as admin
- The `project-lead/SCHEMA.md` is guidance, not enforcement — Paw can deviate if the situation warrants

## Amendments

- **ADR-0007** corrects memory scoping: memory is scoped to `agent_id` (the persistent learning entity), not `soul_id` (the optional personality overlay). See ADR-0007 for rationale.
