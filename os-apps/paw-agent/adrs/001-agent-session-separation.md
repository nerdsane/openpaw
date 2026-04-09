# ADR-001: Agent and Session Separation

**Status:** Accepted
**Scope:** entity-types
**Author:** OpenPaw maintainers
**Date:** 2026-04-08

## Context

OpenPaw agents need two distinct kinds of state. One is the durable identity of an agent: its role, instructions, tools, harness bindings, and lifecycle configuration. The other is the transient execution state of a single conversation or spawned work item. Earlier iterations blurred these together, which made routing, approvals, and capability inheritance harder to reason about.

## Decision

We model persistent agent identity and ephemeral execution separately. `Agent` carries long-lived operational configuration, while `Session` carries per-run execution state such as status, delivery state, reply routing, and pending approvals. Child sessions inherit what they need from the parent agent/session binding rather than becoming new durable identities.

## Consequences

### Positive
- Durable capability configuration stays attached to `Agent` instead of being copied into every run.
- Reply routing, approvals, and resumptions can reason about execution state without mutating agent identity.
- Spawned work is easier to audit because each run has its own `Session` lifecycle.

### Negative
- Cross-entity references between `Agent` and `Session` must be kept coherent.
- Reviewers have to understand two related entity types instead of one merged abstraction.
