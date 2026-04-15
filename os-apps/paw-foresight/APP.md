# paw-foresight

Domain-agnostic foresight engine. Simulates a domain's future through temporal projection — spawning independent probe agents to observe projected states, detecting convergence across probes, evolving the model through simulated time, and presenting actionable directions.

## How the Substrate Works

The Foresight Engine is a **substrate, not a pipeline**. Probes are the simulation — they project what would happen, and their convergent projections become the next step's reality. Quality emerges from structural gates: independent observation, semantic convergence detection, and direction versioning.

```
                    ┌────────────────┐
                    │ ForesightModel │ (knowledge graph JSON — schema-free, domain-specific)
                    └──────┬─────────┘
                           │ Seed → spawn_seed_agent WASM (spawns agent session)
                           ▼
                    ┌─────────────┐
                    │  Projection │ → Start → spawn_orchestrator WASM
                    └──────┬──────┘
                           │
                    ┌──────┴──────┐
                    │ Orchestrator│ (agent session with orchestrate-projection skill)
                    │ runs the    │
                    │ full loop:  │
                    └──────┬──────┘
                           │
          for each step in step_schedule:
                           │
              ┌────────────┼────────────┐
              ▼            ▼            ▼
         Probe-1      Probe-2      Probe-3    (independent sessions)
         practitioner  critic        adjacent   (differentiated prompts)
         Observations  Observations  Observations
         1 Direction   1 Direction   1 Direction
              │            │            │
              └────────────┴────────────┘
                           │
                    Orchestrator reads observations directly,
                    does convergence, writes projected state,
                    dispatches entity actions for audit trail,
                    advances step, repeats
                           │
                           ▼
                    Orchestrator writes final synthesis
                    Projection.Complete
```

**Key properties:**
- Probes work independently — they MUST NOT read each other's observations
- 1 Direction per Probe — want more perspectives, run more Probes
- Directions are versioned: each step archives the old Direction, creates a revision with `parent_direction_id`
- The orchestrator reads observations directly as entities (no serialization bugs)
- Convergence is done by the orchestrator, not a separate session
- Intelligence is in the skill, not WASM — see `system/skills/orchestrate-projection/SKILL.md`

## Entity Types

### ForesightModel
Living knowledge graph for any domain. The `model_type` field determines the domain (software_product, knowledge_domain, business, etc.). The JSON structure is schema-free — whatever the seed agent produces.

- **States**: Created → Seeding → Active ↔ Stale
- **Key fields**: `name`, `model_type`, `signal_source_config` (domain-specific JSON), `seed_model`, `seed_provider`, `seed_soul_id`
- **Key actions**: `Seed`, `SeedComplete`, `RefreshSignals`, `MarkStale`, `Reactivate`
- **WASM**: `spawn_seed_agent` (spawns an agent session to build the knowledge graph — bitter lesson applied)

### Projection
Temporal simulation that advances through adaptive time steps. Event-driven — no polling.

- **States**: Created → Running → Complete / Branched / Failed
- **Key actions**: `Configure`, `Start`, `ProbesReady`, `ProbeStepDone`, `ConvergenceComplete`, `ProjectionUpdated`, `AdvanceStep`, `Complete`
- **WASM**: `spawn_orchestrator` (creates orchestrator session with skill)
- **Skill**: `orchestrate-projection` (the orchestrator reads this to run the full loop)
- **Flow**: Start → spawn_orchestrator → Orchestrator session runs loop → ProbesReady → ProbeStepDone×N → ConvergenceComplete → ProjectionUpdated → AdvanceStep → (repeat) → Complete

### Observation
Probe agents record what they see in the projected state.

- **States**: Created → Confirmed → Escalated / Faded
- **Key actions**: `Record`, `Confirm` (by Convergence Analyst), `Escalate`, `Fade`

### Direction
Direction proposed by a Probe. Versioned across steps.

- **States**: Proposed → UnderReview → Implementing → Implemented → Selected / Archived
- **Key actions**: `Propose`, `Archive` (when superseded by revision), `Select` (human)
- **Fields**: `parent_direction_id` (links to previous version), `step_at`

### DirectionFeedback
Blind review feedback on Directions.

- **States**: Open → Addressed → Resolved / Reopened

## Setup

Depends on `paw-agent` for probe sessions and `paw-fs` for knowledge graph storage.

Model and provider are configured per-probe in `probe_config` (array of `{name, model, provider}`). The seed agent's model/provider are fields on the ForesightModel entity itself (`seed_model`, `seed_provider`).
