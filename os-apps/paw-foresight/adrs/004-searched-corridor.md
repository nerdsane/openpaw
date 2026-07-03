# ADR-004: The searched corridor — claims, routes, and priced amendment

Status: Accepted
Date: 2026-06-12

## Context

The 0.2 corridor (ADR-002) prices each imagined future as a whole: one
repairer derives one linear chain from the endpoint bundle back to the
skeleton, one adversary challenges it, and the deterministic cost — a flat
sum over flags — is the verdict on the entire future. Three limits showed up
in the first live runs:

1. The verdict is a scalar. A 16KB future containing six separable claims
   gets one number; nothing says which parts of the future survive scrutiny
   and which need miracles.
2. The cost is the toll on the first road tried. No alternative route is
   explored when a link draws an expensive flag, no revision happens after
   the adversary speaks, so "repair cost" overstates the cheapest connection.
3. Nothing amends. The endpoint is frozen; the gap between the imagined
   future and the reachable one is priced but never closed. The useful
   output — the nearest *reachable* neighbor of the imagined world — is
   never produced.

Fixing (3) naively creates a worse problem: if repair may amend the future
freely, the cheapest repair is always to amend it back to the consensus
future, and the engine collapses into the forward-projection behavior the
corridor exists to avoid.

## Decision

paw-foresight 0.3 restructures the middle of the corridor:

- **Claims.** After an endpoint bundle is written, a decomposer session
  splits it into 3–8 separable load-bearing claims (new `Claim` entity).
  Each claim gets its own backward bridge and its own price. The world's
  verdict is a vector of per-claim verdicts, not a scalar per endpoint.
- **Routes.** A `Path` is now one route for one claim. Expensive flags
  trigger bounded search: a revision round (the repairer answers the
  adversary's specific objections, at most one revision per route) and
  alternate routes (a new repairer is briefed to beat the standing
  objections via a different mechanism, at most three routes per claim).
  Routes already costing more than the running best are pruned before any
  adversary session is spent. A claim's verdict is the cost of its cheapest
  surviving route.
- **Priced amendment (the drift constraint).** A repairer may amend a
  claim's text only through an explicit `AmendText` action carrying the
  diff and a justification, and every amendment must be flagged as
  `deformation` (a new flag kind, weight 25, severity scaled by how far the
  meaning moved). The search objective is repair cost *plus* deformation
  cost — drifting an anti-modal claim back toward consensus is never free.
  `Claim.original_text` is frozen at creation so drift is always auditable.
- **Conditional edges.** Repairers declare `depends_on` edges between
  bridge nodes (stored through the existing `EventNode.UpdateEdges`
  action). When evidence resolves a node "no", the invalidation walks the
  dependents transitively and re-prices only the affected claims.

The deterministic/judged boundary of ADR-002 is unchanged: sessions
imagine, decompose, propose routes, and flag costs; WASM owns pricing,
pruning, route budgets, and every state transition.

## Constraints carried forward (governing commitments)

Recorded here so they survive contributors and rewrites; ADR-002 readers
are pointed here.

1. Cost constants (kind weights, severity multipliers, the exp(−cost/25)
   decay, the 2×min+20 ceiling, the 0.95/0.05 resolve thresholds, and the
   deformation weight) are **tunable priors**. The ordering — miracle >
   contradiction > incentive ≥ lag — is the design claim; the values await
   calibration against the hindcast library and must never be treated as
   final.
2. Resolution of authored claims is a **judged step**: an adjudicator
   session with a rubric, snapshotted evidence, and a confidence threshold,
   escalating to a human below it. Never a string match or a numeric
   threshold over text.
3. Evidence adapters are **pluggable**. Polymarket and Kalshi are the v1
   adapters, not the architecture.
4. **Amendment is never free.** Any change to a claim's text goes through
   the diff-carrying action and is priced as deformation. An engine change
   that lets repair mutate claims silently reintroduces mode collapse and
   must be rejected in review.

## Consequences

- Worst-case session count per world is bounded by entity counters
  (endpoints × claims × routes × rounds), enforced in WASM, recorded in
  every proof. Search is paid for with pruning: routes that cannot win are
  cut before adversary spend.
- Forecasts now attach to per-claim bridge nodes; the notary
  (register_forecasts) is unchanged.
- The 0.2 single-chain worlds remain readable (in-place evolution per
  ADR-003); `Path` rows without a `claim_id` are 0.2 residue and stay
  valid.
- Dweller contradiction reports (ADR-002's stress-test channel) gain a
  concrete sink: they append to the affected claim's challenge flags and
  participate in pricing.
