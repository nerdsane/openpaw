# paw-foresight

Product direction engine. Simulates a product's future through temporal projection — spawning independent probe agents to observe projected states, detecting convergence across probes, evolving the model through simulated time, and presenting actionable directions a PM can choose between.

## How the Substrate Works

The Foresight Engine is a **substrate, not a pipeline**. Probes are the simulation — they project what would happen, and their convergent projections become the next step's reality. Quality emerges from structural gates: independent observation, semantic convergence detection, and direction versioning.

```
                    ┌──────────────┐
                    │ ProductModel │ (knowledge graph: code, PRs, monitors, README)
                    └──────┬───────┘
                           │ Seed → seed_model WASM
                           ▼
STEP 0              ┌─────────────┐
(day 1)             │  Projection │ → Start → spawn_probes
                    └──────┬──────┘
              ┌────────────┼────────────┐
              ▼            ▼            ▼
         Probe-1      Probe-2      Probe-3    (independent, 1 Direction each)
         Observations  Observations  Observations
         1 Direction   1 Direction   1 Direction
              │            │            │
              └──── ProbeStepDone ──────┘  (self-report to Projection)
                           │
                    handle_probe_done WASM
                    (checks: all reported?)
                           │
                           ▼
                 ┌───────────────────┐
                 │ Convergence       │  (LLM agent: confirms/contradicts)
                 │ Analyst           │  (produces projected state)
                 └─────────┬─────────┘
                           │ ConvergenceComplete
                           ▼
STEP 1          handle_convergence WASM
(day 3)         (respawns probes with projected state + episodic memory)
                           │
              ┌────────────┼────────────┐
              ▼            ▼            ▼
         Probe-1      Probe-2      Probe-3
         sees evolved  sees evolved  sees evolved
         world         world         world
         revises or    revises or    revises or
         doubles down  doubles down  doubles down
              │            │            │
              └──── ProbeStepDone ──────┘
                           │
                    ... repeat until max_steps ...
                           │
                           ▼
                    Projection.Complete
```

**Key properties:**
- Probes work independently — they MUST NOT read each other's observations
- 1 Direction per Probe — want more perspectives, run more Probes
- Directions are versioned: each step archives the old Direction, creates a revision with `parent_direction_id`
- The model evolves through simulated time via convergent projections
- Convergence is detected by a separate LLM analyst, not string matching

## Entity Types

### ProductModel
Living knowledge graph of a product. Aggregates signals from GitHub (code, PRs, issues, commits, README), Datadog (monitors, events), and Temper (alert history).

- **States**: Created → Seeding → Active ↔ Stale
- **Key actions**: `Seed`, `SeedComplete`, `RefreshSignals`, `MarkStale`, `Reactivate`
- **WASM**: `seed_model` (crawls signal sources, builds JSON knowledge graph in TemperFS)

### Projection
Temporal simulation that advances through adaptive time steps. Event-driven — no polling.

- **States**: Created → Running → Complete / Branched / Failed
- **Key actions**: `Configure`, `Start`, `ProbesReady`, `ProbeStepDone`, `ConvergenceComplete`, `AdvanceStep`, `Complete`
- **WASM**: `spawn_probes`, `handle_probe_done`, `handle_convergence`
- **Flow**: Start → spawn_probes → ProbesReady → (probes run) → ProbeStepDone × N → handle_probe_done → Convergence Analyst → ConvergenceComplete → handle_convergence → AdvanceStep → (repeat)

### Observation
Probe agents record what they see in the projected state.

- **States**: Created → Confirmed → Escalated / Faded
- **Key actions**: `Record`, `Confirm` (by Convergence Analyst), `Escalate`, `Fade`

### Direction
Product direction proposed by a Probe. Versioned across steps.

- **States**: Proposed → UnderReview → Implementing → Implemented → Selected / Archived
- **Key actions**: `Propose`, `Archive` (when superseded by revision), `Select` (human)
- **Fields**: `parent_direction_id` (links to previous version), `step_at`

### DirectionFeedback
Blind review feedback on Directions.

- **States**: Open → Addressed → Resolved / Reopened

## Setup

Depends on `paw-agent` for probe sessions and `paw-fs` for knowledge graph storage.

```bash
# Seed a ProductModel
python3 scripts/seed_foresight.py --base-url http://127.0.0.1:3469 --tenant default --start --probe-count 3
```

Probes can use either Anthropic (`claude-sonnet-4-6`) or OpenAI (`gpt-5` via Codex) — auto-detected from environment.
