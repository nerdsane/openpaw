# Run 004 Plan

## Target Criteria

Run 003 won (Engine Borda 56, Baseline Borda 52) but the engine still loses on 4 criteria:

- **Progression** (E=4.0, B=5.0): J3 scored baseline 3 vs engine 2. Root cause: malformed
  citation IDs and formulaic revision sections caused by synthesis running in a crash-recovery
  session without full orchestrator reasoning context.
- **Actionability** (E=4.0, B=5.0): J3 scored baseline 3 vs engine 2. Decision points still
  read as strategic recommendations, not operational playbooks.
- **Challenge** (E=4.0, B=5.0): J1 scored baseline 3 vs engine 2. Engine's "What Surprised Us"
  lists observation-level surprises; baseline's counterfactuals challenge the source thesis
  with mechanism-level reasoning.
- **Breadth** (E=4.0, B=5.0): J1/J2 scored baseline 3 vs engine 2. Engine themes converge
  on one dominant governance thesis; baseline covers more independently connected themes.

All 4 losses are single-judge 3>2 splits. The engine is close on every one.

## Root Cause

**The orchestrator crashes at turn 16 (68KB context overflow in llm_caller WASM).** This is
the #1 priority from the Run 003 diagnosis. The crash means:

1. Synthesis happens in a crash-recovery session without the orchestrator's accumulated
   reasoning, convergence analysis, and probe-level insights
2. The synthesis session loads data from the API but has no analytical context
3. This produces template-compliant but formulaic output — citations without reasoning,
   revisions without genuine temporal development
4. Malformed citation IDs (e.g., "en-019d94af-019d94af?") from the crash context

## Planned Change: Dedicated Synthesis Delegation

Modify ORCHESTRATION_INSTRUCTIONS in `wasm/spawn_orchestrator/src/lib.rs` to:

1. **After probes and convergence complete**: have the orchestrator write a structured
   "analysis handoff" file to the workspace containing:
   - Convergence findings and cross-probe agreements/disagreements
   - Probe-level insights that challenged the source material
   - Key tensions and surprises discovered during convergence
   - Data statistics (observation/direction counts, theme distribution)

2. **Spawn a dedicated synthesis session**: the orchestrator creates a new Agent + Session,
   configures it with:
   - The synthesis template (Steps A-G, quality rules, diversity rules)
   - Instructions to read the analysis handoff file
   - Instructions to load observations and directions from the API
   - The projection ID and model context

3. **Poll and complete**: the orchestrator waits for the synthesis session to finish,
   then dispatches the Complete action on the Projection.

This is ONE architectural change: synthesis delegation. It replaces the current design
where the orchestrator tries to do everything in one session (which crashes at 68KB).

## Expected Impact

- **Progression (+1 Borda)**: Synthesis session gets clean context + orchestrator's analytical
  reasoning via the handoff file. No malformed citations. More organic temporal development.
- **Challenge (+1 Borda)**: The handoff file includes probe-level challenges to the source
  thesis, giving the synthesis session material for stronger counterfactual development.
- **Breadth (maintain or +1)**: Clean context allows better organization of diverse themes.
- **Actionability (maintain)**: Not directly addressed — would need template changes.

Conservative target: Engine Borda 58-60 vs Baseline 52 (delta +6 to +8).
