# Run 001 Diagnosis

## Summary
**Engine: 27.0/48 | Baseline: 24.0/48 | Delta: +3.0**
**Borda: Engine 58.5/72, Baseline 49.5/72 | Winner: Engine**

The fuel budget increase (50B → 500B WASM instructions) fixed the orchestrator crash from Run 000. The engine completed the full 2-step projection loop: 3 probes x 2 steps, convergence, and a 34KB synthesis with all 9 required sections. The engine wins on 4 criteria (Specificity +3, Falsifiability +3, Actionability +2, Progression +1) and ties on 8. No criteria favor the baseline.

## Where the Engine Wins

### Specificity (Engine: 3.0/4, Baseline: 2.0/4) — Borda: 6 vs 3 (+3)
- The engine output includes quantitative thresholds embedded in observation data: "70% harness coverage," "10% manual override," "25% automation of repetitive operations," "3-5 specialized agents." These thresholds come from the multi-probe architecture producing more specific, testable claims.
- The baseline names actors and dates but with less quantitative precision and fewer falsifiable thresholds.

### Falsifiability (Engine: 3.0/4, Baseline: 2.0/4) — Borda: 6 vs 3 (+3)
- The engine has a dedicated "Top 5 Predictions with Falsification Criteria" section where every prediction names a specific date, measurable indicator, and explicit falsification condition: "If this has not occurred by 2027-03-31, this prediction is wrong because..."
- The baseline has confidence levels and "what would change it" statements but no explicit falsification conditions with dates.

### Actionability (Engine: 2.7/4, Baseline: 2.0/4) — Borda: 5.5 vs 3.5 (+2)
- The engine has 3 structured decision points, each with timing triggers, 3 named options (specific tools/configs like "Deploy Cedar policy gates"), effort estimates ("2-4 engineering-weeks"), and recommendations.
- The baseline has decision points but with more generic tradeoffs and less specific effort estimates.

### Progression (Engine: 2.3/4, Baseline: 2.0/4) — Borda: 5 vs 4 (+1)
- The engine has 4 temporal phases (0-3, 3-6, 6-9, 9-12 months) with "Revisions to earlier predictions" subsections in phases 2-4. This demonstrates genuine temporal development where later phases revise earlier ones.
- The baseline has 3 phases with causal links and "what has NOT changed" sections but no explicit revision of earlier predictions.
- One judge scored this 3 vs 2 (engine wins); two scored it 2 vs 2 (tie). The revision mechanism is there but its impact is modest.

## Where Both Score 2 (Tied)

8 criteria tied at 2.0 vs 2.0 across all judges. This is the "competent median" calibration — both outputs are solid but neither reaches "genuinely impressive" (3) on these dimensions:

### Novelty (tied 2/2)
- Both produce extensions of the input knowledge graph without introducing truly external evidence or frameworks. The engine's cross-domain analogies (biology, economics, industrial control) come from the adjacent-domain probe but are somewhat predictable.
- **Fix target for next iteration.** The probes need to bring in evidence from OUTSIDE the knowledge graph — real external signals, data, or cross-domain research not in the input.

### Breadth (tied 2/2)
- The engine covers more themes (8 key findings across 7 distinct themes) but the judges rated it as comparable to the baseline's 5 active directions. The cross-theme connections exist but don't produce non-obvious conclusions.
- **Fix target.** Convergence analysis should explicitly identify where theme interactions produce surprising conclusions, not just confirm agreements.

### Decision Clarity (tied 2/2)
- Neither output opens with the single most important decision. Both structure findings and decision points, but the "so what" requires the reader to synthesize.
- **Fix target.** The synthesis should lead with a 1-paragraph executive recommendation before the executive summary.

### Grounding (tied 2/2)
- The engine cites observation IDs but the reasoning chains from evidence → mechanism → conclusion have gaps. Claims are supported but the "why does this evidence lead to this conclusion?" is implicit.
- **Fix target.** Probe prompts should emphasize explicit reasoning chains, not just observations.

### Challenge (tied 2/2)
- Both identify tensions in the source material but neither overturns a source assumption using external evidence. The engine's "What Surprised Us" section identifies 4 challenges but they're within the input's framing.
- **Fix target.** The critic probe needs to bring external evidence that contradicts the source, not just reframe tensions within it.

### Information Density (tied 2/2)
- Some redundancy exists in both. The engine's 8 key findings cover distinct themes (meeting the diversity mandate) but the active directions overlap with key findings. Convergence reduced redundancy from Run 000 but didn't eliminate it.

## Why the Engine Wins

The engine wins because its multi-agent architecture now completes the full pipeline and produces structural advantages in exactly the areas where the baseline is weakest: quantitative thresholds (Specificity), explicit falsification conditions (Falsifiability), structured decision frameworks with effort estimates (Actionability), and temporal revision (Progression). These are artifacts of the synthesis template design — the engine was told to produce falsification criteria, decision points with effort estimates, and phase revisions. The baseline was given a simpler prompt that didn't mandate these structures.

The win is real but narrow. The engine doesn't outperform on any "deep analysis" criteria — the 8 tied criteria are all at the competent baseline of 2. The multi-agent probes produce more specific observations, but the synthesis doesn't yet demonstrate deeper insight, stronger reasoning chains, or genuinely novel analysis that a single-shot prompt couldn't produce.

## Recommended Changes for Next Iteration

**Priority 1:** Improve probe prompts to bring EXTERNAL evidence. The biggest opportunity is Novelty and Challenge (both stuck at 2). The adjacent-domain probe already has a cross-domain mandate but produces analogies rather than external data. Change: instruct probes to use `temper_web_search` and `temper_web_fetch` to find real, recent signals (news, papers, announcements) not in the knowledge graph and cite them as evidence.

**Priority 2:** Improve the synthesis template to add an upfront executive recommendation (first paragraph = "The #1 thing to do, by when, at what cost") before the executive summary. This targets Decision Clarity.

Per meta-loop rules: make ONE targeted change per iteration. Priority 1 targets the most criteria (Novelty, Challenge, Grounding, Plausibility) and represents a structural change (probes use web search) rather than a prompt edit.
