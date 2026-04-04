# ADR-0013: Foresight Engine

## Status

Accepted

## Context

OpenPaw's self-heal loop (alert → SRE triage → Developer fix → PR) is reactive — it responds to problems after they occur. The next capability layer is proactive: projecting a product's plausible futures, surfacing the strongest directions, and delivering them as working code branches for a human to select.

This engine is inspired by two existing systems:

- **Deep Sci-Fi** — a platform for peer-reviewed science fiction worldbuilding. Agents propose futures with causal chains, other agents validate them through blind review, and approved worlds become shared spaces. Key insight: quality emerges from minimal structural gates (peer confirmation, blind review, graduation), not scoring functions.

- **MiroFish** — a multi-agent simulation engine that seeds knowledge graphs from real data, generates agent personas, and runs temporal simulations. Key insight: time is the substrate — state evolves through rounds, agents act within evolving state, emergence is implicit in the interaction model.

Neither system produces working code. Neither operates on real codebases. Foresight synthesizes their architectural insights into something new: a substrate where Probe agents inhabit a product's projected future and surface implementable directions.

## Decision

Build Foresight as a Temper-native OS app (`paw-foresight`) with 5 entity types forming a substrate. Probes are regular Agent+Session entities from `paw-agent` that interact freely with the substrate through standard Temper tools.

### Substrate, Not Pipeline

Entities define lifecycle. Probes interact freely. No prescribed phases, turn orders, or action types. This is the core design choice — a pipeline constrains better models, a substrate rewards them.

### Minimal Gates (from Deep Sci-Fi)

1. **Observation** — any Probe records freely (no friction)
2. **Confirmation** — a different Probe confirms it matters (Cedar enforced: confirmer != creator)
3. **Direction** — confirmed observations escalated with reasoning + grounding
4. **Review** — blind peer review (reviewers can't see each other's feedback)

No binary graduation gate. Validation depth (confirmation_count, review_count, cross_model_agreement) is a structural fact. The human decides when it's sufficient.

### Probes ARE the Simulation

There is no system that pre-computes the future. Probes read the ProductModel (real signals from GitHub, Datadog, codebase analysis, AlertCycle history) and project what happens next. The advance_step WASM is a minimal timing coordinator — it gives the time horizon and the ProductModel ID. Probes query data themselves and decide what matters. No pre-digested summaries, no anchoring.

### Multi-Model Probes

Probes can run on different models (configured per Projection via probe_config). Cross-model convergence — where fundamentally different reasoning engines independently flag the same area — is the strongest signal the engine can produce.

### No Prescribed Reasoning Format

Directions use free-text `reasoning` (model organizes thinking however it wants) plus structured `grounding` (JSON references to ProductModel signals). This avoids imposing linear causal chains on potentially non-linear reasoning. Better models may think in networks, probability graphs, or multi-path scenarios.

## Alternatives Considered

**Pipeline architecture** — prescribed phases (entropy computation → emergence detection → probe observation → counterfactual testing). Rejected: phases are a ceiling on model capability. A substrate has no ceiling.

**Scoring functions for convergence** — algorithmic quality assessment. Rejected: kills emergent importance. From DSF analysis: "If you added a scoring function, you'd lose the collaborative narrative-building."

**Prescribed lenses** — assign each Probe a focus area (reliability, DX, cost). Rejected: limits model creativity. Probes develop focus naturally through what they notice.

**System-computed projection** — arithmetic trend extrapolation as the projected future. Rejected: creates an arithmetic ceiling. The projection quality should be the model quality.

**Fixed graduation threshold** (2+ reviewers, all feedback resolved). Rejected: arbitrary at scale. Elastic validation depth scales from 3 to thousands of Probes without code changes.

## Consequences

- More entity types to maintain (5 in paw-foresight)
- Probes need sufficient context window for ProductModel reasoning
- Multi-Probe projections have LLM cost (multiple sessions per step)
- Validation depth as spectrum (not binary gate) requires PM judgment
- Cedar policy for blind review adds complexity
- Entity structure rewards model capability — value increases as models improve
