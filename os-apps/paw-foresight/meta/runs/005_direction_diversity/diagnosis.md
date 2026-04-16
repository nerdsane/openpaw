# Run 005 Diagnosis

## Summary

**Engine: 28.3/48 | Baseline: 26.3/48 | Delta: +2.0 raw**
**Engine Borda: 56.0/72 | Baseline Borda: 52.0/72 | Delta: +4.0**
**Winner: Engine** (third consecutive engine win)

The direction diversity constraint was added to the synthesis template but **was not followed
by the orchestrator**. All 12 directions were again included verbatim in the output (10 of
12 governance-themed). Despite this, the engine won by a wider Borda margin than Run 004
(+4.0 vs +3.0) because Specificity and Falsifiability improved substantially. Breadth
remains the engine's largest deficit at -3.0, unchanged from Run 004.

## What Improved (vs Run 004)

- **Specificity: E=6.0 B=3.0** (was E=4.5 B=4.5). All 3 judges scored engine 3-4 vs
  baseline 2-3. The engine now names specific tools, thresholds, and dates in nearly every
  claim: "20-30% cycle-time reduction on well-tested services by July 2026", "at least 2
  model vendors plus 1 open wrapper in production routing by Q4 2026", "escaped-defect rate
  must stay under 1 per 100 agent-authored merges." This was not a planned improvement —
  it emerged from the natural run variation.

- **Falsifiability: E=6.0 B=3.0** (was E=5.0 B=4.0). All 3 judges scored engine 3-4.
  The predictions section includes explicit falsification conditions with dates and
  mechanisms: "If fewer than 30% of enterprise agent rollouts still rely on PR plus CI
  gating... by 2027-01-31." Judges scored baseline only 2 on falsifiability because its
  predictions are more hedged.

- **Plausibility: E=4.5 B=4.5** (was E=4.0 B=5.0). Recovered from baseline win to tie.
  The engine's citation style no longer looks purely mechanical.

- **Quantitative Precision: E=5.5 B=3.5** (maintained from Run 004). Engine cites
  specific numbers (5-15%, 20-30%, $250K, sub-5-minute, per-100-merge rates). Baseline
  uses fewer specific numbers.

## What Failed to Improve

- **Breadth: E=3.0 B=6.0** (unchanged from Run 004). The direction diversity constraint
  was NOT followed. All 3 judges scored baseline 3 vs engine 2. The engine still has 10+
  directions that all converge on governance themes. The Key Findings span 6 themes per
  the diversity mandate, but the massive directions section overwhelms the structural
  diversity.

- **Actionability: E=3.5 B=5.5** (unchanged from Run 004). J2 scored baseline 3 vs
  engine 2. Baseline's decision points connect more clearly to organizational milestones.
  Engine's decision points are well-structured but still read as strategic rather than
  operational.

## Root Cause: Why the Direction Diversity Constraint Failed

The synthesis template Step C was changed from "dump all directions" to "select 5 spanning
4+ themes." But the orchestrator performed synthesis directly within its own context (as
in Run 004) rather than delegating to a synthesis session. When doing synthesis in-context:

1. The orchestrator reads the SYNTHESIS_TEMPLATE as part of its initial user_message
2. It interprets the template instructions as advisory guidance, not as hard constraints
3. When it reaches Step C, it has all 12 directions loaded and defaults to including all
   of them — the path of least resistance
4. The "DO NOT dump all directions" instruction is treated the same as any other prose
   suggestion — sometimes followed, sometimes not

This is the **same failure mode as Run 001** where prose mandates in the synthesis template
were not enforced. The difference: in Run 001 the issue was the orchestrator not reading
the template at all (it wasn't in TemperFS). In Run 005 the template IS read but the
constraint is advisory.

**The fundamental problem:** prose instructions cannot enforce structural constraints on
output. The orchestrator has no mechanism to verify compliance before writing. It reads
"select at most 5" and then proceeds to include all 12 because it has them in context
and including more feels more complete.

## Structural Observations

1. **54 observations** (up from 46 in Run 004, 75 in Run 003). Quality appears good —
   broader tool coverage (LiteLLM, Helm, Atlantis, Sentinel mentioned for the first time).

2. **12 directions** (same as Runs 003-004). The convergence step produces 2 directions
   per probe per step (6 probes × 2 steps = 12). This is stable.

3. **21 orchestrator turns** (up from 13 in Run 004). More turns suggest the orchestrator
   did more work, possibly including the analysis handoff steps.

4. **No synthesis delegation**. Despite explicit "DO NOT synthesize in-context" instructions,
   the orchestrator completed synthesis directly. The delegation path was coded but not
   exercised — same pattern as Run 004.

5. **Engine output: 44.6KB** (similar to Run 004's 44.8KB). Output size is stable.

## Why the Engine Still Wins

Despite unchanged Breadth and Actionability, the engine wins because:
- Specificity improvement (+3.0 Borda) offsets Breadth deficit (-3.0)
- Falsifiability improvement (+3.0 Borda) offsets Actionability deficit (-2.0)
- Progression (+1.0) and Quant Precision (+2.0) provide additional margin
- 7 criteria are tied; engine leads on 3, trails on 2

The engine's structural advantages (dated falsification criteria, specific quantitative
thresholds, obs-cited evidence chains) are now well-established. Its weakness is content
diversity in the directions section.

## Recommended Changes for Run 006

**Priority 1: Enforce direction selection in WASM, not in prose.**

The synthesis template cannot enforce direction selection because the orchestrator treats
it as advisory. Instead, move the constraint into the WASM layer or the convergence step:

Option A: **WASM-enforced direction limit.** After the convergence step, add WASM code that
reads all Direction entities, classifies them by a `theme` field, and ARCHIVES directions
that exceed a per-theme quota. The synthesis step would then see only 5 directions in the
API response, making compliance automatic.

Option B: **Change the convergence step to produce fewer, more diverse directions.** Instead
of 2 directions per probe per step (12 total), instruct the orchestrator to produce at most
1 direction per THEME during convergence, with a maximum of 5 total. This is still a prose
instruction, but it's applied DURING generation (when the orchestrator is actively deciding
what to create) rather than AFTER generation (when all 12 already exist).

Option C: **Add a `theme` field to the Direction entity spec** and modify the synthesis
template to group by `dir_data[did]["theme"]` and select the top 1 per theme. This is
structural — the template's Step C code would loop over themes, not over all directions.

**Recommended: Option B**, because it's one change (to the orchestration instructions)
and addresses the root cause: too many directions are created in the first place. Options
A and C require multiple file changes.

**Priority 2: Fix Actionability.** The decision points are well-structured but strategic.
The baseline's decision points include operational triggers ("when more than 20-30% of
engineering tasks involve coding agents"). Add concrete operational examples to the
decision point template.

Per meta-loop rules: make ONE targeted change per iteration.

## Convergence Status

Engine Borda 56.0 vs Baseline 52.0. Engine wins three consecutive runs (003, 004, 005).
The engine's overall score is stable (Borda 55.5-56.0 across 3 wins). Breadth is the
one persistent deficit preventing further gains.

A-wins streak: 0 (engine keeps winning as the modified challenger).
Convergence: not yet.
