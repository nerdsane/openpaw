# Run 000 Diagnosis (Rubric v3 — 3 Independent Judges)

## Summary

**Engine: 23.7/48 | Baseline: 27.0/48 | Delta: -3.3 | Borda: 49.0 vs 59.0/72**

Scored by 3 independent paw-agent judges (gpt-5.4) under rubric v3 (tightened 3-level anchors on Novelty/Breadth/Progression/Challenge + 3+ cap rule limiting each output to max 3 criteria at 3+). Split-session approach: each judge scored each output independently to stay under the 32KB WASM field limit.

The engine ties baseline on 5 criteria (Novelty, Breadth, Plausibility, Completeness — all 2s; Challenge — mixed). The engine loses on Specificity, Falsifiability, Actionability, Transparency, Quantitative Precision. The engine wins only on Decision Clarity and Challenge (barely, via individual judge variation).

## Per-Criterion Analysis

### Transparency (Engine: 1.0, Baseline: 2.0) — WORST ENGINE CRITERION

All 3 judges gave the engine a 1. The engine's synthesis never cites specific observation IDs, signal references, or knowledge graph nodes. Claims like "selection quality remained scarcer" are asserted without tracing to their source. The baseline references "the graph's signals" and "the knowledge graph" more explicitly.

**Root cause:** The synthesis template and orchestration skill do not instruct the synthesizer to reference specific observations or signals by ID. The probe outputs generate observations, but observation IDs are never carried through to the final narrative.

**Fix:** The synthesis section of the orchestration skill should mandate: "For each major claim, cite the observation ID(s) or signal(s) that support it. Use inline references like [obs-N] or [signal: name]."

### Quantitative Precision (Engine: 1.0, Baseline: 1.7) — SECOND WORST

Engine produces zero quantitative predictions. Every claim is qualitative. The baseline slightly edges it because judges 2 and 3 noted the baseline at least uses "20-30%" thresholds and adoption percentage triggers in decision points.

**Root cause:** No part of the engine pipeline instructs agents to produce numerical estimates. Probes observe patterns but don't estimate magnitudes.

**Fix:** Probe prompts should include: "For each prediction, include at least one measurable indicator (adoption %, market size threshold, timeline in months, or proxy metric)." Synthesis template should require a quantitative dimension for each active direction.

### Specificity (Engine: 2.0, Baseline: 3.0) — 1-POINT GAP

Unanimous: all 3 judges gave engine 2, baseline 3. The engine uses mechanisms ("incident-to-eval loops," "policy gates") but lacks specific actor names and dates. The baseline names Anthropic, OpenAI, Cursor, Cognition, Devin, Aider, Cline, Continue, OpenHands.

**Root cause:** The engine's probes and synthesis don't instruct naming real companies, tools, or dates. The knowledge graph contains these entities, but the orchestration skill doesn't push agents to ground claims in named actors.

**Fix:** Probe prompts should include: "Name specific companies, tools, projects, and approximate dates. Do not use generic categories when specific actors can be named."

### Falsifiability (Engine: 2.0, Baseline: 2.3) — MODERATE GAP

Engine predictions are stated as trends ("that pattern intensified") not as falsifiable claims. The baseline has slightly better structure with its confidence levels section, but neither output achieves strong falsifiability.

**Root cause:** The synthesis template doesn't instruct the orchestrator to state falsification criteria.

**Fix:** Synthesis template should mandate: each major prediction includes (a) a checkable condition, (b) a timeline, (c) what would falsify it.

### Actionability (Engine: 2.0, Baseline: 2.7) — MODERATE GAP

Engine decision points are equal-weight bullets without timing triggers, options, or tradeoffs. The baseline structures 4 decision points with timing triggers, explicit options (A/B/C), and tradeoff descriptions.

**Root cause:** Synthesis template produces Decision Points as a flat bullet list. No instruction to structure as trigger → options → tradeoffs.

**Fix:** Synthesis template should instruct: "For each decision point, provide: (1) timing trigger — observable event that makes this decision urgent, (2) 2-3 options, (3) tradeoff for each option in concrete terms."

### Decision Clarity (Engine: 2.3, Baseline: 2.0) — SLIGHT ENGINE WIN

One of two criteria where the engine edges the baseline. Judge 1 gave engine a 3, noting the executive summary leads with a clear framing. But this is fragile — judges 2 and 3 both gave 2.

### Challenge (Engine: 2.3, Baseline: 2.0) — SLIGHT ENGINE WIN

The engine's critic probe produces genuine counter-narratives ("proxy-driven homeostasis dressed up as evolution"). Under the tightened anchors, this still earns 2-3 from judges. The baseline's "Assumptions to Challenge" section is more structured but less pointed.

### Tied Criteria

- **Novelty (2.0 / 2.0):** Under tightened anchors requiring external evidence for a 3, both drop to 2. Neither introduces insights truly from outside the input material.
- **Breadth (3.0 / 3.0):** Both cover 6+ themes with cross-theme interactions. This is the one criterion where both consistently earn 3.
- **Plausibility (2.0 / 2.0):** Both reference evidence and mechanisms but lack explicit confidence levels meeting the 3-standard.
- **Progression (2.0 / 2.3):** Engine's 2-step progression (90d, 365d) is too coarse for later phases to revise earlier predictions. Baseline's 3-phase structure earns one judge's 3 because Phase 3 builds on Phases 1-2.
- **Completeness (2.0 / 2.0):** Neither includes explicit what-would-change-my-mind for major claims.

## Why the Baseline Still Wins

The same three structural advantages persist:
1. **Full context window** — single-shot model uses all tokens for substance, no coordination overhead
2. **Prompt specificity** — the baseline prompt explicitly requested confidence levels, decision frameworks, challenged assumptions, counterfactuals
3. **Named actors** — the baseline prompt asked for specific entity names, which the engine prompts don't

## Recommended Changes for Next Iteration

**Priority 1 (addresses the 5 criteria where engine scores 1.0-2.0 and baseline scores higher):**

Update the synthesis template in the orchestration skill to mandate:
- Observation/signal references traced to each major claim (Transparency)
- Quantitative indicators for each prediction (Quant Precision)
- Named actors, tools, and dates (Specificity)
- Falsification criteria for each major prediction (Falsifiability)
- Decision points with trigger → options → tradeoffs structure (Actionability)

**Priority 2 (addresses Progression):**
- Increase time steps to 4 (quarterly) instead of 2
- Synthesis template should require later phases to explicitly revise or challenge earlier predictions

Per meta-loop rules: make ONE targeted change per iteration. Priority 1 (synthesis template update) addresses the criteria where the engine scores lowest relative to baseline.
