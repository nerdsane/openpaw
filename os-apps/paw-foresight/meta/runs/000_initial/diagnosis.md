# Run 000 Diagnosis (Rubric v2)

## Summary

**Engine: 18/48 | Baseline: 27/48 | Delta: -9**

Re-scored under the revised rubric with tighter anchors and calibration (2 = competent, 3 = genuinely impressive, 4 = exceptional). The -9 gap is preserved from the original scoring, but absolute scores dropped ~16 points each, creating 30 points of engine headroom for hill climbing.

The engine ties on 3 criteria (Specificity, Novelty, Decision Clarity) and loses the remaining 9. No criterion where the engine wins.

## New Criteria Performance

### Falsifiability (Engine: 1, Baseline: 2) — NEW CRITERION

The engine's predictions are stated as assertions ("selection quality remained scarcer") or trend descriptions ("that pattern intensified"). None include conditions under which they'd be proven wrong.

**Root cause:** The synthesis template doesn't instruct the orchestrator to state falsification criteria. The probe outputs generate observations, but observations are descriptive ("X is happening") not predictive ("X will happen by [date], falsified if Y").

**Fix:** The orchestration skill's synthesis section should mandate that each major prediction includes: (a) a checkable condition, (b) a timeline, and (c) what would falsify it.

### Quantitative Precision (Engine: 0, Baseline: 1) — NEW CRITERION

The engine produces zero quantitative predictions. Every claim is qualitative ("scarcer," "outperformed," "concentrated"). Even the methodology note uses counts (27 observations) rather than quantitative substance.

**Root cause:** No part of the engine pipeline — probes, convergence, or synthesis — instructs agents to produce numerical estimates, adoption percentages, market thresholds, or measurable indicators. The probes observe patterns; they don't estimate magnitudes.

**Fix:** Probe prompts should explicitly instruct: "For each prediction, include at least one measurable indicator (adoption %, market size, threshold, or proxy metric)." The synthesis template should require a quantitative dimension for each active direction.

### Decision Clarity (Engine: 2, Baseline: 2) — RENAMED/TIGHTENED

Neither output opens with THE single most important decision, names a deadline, or quantifies the tradeoff. Both require the reader to extract priorities from a well-organized but unprioritized narrative.

**Root cause (engine):** The synthesis template produces Decision Points as equal-weight bullets. No instruction to rank them or lead with the #1 recommendation.

**Fix:** The synthesis template should instruct: "Open the synthesis with the single most important decision. Name who must decide, by when, and what it costs to wait."

## Unchanged Criteria — Updated Scores

### Transparency (Engine: 1, Baseline: 2) — STILL LOWEST ENGINE SCORE

Same root cause as original diagnosis: synthesis doesn't surface observation IDs or signal references. No change since last analysis.

### Actionability (Engine: 1, Baseline: 2)

Engine decision points are still unprioritized bullets without triggers, options, or tradeoffs. The baseline has all three but lacks concrete cost quantification. Under the tightened rubric, the baseline drops from 4 to 2 because generic tradeoffs no longer qualify for 3+.

### Progression (Engine: 1, Baseline: 2)

Engine still limited to 2 time points. The baseline's 3-phase structure earns a 2 but no longer a 4 because later phases don't revise earlier predictions — they only extend them.

### Completeness (Engine: 2, Baseline: 3)

The baseline's explicit confidence levels, counterfactuals, and challenged assumptions earn a genuine 3 under the strict calibration. The engine still lacks all three.

## Why the Baseline Still Wins

The same three structural advantages from the original diagnosis:
1. **Full context window** — single-shot model produces one coherent response without fragmentation
2. **Prompt specificity** — the baseline prompt explicitly requested confidence levels, decision frameworks, challenged assumptions
3. **No coordination overhead** — every token goes to substance

The rubric tightening reveals that even the baseline has substantial room to improve — it scored 27/48, not 43/48. The criteria where BOTH outputs scored low (Specificity: 2/2, Decision Clarity: 2/2, Quantitative Precision: 0/1) represent shared weaknesses that the engine could address to differentiate itself.

## Recommended Changes for Next Iteration

**Priority 1 (addresses 4 criteria where engine scored 0-1):**
Update synthesis template to mandate:
- Falsification criteria for each major prediction
- Quantitative indicators (thresholds, percentages, measurable proxies)
- Signal/observation references traced to claims (transparency)
- Decision points with triggers, options, and tradeoffs

**Priority 2 (addresses Progression):**
- Increase time steps to 4 (quarterly) instead of 2
- Synthesis template should require phase-by-phase breakdown where later phases reference and potentially revise earlier predictions

**Priority 3:** Fix automated judge infrastructure

Per meta-loop rules: make ONE targeted change per iteration. Priority 1 (synthesis template update) addresses the criteria where the engine scores 0 or 1: Quantitative Precision (0), Falsifiability (1), Transparency (1), Actionability (1), Progression (1).
