# ADR-004: Agent / Session / Team Taxonomy

**Status:** Accepted
**Date:** 2026-04-01
**Deciders:** Sesh (human), Ren (product lead)

## Context

The original `Agent` entity conflated two concerns:

1. **Identity** — a named role on a team with a soul, skills, tools, and session configuration.
2. **Execution** — a single task run with turns, tool calls, a prompt, and terminal status (Completed/Failed/Cancelled).

This conflation caused several problems:

- The dashboard showed a flat list of task runs with no persistent team view.
- There was no way to update an agent's configuration (model, tools, skills) without creating a new entity.
- Agents had no persistent identity across tasks — each run was a fresh entity with duplicated configuration.
- There was no concept of a "team" — agents existed in isolation with no grouping or coordination structure.

## Decision

### Rename current Agent to Session

The existing `Agent` entity (which tracks a single task execution with states Created -> Prompted -> Running -> ... -> Completed/Failed/Cancelled) is renamed to **Session**. A Session represents one task run: it has a prompt, turns, tool calls, and a terminal outcome.

### New Agent entity — persistent team member identity

A new **Agent** entity represents a named role on a team. It carries:

- Identity: name, role, description, soul_id
- Team membership: team_id
- Session configuration template: model, provider, tools_enabled, max_turns, skill_ids

Agents have a simple lifecycle: Created -> Active -> Archived. Sessions are spawned from Agents to execute tasks, inheriting the agent's configuration.

### New Team entity — group of agents

A new **Team** entity groups agents working on a project. It carries:

- Identity: name, description
- Project link: harness_id (links to a Harness for convention injection)

Teams have the same lifecycle as Agents: Created -> Active -> Archived.

### Relationships

```
Team (1) ──has──> (N) Agent (1) ──spawns──> (N) Session
                        │
                        └── soul_id ──> Soul
                        └── skill_ids ──> Skill[]
```

## Consequences

- **Clean taxonomy.** Session = ephemeral task run. Agent = persistent team member. Team = organizational grouping. No conflation.
- **Dashboard shows a real team.** The Observe UI can render a team page with agent cards, each showing recent sessions, status, and configuration.
- **Sessions link to agents.** Every Session carries an `agent_id` field, enabling queries like "show me all sessions for the SWE agent" or "what's Ren's average session duration?"
- **Agents persist across tasks.** Changing an agent's model or tools updates one entity; all future sessions inherit the new config.
- **Backward compatible.** Existing proof scripts and WASM integrations that reference `Agent` are updated to reference `Session`. The OData entity set name changes from `Agents` to `Sessions`.
- **Cedar policies split cleanly.** Agent identity policies (who can configure/archive agents) are separate from session execution policies (who can prompt/pause/cancel sessions).
