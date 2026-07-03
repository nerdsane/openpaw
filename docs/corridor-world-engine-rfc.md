# RFC: Corridor World Engine (paw-foresight 0.2 → 0.3)

Status: Draft
Date: 2026-06-11 (0.2) · 2026-06-12 (0.3, the searched corridor)
Repos affected: temperpaw (this RFC), deep-sci-fi (consumer)

## The engine in one paragraph

Sample worlds at several fixed distances from consensus. For each, search
backward through a graph of small, dependent, individually-priceable steps
for the cheapest connection to today — allowing the world to be amended,
but pricing every amendment so that drifting back to consensus is never
free. Rank what survives. The intelligence is in the imagining, the
decomposing, and the route-proposing; the arithmetic is in the pricing and
the pruning; reality, arriving later, grades everything.

A prediction-market question is a world projected down to one coordinate;
this engine builds the joint object — the world and its dependency
structure — and the single-question forecasts fall out of it for free. The
0.3 revision (ADR-004) turns the middle of the corridor from a priced
single chain into that search: per-claim decomposition, alternate routes,
and the drift constraint.

## What we are addressing

paw-foresight 0.1 forecasts by stepping forward in time: probe agents observe a projected state, an analyst clusters their observations, a projector evolves the state, repeat. This works, but it inherits a structural weakness of language models: asked "what happens next," a model returns the consensus view of the present, and iterating that question compounds the blandness. The same weakness was observed independently across many iterations of the Deep Sci-Fi project. Forward stepping produces futures that are rigid, generic, and anchored to today's news.

A second problem: nothing grades the output. Confidence numbers are self-reported, scenario quality is judged by other models, and no prediction is ever checked against what actually happened.

## Expected end state

One engine that produces futures three ways from the same machinery:

- Near-term decision briefs (months out) whose specific claims are registered as forecasts and graded when reality resolves them.
- Deeply developed far-future worlds (decades out) whose near-term on-ramps carry the same graded claims.
- A public track record: every world shows what it predicted, what resolved, and how well.

Deep Sci-Fi 2.0 consumes this engine as its backend: a catalog of fiction worlds with receipts. The Python Deep Sci-Fi backend is discontinued.

## How it works

Generation never asks a model "what happens next." Instead:

1. **Skeleton (forward, non-generative).** A surveyor agent records what is already determined about the domain: demographics, shipped infrastructure, dated commitments, live market prices. These become EventNode entities in a world graph, stratified by rate of change.
2. **Endpoints (the future end).** Endpoint-writer agents produce documents native to the target date — postmortems, filings, reviews — under driver configurations assigned by the solver. Document imitation is in-distribution for language models in a way that prediction is not, and retrospective genres force dates, actors, and causal chains.
3. **Repair (backward).** Repairer agents work each endpoint back toward the skeleton: for this document to exist, what must have happened, by when, done by whom? Each required event is proposed; adversary agents attack the repairs; bounded actor micro-sims (one question to an agent embodying a named actor, no chaining) settle incentive disputes.
4. **Costing (deterministic).** A WASM module aggregates the flags raised during repair into a repair cost per path. Cheap paths gain probability weight; expensive ones are discarded or kept as labeled tails. No model computes its own score.
5. **Grading (reality).** Near-term claims implied by weighted paths are registered as immutable Forecast entities and graded as events resolve. Hindcast runs (seed the engine at a past date with a frozen corpus, grade against what actually happened) give the engine an objective fitness signal; engine components are kept only if they improve it.

Stories and in-world documents are Artifact entities behind a blocking consistency gate. Dweller entities (characters with persistent memory and public track records) traverse paths when worlds are built or updated — their traversals double as consistency tests, and their accumulated experience is what stories are written from.

Time lives in a clock module with empirical lag distributions; state lives in the world graph; probability lives in the scoreboard. Agents only ever do narrow jobs: record a fact, write a document, propose a repair, attack one, answer one actor question, write one story.

## Entity model (nine types)

| Entity | Role | Key states |
|---|---|---|
| World | Root; domain config, frontier date, graph snapshot ref | Seeding → Active ↔ Updating → Archived |
| EventNode | Graph node: statement, layer, probability, provenance, edges | Proposed → Confirmed → Resolved / Retired |
| Endpoint | Sampled future document bundle | Sampled → UnderRepair → Scored → Weighted / Discarded |
| Path | A repaired corridor solution with cost | Solving → Repaired → Scored → Canonical / Tail |
| Artifact | Stories and in-world documents | Drafted → ConsistencyChecked → Published → Retconned |
| Dweller | Inhabitant with memory and track record | Created → Active → Retired |
| Forecast | Preregistered marginal, immutable once registered | Preregistered → Resolved → Scored |
| Hindcast | Retrodiction run with frozen corpus | Configured → Running → Scored |
| Lens | Projection of a world onto a consumer's concern surface | Configured → Live |

All document content is stored as paw-fs file references; entity fields hold metadata only.

## What replaces what

The 0.1 entities (ForesightModel, Projection, Observation, Direction, DirectionFeedback) and their seven WASM modules are removed. The probe idea survives as actor micro-sims; the evidence-citation discipline survives in EventNode provenance; the meta-loop's judge tournaments are replaced by hindcast grading. See ADR-002 (architecture) and ADR-003 (in-place evolution mechanics).

## Out of scope

- Any component that places bets. Forecast entities are the product; trading them is someone else's program.
- The visionary/decision-support site (a later consumer of the same engine via Lens entities).
- Self-serve external agent registration for Deep Sci-Fi 2.0 (deferred; the one-door Cedar design makes it additive later).

## Considerations recorded for later

- External agents as additional repairers/adversaries/dwellers through the same governed actions, with calibration-based reputation.
- Actor micro-sim library matured into a separately hindcast-graded component (population/institution response modeling).
- Conditional decision injection ("we ship X in August") via Lens for the visionary product.
