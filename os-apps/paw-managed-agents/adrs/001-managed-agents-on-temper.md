# ADR-001: Anthropic Managed Agents API on Temper

**Status:** Accepted
**Scope:** entity-types, integrations, policies, field-invariants
**Author:** codex
**Date:** 2026-04-15
**Related:** Temper ADR-0041, OpenPaw ADR-0005

## Context

OpenPaw needs a Temper-native implementation of the managed-agents API shape.
The closest upstream reference is Crucible in the Temper repo, but Crucible
uses a polling sidecar and includes API divergences that do not fit OpenPaw's
entity-first execution rule.

## Decision

Use three API-facing root entities:

- `ManagedEnvironment`
- `ManagedAgent`
- `ManagedSession`

Represent nested arrays as child entities so the API remains queryable over
OData without introducing a custom orchestration layer:

- `AgentMcpServer`
- `AgentSkill`
- `AgentTool`
- `AgentToolConfig`
- `SessionEvent`
- `SessionResource`
- `EnvironmentPackage`

Drive execution through WASM integrations only:

- `session_orchestrator` bridges managed sessions to `OpenPaw.Session`
- `event_emitter` translates inner session results into `SessionEvent` rows
- `environment_provisioner` materializes a `Paw.Compute.Computer`
- `session_terminator` performs cleanup and emits termination events

Adopt ADR-0041-style field invariants and cross-invariants so validation lives
in the app specs, not in imperative Rust glue.

## Consequences

### Positive

- The feature is Temper-native and inspectable through entity transitions.
- Crucible's invariant patterns can be reused without importing its sidecar.
- The implementation reuses the battle-tested OpenPaw agent/session loop.

### Negative

- The app introduces several child entity types to represent nested API data.
- Bridging to `OpenPaw.Session` means some upstream API features are mapped
  into prompts or metadata until the inner session app grows native support.

### Risks

- The managed-agents API is still beta and may evolve.
- `Paw.Compute.Computer` is currently a logical environment record, not a full
  remote-control plane, so environment provisioning is intentionally lightweight.
