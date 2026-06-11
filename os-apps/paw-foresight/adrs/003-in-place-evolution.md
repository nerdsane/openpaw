# ADR-003: In-place evolution of paw-foresight (0.1 → 0.2)

Status: Accepted
Date: 2026-06-11

## Context

The corridor engine (ADR-002) replaces every entity type and WASM module in this app. The app is installed on the production server with live 0.1 entities (ForesightModels, Projections, Observations, Directions) in the event store. Platform install mechanics (`install_os_app`, temper-platform/src/os_apps/mod.rs) determine what an upgrade can and cannot do:

- Specs are upserted by entity-type name and never deleted; specs absent from a new bundle persist in the registry and store.
- The tenant CSDL is merged across all installed apps.
- Cedar bundle policies are appended to the tenant policy text.

## Decision

1. **Same app identity.** The install record is keyed by app name; renaming would orphan it and double-register. Version bumps 0.1.0 → 0.2.0 (documentation; reconcile is digest-driven).
2. **No entity-type name reuse.** All nine new types use names disjoint from the five old ones. Reusing a name would upsert over a spec with live instances whose persisted states may not exist in the new automaton — undefined recovery for zero benefit. Old specs and entities remain as inert, queryable residue.
3. **Old surface retired by Cedar, not deletion.** Removing spec files does not remove specs; old actions would remain invocable with their WASM gone. The bundle ships a retirement policy denying Create and all mutating actions on the five old types, **scoped to non-system principals** so that an image rollback (which reinstalls the old bundle by digest and restores old WASM) leaves old system flows functional.
4. **Removal is one dedicated, revertable commit** (five spec files, seven WASM crates), landed only after a local rehearsal: seed old entities locally, restart with the new bundle, verify clean boot, old entities readable, reconcile metrics clean. The same rehearsal gates the production deploy, preceded by a Postgres snapshot and a check that no old entity is in a non-terminal state.
5. **Cross-app name-collision gate.** Because the tenant CSDL is merged, new entity names were grepped against every app's specs before being fixed (2026-06-11: zero collisions among World, EventNode, Endpoint, Path, Artifact, Dweller, Forecast, Hindcast, Lens). A CI test asserting tenant-wide entity-name uniqueness accompanies this change. (The audit surfaced a pre-existing collision — WorkCycle in paw-harness and paw-patrol — tracked separately.)

## Consequences

- Registry and dashboard carry five retired entity types indefinitely; this is the platform's designed behavior, accepted in exchange for boot-safe upgrades and symmetric rollback.
- Effective tenant Cedar policy accumulates old foresight text; the new policy file is authored as a complete replacement-intent document, and the effective policy is verified (and pruned via the policy APIs if contradictory) as part of the production install proof.
- Rollback plan is a previous-image redeploy; new specs persist harmlessly under the old bundle for the same reason old ones persist under the new.
