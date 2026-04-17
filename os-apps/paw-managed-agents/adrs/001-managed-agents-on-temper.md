# ADR-001: Anthropic Managed Agents API on Temper

**Status:** Accepted
**Scope:** entity-types, integrations, policies, field-invariants
**Author:** codex
**Date:** 2026-04-15
**Related:** Temper ADR-0041, OpenPaw ADR-0005

## Context

OpenPaw needs a Temper-native implementation of Anthropic's
`managed-agents-2026-04-01` beta shape. The closest upstream reference is
Crucible in the Temper repo, but Crucible uses a polling sidecar and includes
API divergences that do not fit OpenPaw's entity-first execution rule.

## Decision

Use three API-facing root entities:

- `ManagedEnvironment`
- `ManagedAgent`
- `ManagedSession`

Represent nested arrays as child entities so the API remains queryable over
OData without introducing a custom orchestration layer or a parallel REST
facade:

- `AgentMcpServer`
- `AgentSkill`
- `AgentTool`
- `AgentToolConfig`
- `SessionEvent`
- `SessionResource`
- `EnvironmentPackage`

Drive execution through WASM integrations only:

- `session_orchestrator` bridges managed sessions to `OpenPaw.Session`
- `managed_agent_updater` keeps the inner `OpenPaw.Agent` in sync
- `event_emitter` translates inner session results into `SessionEvent` rows
- `session_terminator` performs cleanup and emits termination events

`ManagedEnvironment` is a reusable sandbox template. It stores networking and
package policy that gets forwarded into the inner session's sandbox
configuration. The actual sandbox remains ephemeral and is provisioned lazily
by the existing `paw-agent` runtime when a tool first needs one.

Adopt ADR-0041-style field invariants and cross-invariants so validation
lives in the app specs, not in imperative Rust glue. This app is the first
OpenPaw app to use field invariants as a primary API-contract mechanism.

## Consequences

### Positive

- The feature is Temper-native and inspectable through entity transitions.
- Crucible's invariant patterns can be reused without importing its sidecar.
- The implementation reuses the battle-tested OpenPaw agent/session loop.
- OData remains the only public protocol surface for now, which keeps the app
  aligned with Temper primitives.

### Negative

- The app introduces several child entity types to represent nested API data.
- Bridging to `OpenPaw.Session` means some upstream API features are mapped
  into prompts or metadata until the inner session app grows native support.

### Risks

- The managed-agents API is still beta and may evolve.
- The inner-session bridge must stay aligned with the sandbox fields accepted
  by `paw-agent`'s `Session.Configure` action.
