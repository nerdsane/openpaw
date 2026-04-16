# Run 008 Diagnosis

## Summary
**Engine: 28.7/48 | Baseline: 26.3/48 | Delta: +2.3 | Borda: 55.5 vs 52.5 (+3.0)**

Sixth consecutive engine win. The planned Cross-Theme Interactions section was added to the synthesis template but the synthesizer agent skipped it entirely — the output has no cross-theme section. Breadth remains the persistent deficit (E=3.0, B=6.0 Borda, unchanged across Runs 004-008). New gain: Progression improved significantly (+2.0 Borda, engine avg 3.3/4) with genuine temporal revision across phases.

## Borda Breakdown

| Criterion | Engine Borda | Baseline Borda | Delta | Engine Avg | Baseline Avg |
|-----------|-------------|---------------|-------|-----------|-------------|
| Specificity | 4.0 | 5.0 | -1.0 | 2.7 | 3.0 |
| Novelty | 5.0 | 4.0 | +1.0 | 2.3 | 2.0 |
| **Falsifiability** | **6.0** | **3.0** | **+3.0** | 4.0 | 2.0 |
| **Breadth** | **3.0** | **6.0** | **-3.0** | 2.0 | 3.0 |
| Plausibility | 4.5 | 4.5 | 0.0 | 2.0 | 2.0 |
| **Progression** | **5.5** | **3.5** | **+2.0** | 3.3 | 2.3 |
| Actionability | 4.0 | 5.0 | -1.0 | 2.3 | 2.7 |
| Decision Clarity | 4.5 | 4.5 | 0.0 | 2.0 | 2.0 |
| Completeness | 4.5 | 4.5 | 0.0 | 2.0 | 2.0 |
| Transparency | 5.0 | 4.0 | +1.0 | 2.0 | 1.7 |
| Challenge | 4.5 | 4.5 | 0.0 | 2.0 | 2.0 |
| Quant. Precision | 5.0 | 4.0 | +1.0 | 2.0 | 1.7 |

## What Changed This Run

**Planned change:** Added a mandatory "Step C4: Cross-Theme Interactions" section to the SYNTHESIS_TEMPLATE in `wasm/spawn_orchestrator/src/lib.rs`. The section instructed the synthesizer to produce 4-5 entries, each connecting two different themes to derive a non-obvious conclusion with specific format requirements and citation rules. Updated Step G assembly order to include it as a mandatory section. Added a Content Diversity Rule #15.

**Actual behavior:** The synthesizer agent produced the same output structure as Run 007 — it followed all existing template sections but completely ignored the new Cross-Theme Interactions section. The output went directly from Active Directions to Source Thesis Challenges, skipping Step C4.

**This confirms the pattern observed across 6 runs (003-008):** Prose-based structural mandates in the synthesis template are unreliable. The synthesizer agent:
- Follows existing sections it was already following (Key Findings, Temporal Progression, Active Directions, etc.)
- Ignores NEW sections added to the template in later iterations
- Does not enforce diversity/structural constraints embedded in prose

This is the same failure mode seen in Runs 005-006 where direction consolidation instructions were ignored, and similar to Runs 001-002 where quality mandates were ignored before the WASM-embedding fix.

## Lowest-Scoring Criteria

### Breadth (Engine: 2.0/4 avg, Baseline: 3.0/4 avg, Borda: E=3.0 B=6.0)
- All 3 judges scored engine Breadth at 2, baseline at 3 — unanimous
- The engine covers 6 themes in Key Findings (technical architecture, economics/market, evaluation/testing, cross-domain, organizational/adoption, governance/policy) but treats them largely independently
- The baseline weaves themes into a single narrative with natural cross-theme interactions
- **Root cause:** Absence of a dedicated cross-theme interaction section. The template mandated it, the synthesizer skipped it. The engine's multi-probe architecture produces deep thematic analysis but no section forces cross-theme reasoning.
- **Why prose failed:** The synthesizer reads the template from a workspace file. It follows the template's existing structure (which it has executed successfully in prior runs) but ignores additions. The template grew from ~10 sections to ~11 sections, and the agent may be using a cached understanding of the expected output structure rather than re-reading the template on each run.

### Actionability (Engine: 2.3/4 avg, Baseline: 2.7/4 avg, Borda: E=4.0 B=5.0)
- Slight baseline advantage. Engine Decision Points name specific tools (Cedar, OPA, Buildkite) but tradeoff quantification is generic ("2-4 engineering-weeks" patterns repeat).
- Baseline has slightly more natural integration of recommendations into the narrative.

### Specificity (Engine: 2.7/4 avg, Baseline: 3.0/4 avg, Borda: E=4.0 B=5.0)
- Minor baseline advantage. Both name real actors and timelines. Baseline slightly more consistent in linking actors to dates.

## What the Engine Gains

### Falsifiability (Engine: 4.0/4 avg, Baseline: 2.0/4 avg, Borda: E=6.0 B=3.0)
- All 3 judges scored engine at 4, baseline at 2 — unanimous
- Engine has explicit "If [condition] has not occurred by [date], this prediction is wrong because [mechanism]" for all 5 predictions
- This is the engine's strongest criterion across all runs — structural advantage from the mandatory falsification template

### Progression (Engine: 3.3/4 avg, Baseline: 2.3/4 avg, Borda: E=5.5 B=3.5)
- NEW gain this run. Engine averaged 3.3 (up from ~2.5 in prior runs)
- Engine's Phase 2-4 Revisions sections are substantive: they revise specific earlier predictions with mechanisms
- Phase 4 explicitly falsifies earlier predictions (e.g., "Earlier confidence that model quality alone would unlock scale is effectively falsified")
- Baseline has temporal phases but predictions are more independent snapshots

## What the Change Did NOT Achieve

1. **Cross-theme interactions section was skipped** — zero impact on Breadth
2. **Actionability regressed slightly** (-1.0 Borda vs Run 007's 0.0 tie) — possibly because the synthesis is longer (34KB) and decision points feel formulaic
3. **Specificity regressed slightly** (-1.0 Borda vs Run 007's 0.0 tie)

## Why the Engine Wins

Same structural advantages as Runs 003-007:
1. **Falsifiability:** Multi-probe architecture + mandatory falsification template = consistent 4.0/4
2. **Progression:** Improved temporal revision in synthesis = now 3.3/4
3. **Transparency + Quant Precision:** Observation citations and measurable indicators throughout

## Why Breadth Remains the Deficit

**Six runs of evidence (003-008) now prove that the Breadth deficit cannot be fixed through the synthesis template alone.** Every prose-based intervention has been ignored:
- Run 003: Diversity constraints → not followed by orchestrator
- Run 005: Direction selection/consolidation → not followed by orchestrator  
- Run 006: Direction consolidation in convergence → not followed by orchestrator
- Run 007: Probe theme enforcement in WASM → followed, but didn't fix Breadth (directions diverse, synthesis independent)
- Run 008: Cross-theme interactions section in template → synthesizer skipped the section entirely

The pattern is clear: **WASM-level interventions work (Run 007 theme enforcement was followed). Prose-level interventions in the synthesis template do not work.** The fix must be structural at the WASM level, not advisory in the template.

## Recommended Changes for Next Iteration

**Priority 1:** Move cross-theme interaction generation to WASM or a dedicated pre-synthesis session.

Options:
- **Option A (WASM approach):** The WASM module creates a dedicated "cross-theme analyst" session after probes complete, which reads all observations and directions, identifies cross-theme pairs, and writes a cross-theme analysis file. The synthesizer reads this file as input — it doesn't need to generate cross-theme reasoning, just incorporate pre-computed interactions.
- **Option B (Split synthesis):** Create TWO synthesis sessions: (1) a "structure synthesizer" that loads observations/directions and generates a structured JSON with cross-theme interactions, then (2) a "narrative synthesizer" that reads that JSON and produces the final markdown.

Either approach enforces cross-theme reasoning at the structural level rather than relying on a prose mandate the agent ignores.

**Priority 2:** If Option A doesn't work, increase the number of probes to 8 (adding governance/policy and workforce/labor-market themes) to increase raw theme coverage. The hypothesis is that with 8+ themes in observations, even an unstructured synthesis will naturally reference more cross-theme interactions.

Per meta-loop rules: make ONE targeted change per iteration.
