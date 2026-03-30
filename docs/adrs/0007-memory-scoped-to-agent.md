# ADR-0007: Memory Scoped to Agent, Not Soul

## Status

Accepted

## Context

ADR-0006 introduced soul architecture with a three-file separation (SOUL.md, STYLE.md, SKILL.md) and scoped agent memory to `soul_id`. The reasoning was that memories should persist across agent runs sharing the same soul, treating the soul as the persistent identity.

This scoping is wrong. The taxonomy is:

- **Agent** is the persistent entity. It survives server restarts (`Resume` action), accumulates sessions (`session_file_id`), has heartbeat monitoring, parent/child relationships, and a continuous conversation history. An agent is analogous to Claude Code — a persistent identity that can have many sessions.
- **Soul** is an optional personality overlay. Not all agents have one — SWE and SRE task agents may operate with no soul (empty `soul_id`). A soul defines WHO the agent is (voice, style, worldview), not what it knows.

The `soul_id` scoping creates three problems:

1. **Soul-less agents have no memory.** The `load_memory_block` function is guarded by `if !soul_id.is_empty()`, and both `save_memory` and `recall_memory` filter by `SoulId`. An SRE agent with no soul cannot learn.

2. **Agents sharing a soul incorrectly share memories.** Two SWE agents both assigned `soul_id: "swe"` see each other's memories. They work on different projects and learn different things — shared memory is a data leak.

3. **`author_agent_id` is a workaround.** The Memory entity has both `soul_id` (scoping) and `author_agent_id` (provenance). The existence of `author_agent_id` acknowledges that agents are distinct even when they share a soul, but it's used only for audit, not for access control. The real ownership relationship was mislabeled.

## Decision

Scope memory to `agent_id` — the persistent entity that learns.

### Changes

1. **Add `agent_id` to Memory entity** as the primary scoping field.
2. **Remove `author_agent_id`** — redundant with `agent_id` (the owning agent IS the author).
3. **Keep `soul_id` on Memory** as optional metadata for cross-agent queries (e.g., "what have all agents with this personality learned"), not for access control.
4. **Update Cedar policy** from `resource.SoulId == principal.soul_id` to `resource.AgentId == principal.id`. The principal ID is already populated from the `x-temper-principal-id` header.
5. **Remove the soul_id guard** on memory loading in `assemble_system_prompt`. Memory loads for every agent, regardless of whether it has a soul.
6. **Update OData filters** in `load_memory_block` and `recall_memory` to filter by `AgentId` instead of `SoulId`.

### Relationship model

```
Agent (persistent)
├── agent_id (PRIMARY) ──→ Memory (scoped to this agent)
└── soul_id (optional) ──→ Soul (personality overlay)
```

- Agents can only edit their own memories (`resource.AgentId == principal.id`)
- Supervisors and humans can manage any memory
- All authenticated agents can read/list all memories

## Consequences

### Positive

- Agents always have memory regardless of soul assignment
- Two agents sharing a soul maintain separate, private knowledge
- The ownership model is clean: one field (`agent_id`) for scoping, no redundant `author_agent_id`
- Cedar policy uses `principal.id` which is already available — no new headers or attributes needed

### Negative

- Existing memories (scoped to `soul_id` with no `agent_id`) become invisible under the new filter. This is acceptable — those memories were scoped wrong and the system is pre-production.

### Neutral

- `soul_id` remains on Memory as optional metadata — no loss of queryability for cross-agent analysis
- No migration script needed — old memories simply age out
