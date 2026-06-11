# Proof: Legacy surface removal + Cedar retirement binds (A6, mechanical half)

**Date:** 2026-06-11
**Branch:** codex/corridor-engine
**Scope:** ADR-003 retirement commit — five legacy specs and seven legacy WASM crates removed, app at 0.2.0, retirement policy proven to bind after pruning the appended tenant policy.

## What was removed (one revertable commit)

- Specs: foresight_model, projection, observation, direction, direction_feedback (.ioa.toml)
- WASM crates: spawn_seed_agent, spawn_probes, handle_probe_done, handle_convergence, handle_projection_updated, advance_step, seed_model
- Cedar: 0.1 broad permits replaced by retirement policy (read/list open to all; mutation system-only for rollback symmetry)
- app.toml → 0.2.0
- CSDL keeps the legacy entity sets: residue stays readable everywhere

## Boot on the legacy-seeded store (post-removal)

- `/readyz` 200; 0 ERROR lines.
- Legacy ForesightModel readable with fields intact; new sets serve (Worlds populated, Forecasts empty list).
- Tests: corridor_engine_contract 10/10, corridor_cedar_matrix 8/8 (incl. the new legacy-retirement matrix).

## Finding: Cedar append accumulation is real (risk register #9, confirmed live)

A legacy `Projections` create as a plain agent principal returned **201** on the seeded store: installs APPEND bundle policy to the tenant policy text and never remove (verified in temper-platform `install_os_app` — `combined = existing + "\n" + bundle`), so the 0.1 permits persisted across reconciles and out-voted deny-by-default.

**Mitigation built and verified:** `scripts/prune_foresight_legacy_policy.py` — GETs the effective tenant policy, removes the retired 0.1 statements (statement-level, preserving runtime-approved decision rules), collapses exact duplicates, PUTs back via `PUT /api/tenants/{tenant}/policies` (server-validated).

Run against the seeded store: 7 statements removed (5 entity permits + 2 legacy WASM module permits), 50,969 → 48,578 bytes, `status: loaded`. Re-test: legacy create now **403**; corridor sets unaffected.

## Consequence for A7 (prod runbook)

After the prod deploy reconciles the 0.2.0 bundle, run the prune script against production (with the prod admin key) and record the removed-statement list in the prod proof. Until pruned, retirement does not bind on prod. A platform-level fix (per-app policy sections with replace-on-reconcile semantics in temper) is the durable cure — candidate follow-up, out of this effort's scope.

## Residual

- Live corridor e2e (the A6 flagship runs + hindcast) still pending — next step.
