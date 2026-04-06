# paw-foresight

Product direction engine. Simulates a product's future by stepping through time, spawning probe agents to observe projected states, and escalating confirmed observations into actionable directions.

## Entity Types

### ProductModel
Living knowledge graph of a product. One per project. Aggregates signals from repo, monitoring, and external sources.

- **States**: Created -> Seeding -> Active <-> Stale
- **Key actions**: `Seed` (repo_url, signal_source_config), `SeedComplete`, `RefreshSignals`, `MarkStale`, `Reactivate`
- **WASM**: `seed_model` (crawls signal sources, builds graph snapshot)

### Projection
Temporal clock that advances through adaptive time steps. Probe agents observe the projected state at each step.

- **States**: Created -> Running -> Complete / Branched / Failed
- **Key actions**: `Configure` (product_model_id, horizon, max_steps, step_schedule), `Start`, `ProbesReady`, `StepComplete`, `AdvanceStep`, `Branch`
- **WASM**: `spawn_probes` (creates probe agents), `advance_step` (steps the simulation forward)
- **Step loop**: Start -> spawn_probes -> ProbesReady -> advance_step -> StepComplete -> AdvanceStep (repeats) -> Complete

### Observation
Gate-based observation lifecycle. Probe agents record observations; a different probe must confirm before escalation.

- **States**: Created -> Confirmed -> Escalated / Faded
- **Key actions**: `Record` (content, importance, signal_refs, counterfactual), `Confirm` (Cedar enforces confirmer != creator), `Escalate` (promotes to Direction), `Fade`
- Gate 1: Any probe can Record (free). Gate 2: Different probe must Confirm.

### Direction
Proposed product direction with validation depth. Emerges from escalated observations, passes through peer review and implementation.

- **States**: Proposed -> UnderReview -> Implementing -> Implemented -> Selected / Archived
- **Key actions**: `Propose`, `SubmitForReview`, `RecordReview`, `RecordConfirmation`, `MarkCrossModelAgreement`, `BeginImplementation`, `ImplementationComplete`, `Select`
- Human selects from Implemented directions. `Select` is the final approval.

### DirectionFeedback
Blind review feedback on Directions. Cedar enforces reviewer cannot be the proposer.

- **States**: Open -> Addressed -> Resolved / Reopened
- **Key actions**: `Submit` (category, severity, description), `Address`, `Resolve`, `Reopen`

## Setup

Depends on `paw-agent` for probe agents. Start by creating a ProductModel with `Seed`, then create Projections to simulate futures.
