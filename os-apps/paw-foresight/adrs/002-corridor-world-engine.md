# ADR-002: Corridor world engine replaces the projection-probe engine

Status: Accepted
Date: 2026-06-11

## Context

The 0.1 engine forecasts by forward simulation: probes observe a projected state, a convergence analyst clusters observations, a model projector evolves the state, and the loop repeats (ADR-001). Two findings from running it:

1. Forward stepping compounds model bias. Each step re-anchors on the previous step's already-modal text. The meta-improvement loop (runs 000–010, then v4 runs 001–002) plateaued: structural changes moved Specificity but Progression — genuine temporal development — never improved, because the generative direction itself was the limit.
2. Output is never graded by reality. Judge tournaments compare outputs against each other, not against what happened. The loop converged on ties.

## Decision

Replace forward simulation with a corridor architecture:

- The present contributes only determined facts (skeleton EventNodes, surveyor-authored, prediction forbidden).
- Futures are sampled as endpoint document bundles under solver-assigned driver configurations.
- Generation runs backward: repairers derive the events an endpoint requires; adversaries attack them; bounded actor micro-sims (one question, structured verdict, no chaining) settle incentive disputes.
- Plausibility is repair cost, aggregated deterministically in WASM from flags raised during repair. Models never score their own work.
- Near-term implied claims are registered as immutable Forecast entities and graded on resolution. Hindcast runs replace judge tournaments as the engine's fitness signal.

Nine entity types (World, EventNode, Endpoint, Path, Artifact, Dweller, Forecast, Hindcast, Lens), four WASM module families (clock, solver, evidence, renderer), seven souls (surveyor, bookmaker, endpoint-writer, repairer, adversary, dweller, actor). Full design: docs/corridor-world-engine-rfc.md.

## Consequences

- The five 0.1 entity types and seven WASM modules are removed (mechanics in ADR-003).
- LLM sessions hold no authority over state, time, probability, or scoring; every contract an agent must honor is an entity action schema, not prose (the 0.1 lesson: prose composition contracts were ignored).
- Multi-step forward chains are structurally impossible: no module feeds one micro-sim's output into another as a premise.
- The engine gains an external, objective fitness signal (hindcast scores, forecast Brier) at the cost of slower iteration: grading requires either waiting for reality or building frozen-corpus hindcasts.
- Run cost shifts from probe-steps to endpoint count × repair depth; budget ceilings live on the World entity.
