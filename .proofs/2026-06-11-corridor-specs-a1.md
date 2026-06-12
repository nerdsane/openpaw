# Proof: Corridor entity specs coexist with the legacy engine (A1)

**Date:** 2026-06-11
**Branch:** codex/corridor-engine (worktree off main @ 8a1d5011)
**Scope:** ADR-002 entity surface — nine corridor specs, CSDL, Cedar, stub WASM crates — booting alongside the five legacy paw-foresight entity types.

## What was built

- Nine entity specs: `world`, `event_node`, `endpoint`, `path`, `artifact`, `dweller`, `forecast`, `hindcast`, `lens` (`os-apps/paw-foresight/specs/*.ioa.toml`)
- `model.csdl.xml` extended with the nine entity types, their bound actions, and entity sets (legacy five retained)
- `policies/foresight.cedar` extended with corridor separations: Forecast create/resolve/score system-only; Path scoring/classification system-only; Artifact PassCheck/Publish/Retcon system-only; EventNode Confirm forbidden to its author; Dweller animated only by its backing agent; track records system-graded
- `app.toml`: ten corridor `[[wasm_modules]]` declared; `startup_install = "core"` added (deploys become self-upgrading); legacy modules remain auto-discovered until the ADR-003 removal commit
- Ten stub WASM crates (compile, log, and fail loudly if dispatched before A3 implements them)

## Red → green

- `crates/temperpaw/tests/corridor_engine_contract.rs` written first: **7 failed / 1 passed** (pre-implementation)
- After implementation: **9 passed / 0 failed** (includes the cross-app entity-name uniqueness guard with the pre-existing WorkCycle collision allowlisted, and the CSDL-serves-all-sets test added after the gate finding below)

## Live boot verification (local)

Server: `target/debug/temperpaw-server`, `TEMPERPAW_WASM_STARTUP_POLICY=build`, fresh Turso db, port 4500.

- Boot clean; `/readyz` 200.
- Install log: `Installed os-app 'paw-foresight' ... added=["Artifact", "Direction", "DirectionFeedback", "Dweller", "Endpoint", "EventNode", "Forecast", "ForesightModel", "Hindcast", "Lens", "Observation", "Path", "Projection", "World"]` — 9 new + 5 legacy types registered together.
- OData: **14/14 entity sets serve** (`Worlds`…`Lenses` + `Projections`…`DirectionFeedbacks`), verified with admin principal + bearer key.
- Smoke create: `POST /tdata/Worlds {"name":"a1-smoke","domain":"test"}` → `entity_id en-019eb6aa-dc8c-7bb0-98bb-72313275361a`, status `Created`.

## Findings worth recording

1. **CSDL is the serving surface.** A spec without a `model.csdl.xml` entry registers (install log shows it added) but its entity set 404s. Pinned by the `csdl_serves_all_corridor_entity_sets` contract test.
2. **Cedar is deny-by-default per entity type.** New types returned `AuthorizationDenied` until the corridor policy landed — confirms the A5 premise that policies must be active during all local e2e, and that the ADR-003 retirement-by-Cedar mechanism will actually bite.
3. **paw-foresight was never a startup app.** It lacked `startup_install = "core"`, so local boots (and prod deploys) don't reconcile it automatically. Flag added; this is also the mechanism A7 relies on for the prod upgrade.
4. Pre-existing cross-app entity-name collision (WorkCycle: paw-harness vs paw-patrol) surfaced by the new uniqueness guard; allowlisted with a tracking note, fix spun off separately.

## Residual

- Stub WASM modules intentionally fail if dispatched (A3 implements them).
- Cedar matrix tests (A5) will adversarially exercise the separations added here; CheckerVerdict is deliberately loose until then.
