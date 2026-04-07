# ADR-0016: Event-Driven Temporal Simulation

## Status

Accepted

## Context

The Foresight Engine v1 (ADR-0013, ADR-0015) used a polling loop: `advance_step` WASM checked probe sessions every 15s, convergence ran asynchronously, probes were amnesiac on respawn, and the ProductModel stayed static between steps. This produced:

1. **43 redundant Directions** from 2 probes × ~3 directions × 5 steps — each respawned probe re-derived the same conclusions
2. **No simulation** — the "projected state" never changed between steps; probes stared at the same snapshot
3. **Polling waste** — 15-second poll cycles consuming WASM fuel when nothing changed
4. **No episodic memory** — probes couldn't build on prior observations

## Decision

Replace the polling loop with an event-driven simulation where:

1. **Probes self-report** via `ProbeStepDone` action on the Projection entity
2. **Convergence Analyst produces projected state** — convergent projections become the next step's reality
3. **Probes get episodic memory** — respawned with their own prior Observations and Direction
4. **One Direction per Probe** — want N directions? Run N probes

### New action chain

```
Start → spawn_probes → ProbesReady (waits)
  → Probes run independently, each calls ProbeStepDone
  → handle_probe_done WASM: all reported? → spawns Convergence Analyst
  → Analyst produces projected_state.json → calls ConvergenceComplete
  → handle_convergence WASM: respawns probes with projected state + memory → AdvanceStep
  → repeat until max_steps → Complete
```

### Projected state structure

The Convergence Analyst produces a JSON document that evolves with each step:

```json
{
  "base_model": { original knowledge graph (frozen) },
  "step_history": [
    { step, day_offset, convergent_observations, contradictions, projected_changes }
  ],
  "current_projected_state": { merged base + all projected changes }
}
```

Probes at step N see the `current_projected_state` which includes all convergent projections from steps 0 through N-1. This is the simulation — the world evolves through projected time.

## Consequences

- **No more polling** — the system is idle between probe completions, zero wasted compute
- **Probes build on prior work** — episodic memory prevents re-derivation
- **The model genuinely evolves** — each step sees a different projected world
- **Directions converge rather than accumulate** — 1 per probe, revised each step
- **Convergence Analyst becomes a blocking dependency** — it must finish before probes respawn (previously it was fire-and-forget)

## Alternatives Considered

- **Real-time stepping** (wait actual days between steps): defeats the purpose of simulation; the engine should project months in minutes
- **Polling with memory** (keep polling but add episodic context): doesn't solve the static model problem and wastes compute
- **Single monolithic WASM module** for all triggers: the temper-wasm-sdk Context doesn't expose which trigger invoked the module, so separate crates are cleaner
