# Run 002 Diagnosis

## Summary
**Engine: 27.3/48 | Baseline: 24.0/48 | Delta: +3.3**
**Borda: Engine 58.5/72, Baseline 49.5/72 | Winner: Engine**

Adding web search tools (`temper_web_search`, `temper_web_fetch`) to probes successfully lifted Novelty from tied-at-2 (Run 001) to engine-winning (Borda +2). The probes found real external evidence: GitHub Copilot coding agent launch, VS Code MCP spec support, Harness MCP server, OpenAI Codex CLI, OWASP LLM01:2025, DORA 2025 AI report, NIST AI Agent Standards Initiative, Anthropic Economic Index, Stack Overflow 2024 survey — 11 distinct external sources across 13 observations. Specificity and Falsifiability remain strong wins (Borda +3 each). However, 8 criteria remain tied at 2/2, and Actionability regressed from +2 (Run 001) to tied. The overall Borda is identical to Run 001 (58.5 vs 49.5).

## Intervention Assessment

**Target criteria from Run 001 diagnosis:**
| Criterion | Target | Result | Verdict |
|-----------|--------|--------|---------|
| Novelty | Break 2/2 tie | Engine 3 vs Baseline 2 (Borda +2) | **Hit** |
| Challenge | Break 2/2 tie | Tied 2/2 (Borda 0) | **Miss** |
| Grounding | Break 2/2 tie | Tied 2/2 (Borda 0) | **Miss** |
| Plausibility | Break 2/2 tie | Tied 2/2 (Borda 0) | **Miss** |

**Novelty improved** because probes brought genuinely external evidence (OWASP, DORA, NIST) and connected signals the input didn't connect (MCP adoption speed + auth spec instability → protocol outrunning trust). All 3 judges noted the engine's external grounding as the differentiator; 2 of 3 scored engine Novelty at 3.

**Challenge stayed flat** because the external evidence added nuance to the source thesis rather than contradicting it. The "harness as attack surface" insight (from OWASP) *complicates* the harness-first thesis but doesn't *overturn* it. Judges explicitly noted: "identifies tensions rather than makes specific contradictory predictions" (J1), "adds a caveat... closer to anchor 2 than anchor 3" (J3).

**Grounding stayed flat** because having external citations is necessary but not sufficient. The reasoning chains from evidence → mechanism → conclusion still have gaps. Judge 2: "DORA's amplification finding translates to coordination-cost-overshoot mechanism isn't fully spelled out." Judge 3: "500+ threshold is not grounded in any cited growth rate or data point."

**Plausibility stayed flat** because the confidence percentages (75%, 70%, etc.) appear arbitrary — no judge saw evidence that the numbers were derived rather than asserted. External sources strengthen individual claims but the systematic stated-assumptions coverage needed for score 3 is absent.

## Regression: Actionability (Run 001: +2 → Run 002: tied)

This is the key regression. In Run 001, the engine won Actionability because the synthesis template produced structured decision points with effort estimates. In Run 002, the same template is in use, and the synthesis does contain decision points with effort estimates. But 2 of 3 judges scored it 2/2 (Run 001 had 2 of 3 at engine 2, baseline 2, with one at engine 3, baseline 2). The Borda shifted from 5.5/3.5 to 4.5/4.5.

Root cause: the longer synthesis (42KB vs 34KB in Run 001) buries the decision points deeper in the document. The additional external evidence produces more findings and more temporal analysis, which pushes actionable content later. The "signal-to-decision" ratio degraded.

## Where the Engine Wins

### Specificity (Engine: 3.0/4, Baseline: 2.0/4) — Borda: 6 vs 3 (+3)
All 3 judges unanimously scored Specificity 3 for engine vs 2 for baseline. The engine names real actors with specific dates (VS Code June 12 2025, Harness May 29 2025, GitHub Copilot by July 2026) and quantitative thresholds (60% PR acceptance, 500+ MCP servers, 75% confidence). The baseline uses phase ranges and qualitative confidence. This win is structural and durable.

### Novelty (Engine: 2.7/4, Baseline: 2.0/4) — Borda: 5.5 vs 3.5 (+2)
**New win from this iteration.** Two judges scored engine 3, one scored 2. The engine's novel insights cited by judges: (1) harness-as-attack-surface from OWASP, (2) autonomy-reduces-throughput from DORA, (3) protocol-outrunning-trust connecting MCP adoption to auth maturity. Judge 3 was the outlier, rating both at 2: "insights are applications of external data to input themes rather than fundamentally new frameworks."

### Falsifiability (Engine: 3.3/4, Baseline: 2.0/4) — Borda: 6 vs 3 (+3)
Strong win. Judge 3 gave the engine a 4 — the only score above 3 in the entire judging panel: "textbook match to the score-4 anchor" for the dedicated falsification section with dates, measurable indicators, and explicit "this prediction is wrong because..." statements.

### Progression (Engine: 2.3/4, Baseline: 2.0/4) — Borda: 5 vs 4 (+1)
Marginal win, same as Run 001. Judge 3 scored engine 3 for its "Revisions to earlier predictions" mechanism. Other 2 judges noted the revisions are "conditional hedges" rather than genuine temporal learning.

## Where Both Score 2 (Tied) — 8 Criteria

### Challenge (tied 2/2) — **Primary fix target**
The critic probe successfully used web search and found external evidence (OWASP, NIST, MCP auth spec), but the evidence was used to *enrich* the source thesis rather than *challenge* it. The anchor-3 requirement is: "Makes a specific prediction that contradicts the source, with evidence from the source itself, AND explains the mechanism by which the source's assumption fails." The anchor-4 requirement goes further: external evidence that overturns a source assumption. Current probes are cooperative — they find evidence that supports or nuances the thesis. The critic needs to be adversarial: find evidence that the source thesis is wrong, then make a specific prediction based on that contradiction.

### Grounding (tied 2/2) — **Secondary fix target**
External evidence is now present but the reasoning chains have gaps. The synthesis presents evidence and conclusions but the intermediate mechanism step is often implicit. "DORA says AI amplifies weaknesses → some firms will narrow permissions" has a plausible logical leap but the mechanism (specifically how amplification causes permission narrowing) isn't spelled out.

### Decision Clarity (tied 2/2)
Neither output opens with the #1 decision. Both bury decision points after extensive analysis. This was identified as Priority 2 in Run 001 diagnosis but was not implemented (one-change-per-iteration rule).

### Plausibility (tied 2/2)
External sources are cited but confidence levels lack derivation methodology. The 75%/70%/60% numbers appear asserted rather than derived from evidence.

### Breadth (tied 2/2)
Both outputs cover multiple themes. The engine labels 12 distinct themes but judges note overlap (trust deficit + governance-gated adoption; selection + harness quality). Cross-theme interactions are stated but don't produce non-obvious conclusions.

### Actionability (tied 2/2) — **Regressed from +2**
See regression analysis above. The synthesis template still mandates decision points but the longer output dilutes their impact.

### Completeness (tied 2/2)
The engine has explicit assumptions and methodology sections but per-claim assumptions are limited to 4 high-level items.

### Information Density (tied 2/2)
Some redundancy between findings and directions. 12 findings could be compressed to ~8 per Judge 3. The 42KB synthesis is 23% longer than Run 001's 34KB without proportional information gain.

## Recommended Changes for Next Iteration

**Priority 1: Adversarial critic probe.** The critic probe currently finds external evidence that enriches the source thesis. Change the critic probe prompt to require:
1. Finding at least one external signal that **contradicts** a core claim in the knowledge graph
2. Making a specific, dated prediction based on the contradiction (not just noting a tension)
3. Explaining the mechanism by which the source's assumption fails

This targets Challenge (the hardest remaining criterion to crack) and has spillover to Novelty (contradictory insights are inherently novel) and Grounding (the contradiction requires an explicit reasoning chain).

**Priority 2: Synthesis template — upfront executive recommendation.** Add a mandatory first section before the Executive Summary: "TOP RECOMMENDATION: [what to do], [by when], [what it costs], [what happens if you don't]." This directly targets Decision Clarity.

**Priority 3: Fix Actionability regression.** Add a word-count or section-count constraint to the synthesis template to prevent the output from growing without bound. Or require decision points to appear within the first 30% of the document.

Per meta-loop rules: make ONE targeted change per iteration. **Priority 1** is recommended because it targets the criterion with the most room for improvement (Challenge at 2 with no judge-level variation) and has the highest potential for cascade effects across Novelty, Grounding, and overall analytical depth.
