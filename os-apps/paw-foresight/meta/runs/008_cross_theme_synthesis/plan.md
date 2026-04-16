# Run 008 Plan

## Target Criteria

- **Breadth**: Engine scored 2.0/4 avg (Borda 3.0), Baseline scored 3.0/4 avg (Borda 6.0). Root cause: synthesis treats themes independently; cross-theme interactions are absent or superficial. This is the persistent deficit across Runs 004-007.

## Planned Change

**Add a mandatory cross-theme interaction section to the synthesis template** (SYNTHESIS_TEMPLATE in `wasm/spawn_orchestrator/src/lib.rs`).

Between Active Directions (Step C) and Source Thesis Challenges (Step F), insert a new **Step C2: Cross-Theme Interactions**. This section forces the synthesizer to:

1. Identify 4-5 pairs of themes from the directions and observations
2. For each pair, derive a **non-obvious conclusion** that neither theme implies alone
3. Each interaction must cite observations from at least 2 different probes
4. Each interaction must produce a specific, falsifiable prediction

This directly targets the Breadth rubric anchor for 3: "6+ themes with explicit cross-theme interactions where the interaction produces a non-obvious conclusion."

**File changed:** `os-apps/paw-foresight/wasm/spawn_orchestrator/src/lib.rs` (SYNTHESIS_TEMPLATE constant)

**What stays the same:** Probe creation, theme enforcement, orchestrator flow, all other synthesis template sections. ONE change only.

## Expected Impact

- **Breadth**: Should improve from 2.0 to 2.5-3.0 avg. The explicit cross-theme section provides the "non-obvious conclusions" the rubric requires for a 3.
- **Novelty**: May indirectly improve — cross-theme reasoning can surface insights neither theme produces alone.
- **Other criteria**: Should hold steady. Falsifiability, Transparency, Quant Precision (engine strengths) are unaffected.
- **Risk**: The synthesis may become longer, potentially hitting the 32KB field limit for judge sessions. Monitor output size.
