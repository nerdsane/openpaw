# ADR-003: System Skill Full Injection + App-First Posture

**Status:** Accepted
**Scope:** integrations
**Author:** seshendra
**Date:** 2026-04-09

## Context

The README describes agents that proactively create Temper apps when capabilities are missing: "When an agent needs a capability that doesn't exist, it designs one." The reference material for this behavior already existed in two system skills (`platform-awareness` and `temper-app-creation`), but system skills were injected as L0 only — agents saw name + description in an XML listing and had to explicitly call `temper.read(path)` to get the full content. Most agents never loaded them.

This meant the vision of app-first agents was aspirational documentation, not embedded behavior. The gap was structural: the mechanism that reached every agent (system skill discovery) didn't carry enough information to drive behavior.

## Decision

### 1. Full injection for system skills

Change `load_skills_block()` in `llm_caller` so that skills at `/system/skills/` inject their complete body content into every agent's system prompt — not just name, description, and path.

- System skills (scope_priority == 2): full content injected between `<skill>...</skill>` tags
- Project-scoped and agent-scoped skills: remain L0 (self-closing `<skill ... />` tags with name, description, path)

This follows the principle that system skills represent **platform knowledge all agents need** — they're the kernel's instruction set, not optional reading.

Project and agent skills remain L0 because they're contextual and potentially numerous. Progressive disclosure via `temper.read(path)` is appropriate for those scopes.

### 2. App-first content in system skills

With full injection, system skill content changes become universal behavioral changes. Three skills were updated:

- **platform-awareness**: Added "Default posture" paragraph — when a need isn't met by an installed app, design one.
- **temper-app-creation**: Updated description to be action-oriented ("your primary way to extend the platform") rather than reference-manual-like.
- **research-first-planning**: Added capability/app check to the Research phase and app-opportunity evaluation to the Plan phase.

## Consequences

### Positive

- Every Temper agent — regardless of app, soul, or configuration — now has app-first thinking in its base knowledge
- System skills become the definitive "platform instruction set" rather than optional reference material
- No per-agent or per-app configuration needed; the posture is inherited structurally
- Adding new platform-wide knowledge is now a single file change (create/update a system skill)

### Negative

- System skill content consumes prompt tokens for every agent. Currently ~3 skills; if system skills grow significantly, this becomes a token budget concern.
- The full injection means system skill content should be concise and behavioral, not encyclopedic. Large reference docs should move to project or agent scope.
- Breaking change for agents that relied on system skills being L0-only (unlikely, but possible if an agent parsed the `<available_skills>` XML expecting only self-closing tags)
