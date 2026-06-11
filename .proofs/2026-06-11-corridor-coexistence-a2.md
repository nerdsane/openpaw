# Proof: Legacy entities survive the corridor bundle across restart (A2)

**Date:** 2026-06-11
**Branch:** codex/corridor-engine
**Scope:** ADR-003 rehearsal — a store containing live legacy paw-foresight entities boots cleanly under the corridor bundle. Miniature of the production upgrade.

## Procedure

1. Server running the corridor bundle (A1 state), Turso store `/tmp/corridor-a1.db`.
2. Seeded legacy entities via OData: ForesightModel `en-019eb6ac-056a…` ("legacy-rehearsal"), Projection `en-019eb6ac-0585…`, Observation `en-019eb6ac-059d…` — store now holds legacy spec rows AND legacy instances, plus a corridor World.
3. Stopped the server; restarted on the SAME store.

## Results

- `/readyz` 200 within 5s of listening.
- Reconcile: `OS app unchanged; skipping hot reconcile` (digest `sha256:87f8323…`) — digest-driven reconcile behaves as ADR-003 expects.
- Legacy ForesightModel readable by id after restart, fields intact (`name = legacy-rehearsal`, status `Created`).
- Counts post-restart: ForesightModels 1, Projections 1, Observations 1, Worlds 1 — both generations coexist in one store.
- Boot log: 0 ERROR lines.

## What this de-risks

The A6 removal commit and the A7 production deploy both depend on exactly this: old spec rows reload from the store, old instances stay readable, and the new bundle installs around them. Re-run this rehearsal after the A6 removal commit (old spec FILES gone) before any prod deploy.
