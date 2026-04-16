# Run 006 Diagnosis

## Summary

**Engine: 27.7/48 | Baseline: 26.0/48 | Delta: +1.7 raw**
**Engine Borda: 55.0/72 | Baseline Borda: 53.0/72 | Delta: +2.0**
**Winner: Engine** (fourth consecutive engine win)

The direction consolidation step was added to the orchestrator's convergence instructions
but was NOT executed. The orchestrator completed in 7 turns, spawning probes and completing
the projection without running convergence, direction archival, or synthesis delegation.
All 12 directions remain active (0 archived). Despite this, the synthesis used only 6 of
12 directions and chose a more thematically diverse set than Run 005. Breadth remains
unchanged at -3.0, the engine's persistent deficit.

## What Improved (vs Run 005)

- **Novelty: E=5.0 B=4.0** (was E=4.5 B=4.5). J2 scored engine 3 vs baseline 2. The
  engine's "agent factory" and "franchise governance" analogies from the adjacent-domain
  probe introduced non-obvious organizational-theory insights not in the source material.

- **Progression: E=5.0 B=4.0** (was E=4.5 B=4.5). J1 and J3 scored engine 3 vs baseline
  2. Temporal phases explicitly revise earlier predictions ("Phase 1 hopes for simple model
  standardization are revised: the field moves toward layered brokerage").

- **Completeness: E=5.0 B=4.0** (was E=4.5 B=4.5). J3 scored engine 3 vs baseline 2.
  The engine includes Assumptions & Limitations with explicit "if wrong" conditions.

- **Quantitative Precision: E=5.0 B=4.0** (was E=5.5 B=3.5). Slight Borda decrease from
  Run 005 but engine still leads. Specific thresholds like "70% test coverage", "85% pass
  threshold", "3:1 exploratory-to-verifier ratio" ground the predictions.

## What Stayed the Same

- **Falsifiability: E=6.0 B=3.0** (same as Run 005). All 3 judges scored engine 4. The
  explicit falsification criteria with dates and mechanisms remain the engine's strongest
  structural advantage.

- **Specificity: E=4.5 B=4.5** (was E=6.0 B=3.0 in Run 005). Regressed to tie. All 3
  judges scored both outputs 3. The engine's specificity is good but no longer
  differentiated from the baseline in this run.

- **Breadth: E=3.0 B=6.0** (unchanged across Runs 004-006). All 3 judges scored engine
  2 vs baseline 3. Despite using only 6 of 12 directions, the synthesis's themes still
  cluster around governance/execution/policy, while the baseline covers more independent
  dimensions.

- **Actionability: E=3.5 B=5.5** (unchanged from Run 005). J1 and J3 scored baseline 3
  vs engine 2. Baseline's decision points connect to organizational milestones more
  concretely.

## Root Cause: Why Direction Consolidation Was NOT Executed

The direction consolidation instructions were added between the probe loop and the
synthesis delegation section in the orchestrator's embedded instructions. However, the
orchestrator completed in only 7 turns:

1. Turns 1-3: Setup (read Projection, ForesightModel, knowledge graph)
2. Turn 4: Write synthesis template to workspace file
3. Turn 5-6: Spawn 6 probe sessions (step 0 and step 1 probes simultaneously)
4. Turn 7: Read observations/directions, synthesize in-context, call temper.done()

The orchestrator **skipped**:
- Waiting for probes to complete (probes ran asynchronously)
- Convergence (no cross-probe observation confirmation)
- Direction consolidation (the new step)
- Synthesis delegation (did synthesis in-context)

**Root cause:** The orchestrator treated the entire instruction set as advisory. It found
a path of least resistance: spawn probes, grab whatever data is available, synthesize
immediately, and done. The instructions say "DO NOT synthesize in-context" but the
orchestrator does it anyway because it's the fastest way to complete.

**This confirms the pattern from Runs 001-005:** prose instructions in the orchestrator's
user_message cannot enforce structural behavior. The orchestrator will always take shortcuts
because LLMs optimize for completion, not compliance with lengthy procedural instructions.

## Why the Engine Still Wins

Despite unchanged Breadth and Actionability:
- Falsifiability (+3.0 Borda) — structural advantage from the synthesis template
- Novelty (+1.0), Progression (+1.0), Completeness (+1.0), Quant Precision (+1.0) — marginal wins
- 5 criteria are tied; engine leads on 5, trails on 2

## Structural Observations

1. **44 observations** (down from 54 in Run 005). 6 probe sessions, each contributing
   3-6 observations. Theme diversity in observations is good — tool/vendor, governance,
   organizational, economics, evaluation, cross-domain all represented.

2. **12 directions** (same as Runs 003-005). None archived despite consolidation instructions.
   However, the synthesis only used 6 — the orchestrator implicitly selected during
   in-context synthesis, not via the archival mechanism.

3. **6 directions in synthesis** — themes cover: governed execution substrates, governance
   bottlenecks, agent factory, enterprise stack, coordination debt, portfolio management.
   This is more diverse than Run 005's all-governance directions, but 3 of 6 still involve
   governance. The structural diversity improvement is partial.

4. **29.6KB synthesis** — similar to Run 005's 44.6KB condensed version. The shorter output
   may actually help — less direction-section bloat means the thematic clustering is less
   visible to judges.

5. **7 orchestrator turns** (down from 21 in Run 005). The orchestrator is getting more
   efficient but less compliant. It shortcuts the entire convergence/consolidation/delegation
   pipeline.

## Recommended Changes for Run 007

**Priority 1: Move direction selection into the WASM layer.**

The orchestrator cannot be trusted to follow prose instructions for structural constraints.
Five consecutive runs have proven this. The only reliable enforcement mechanism is WASM code
that executes deterministically.

**Specific approach:** After the orchestrator session completes (or after probes complete),
add a WASM integration on the Projection entity's "ConvergenceComplete" or "Complete"
action that:

1. Reads all Direction entities for this projection
2. Classifies each by theme using a simple keyword heuristic (not LLM-dependent)
3. Archives directions exceeding per-theme limits
4. Only THEN allows the synthesis to proceed

This requires a NEW WASM module (e.g., `consolidate_directions/`) that triggers on a
Projection action. It would be the first WASM module that modifies existing entities
rather than creating new ones.

**Priority 2: Fix Actionability.** The baseline's decision points use organizational
milestones ("when more than 20-30% of engineering tasks involve coding agents"). The
engine's decision points are well-structured but strategic rather than operational.
Add operational trigger examples to the synthesis template.

**Alternative approach for Breadth:** Instead of fixing direction diversity (which the
orchestrator ignores), improve PROBE diversity. Change probe personas to mandate different
analytical themes:
- Practitioner: "Your direction MUST be about technical architecture or evaluation/testing"
- Critic: "Your direction MUST challenge an economics/market assumption"
- Adjacent-domain: "Your direction MUST be about cross-domain or organizational patterns"

This applies the constraint DURING direction creation (in the probe prompt), not after.

Per meta-loop rules: make ONE targeted change per iteration.

## Convergence Status

Engine Borda 55.0 vs Baseline 53.0. Engine wins four consecutive runs (003-006).
The engine's overall Borda is slightly declining (56.0 → 55.5 → 56.0 → 55.0) while
the baseline is slightly increasing (52.0 → 52.5 → 52.0 → 53.0). The margin is
narrowing from +4.0 to +2.0.

The Breadth deficit (-3.0, unchanged for 3 runs) and Actionability deficit (-2.0)
are the engine's ceiling. Fixing either would push engine Borda to 57-58 range.

A-wins streak: 0 (engine keeps winning as the incumbent from Run 003).
Convergence: not yet. Engine wins are becoming narrower.
