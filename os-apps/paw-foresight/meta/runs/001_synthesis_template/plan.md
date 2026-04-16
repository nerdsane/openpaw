# Run 001 Plan

## Target Criteria

The 5 criteria where the engine scores lowest relative to baseline are all rooted in the same cause: the synthesis template in `SKILL.md` lacks specific output instructions.

| Criterion | Engine | Baseline | Gap | Root Cause |
|-----------|--------|----------|-----|------------|
| Transparency | 1.0 | 2.0 | -1.0 | Synthesis never cites observation IDs or signal references |
| Quant Precision | 1.0 | 1.7 | -0.7 | No instruction to produce numerical estimates or thresholds |
| Specificity | 2.0 | 3.0 | -1.0 | No instruction to name companies, tools, or dates |
| Actionability | 2.0 | 2.7 | -0.7 | Decision points are flat bullets, not trigger→options→tradeoffs |
| Falsifiability | 2.0 | 2.3 | -0.3 | No instruction to state falsification criteria |
| Progression | 2.0 | 2.3 | -0.3 | Only 2 time steps; no instruction to revise earlier predictions |

## Planned Change

**ONE change: Rewrite the Final Synthesis section of `os-apps/paw-foresight/system/skills/orchestrate-projection/SKILL.md`.**

The current synthesis template is a bare markdown skeleton with no quality mandates. The new template will:

1. **Transparency**: Mandate inline observation references `[obs: ID]` and signal citations for every substantive claim
2. **Quantitative Precision**: Require at least one measurable indicator (%, threshold, timeline, proxy metric) per prediction
3. **Specificity**: Require named actors (companies, tools, projects) and specific dates rather than generic categories
4. **Falsifiability**: Require each major prediction to include a falsification condition ("If X has not happened by Y, this is wrong because Z")
5. **Actionability**: Structure Decision Points as: timing trigger → 2-3 options → concrete tradeoffs per option
6. **Progression**: Add instruction for 4 quarterly phases where later phases explicitly revise/qualify earlier predictions
7. **Completeness**: Add Assumptions & Limitations section with confidence levels and what-would-change-my-mind

No changes to probe prompts, entity specs, WASM, or architecture. This is purely a synthesis-template quality upgrade.

## Expected Impact

| Criterion | Current | Expected | Why |
|-----------|---------|----------|-----|
| Transparency | 1.0 | 2.0+ | Inline observation refs give provenance |
| Quant Precision | 1.0 | 2.0+ | Explicit quantitative mandate |
| Specificity | 2.0 | 2.5+ | Named-actor mandate, same KG has the entities |
| Falsifiability | 2.0 | 2.5+ | Explicit falsification criteria mandate |
| Actionability | 2.0 | 2.5+ | Structured decision framework |
| Progression | 2.0 | 2.0+ | Revision-of-earlier-predictions instruction |

Conservative target: Engine 28+/48, narrowing the gap from -3.3 to within -1.
