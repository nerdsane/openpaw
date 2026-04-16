# Run 007 Plan

## Target Criteria

- **Breadth**: Engine Borda 3.0 vs Baseline 6.0 — unchanged for 3 consecutive runs (004-006). Root cause: directions cluster on governance themes because the orchestrator ignores prose-based diversity constraints (proven across 5 runs).
- **Actionability**: Engine Borda 3.5 vs Baseline 5.5 — secondary deficit.

## Diagnosed Root Cause

Prose instructions in the orchestrator's user_message are advisory — the orchestrator shortcuts to completion (spawn probes, grab data, synthesize in-context, done). Convergence, direction consolidation, and synthesis delegation are consistently skipped. This has been proven across Runs 001-006. Adding more text to the orchestrator instructions will not fix this.

The directions produced by probes cluster on governance themes because probes have no theme constraints. The orchestrator was supposed to apply theme diversity post-hoc (via direction consolidation), but it never executes that step.

## Planned Change

**Move probe creation from the orchestrator into the WASM layer, with hard-coded theme-constrained personas.**

Instead of the orchestrator deciding how to create probes (and ignoring structural constraints), the WASM `spawn_orchestrator` module will:

1. Create 6 probe sessions directly, each with a hard-coded prompt that includes:
   - The projection_id and ForesightModel ID
   - Step number (0 or 1) and time range
   - Persona (practitioner, critic, adjacent-domain)
   - **MANDATORY theme constraint** for directions
2. Create 1 orchestrator session with simplified instructions: just wait for probes and synthesize

### Probe Theme Assignments

| Probe | Step | Time Range | Theme Constraint |
|-------|------|-----------|-----------------|
| Practitioner | 0 | 0-180 days | technical-architecture OR evaluation/testing |
| Practitioner | 1 | 180-365 days | technical-architecture OR evaluation/testing |
| Critic | 0 | 0-180 days | economics/market OR organizational/adoption |
| Critic | 1 | 180-365 days | economics/market OR organizational/adoption |
| Adjacent-Domain | 0 | 0-180 days | cross-domain (analogies from other fields) |
| Adjacent-Domain | 1 | 180-365 days | cross-domain (analogies from other fields) |

### Expected Direction Distribution

- 4 directions on technical-architecture / evaluation/testing (practitioner probes)
- 4 directions on economics/market / organizational/adoption (critic probes)
- 4 directions on cross-domain patterns (adjacent-domain probes)
- 0-2 governance directions (only if probes organically include governance as secondary)

This guarantees at least 4 distinct themes across 12 directions without relying on the orchestrator.

## Expected Impact

- **Breadth**: E should improve from 3.0 to 4.5-6.0 Borda (directions span 4+ themes structurally)
- **Actionability**: May improve slightly if diverse directions produce more operational decision points
- **Overall Borda**: Engine should increase from 55 to 57-60 range
- **Risk**: Probes may struggle to access knowledge graph (File entity 404 issue from Run 006). Mitigation: probe prompts include ForesightModel ID for direct access + instructions to use web_search as fallback.

## What This Change Does NOT Touch

- Synthesis template (unchanged from Run 006)
- Entity specs (no new states or actions)
- Scoring/judging methodology (same 3-judge protocol)
