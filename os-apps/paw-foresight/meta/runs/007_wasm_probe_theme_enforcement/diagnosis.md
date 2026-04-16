# Run 007 Diagnosis

## Summary
**Engine: 27.0/48 | Baseline: 25.7/48 | Delta: +1.3 | Borda: 56.0 vs 52.0 (+4.0)**
Fifth consecutive engine win. WASM-level probe theme enforcement achieved direction diversity (5 categories, 0 governance clustering) but Breadth Borda did not improve — baseline still wins 6.0 to 3.0. The engine gained on Falsifiability (+3.0), Transparency (+2.0), and Quantitative Precision (+2.0), while losing Breadth (-3.0) as before.

## Borda Breakdown

| Criterion | Engine Borda | Baseline Borda | Delta |
|-----------|-------------|---------------|-------|
| Specificity | 4.5 | 4.5 | 0.0 |
| Novelty | 4.5 | 4.5 | 0.0 |
| **Falsifiability** | **6.0** | **3.0** | **+3.0** |
| **Breadth** | **3.0** | **6.0** | **-3.0** |
| Plausibility | 4.5 | 4.5 | 0.0 |
| Progression | 4.5 | 4.5 | 0.0 |
| Actionability | 4.5 | 4.5 | 0.0 |
| Decision Clarity | 4.5 | 4.5 | 0.0 |
| Completeness | 4.5 | 4.5 | 0.0 |
| **Transparency** | **5.5** | **3.5** | **+2.0** |
| Challenge | 4.5 | 4.5 | 0.0 |
| **Quant Precision** | **5.5** | **3.5** | **+2.0** |

## Lowest-Scoring Criteria

### Breadth (Engine avg: 2.00/4, Baseline avg: 3.00/4, Borda: E=3.0 B=6.0)
- All 3 judges scored engine Breadth at 2, baseline at 3
- Engine covers 5 direction themes (economics/market, technical-architecture, evaluation/testing, organizational/adoption, cross-domain) with connections noted
- Baseline covers 6+ themes with explicit cross-theme interactions that produce non-obvious conclusions
- **Root cause:** Direction diversity is necessary but not sufficient for Breadth. The engine has 5 distinct themes in its directions, but the synthesis does not articulate enough cross-theme interactions. The rubric requires "6+ themes with explicit cross-theme interactions where the interaction produces a non-obvious conclusion" for a 3. The engine's synthesis treats themes somewhat independently — each Key Finding and Direction is self-contained within its theme. The baseline, being a single coherent narrative, naturally weaves themes together.
- **Fix:** The synthesis template needs an explicit cross-theme interaction section that forces the synthesizer to connect findings across themes and derive non-obvious conclusions from those connections.

### Novelty (Engine avg: 2.00/4, Baseline avg: 2.00/4, Borda: tie)
- Both outputs score 2 (1-2 original insights), neither reaches 3 (multiple insights from OUTSIDE input)
- **Root cause:** Probes use web search but their observations stay close to the domain. The rubric requires "grounded in evidence FROM OUTSIDE the input" for a 3.
- **Fix:** Adjacent-domain probes could be given more aggressive cross-domain search prompts (e.g., search for analogies in specific other industries rather than general DSE terms).

## What the Change Achieved

The WASM-level theme enforcement worked exactly as designed:
- 12 directions across 5 categories (economics/market: 2, technical-architecture: 2, evaluation/testing: 2, organizational/adoption: 2, cross-domain: 4)
- Zero governance-only clustering (first time in the meta-improvement loop)
- Probes operated independently (each read ForesightModel directly, used web search)
- Orchestrator was simplified to just wait + synthesize

However, **direction diversity did not translate to Breadth score improvement**. The hypothesis from Run 006 was that prose-based diversity constraints were the bottleneck. Moving constraints to WASM proved the orchestrator was indeed ignoring prose, but the real Breadth deficit is in the synthesis, not the directions.

## Why the Engine Wins

The engine's structural advantages over the baseline:
1. **Falsifiability:** Multi-agent probes produce explicit falsification criteria with dates and reasoning (rubric 3). The baseline's predictions are less systematically falsifiable.
2. **Transparency:** Engine's synthesis cites specific observations throughout. Baseline uses vaguer attribution.
3. **Quantitative Precision:** Engine probes generate measurable indicators and thresholds. Baseline stays more qualitative.

These three criteria have been consistent engine strengths since Run 003. The engine's architecture naturally produces referenced, falsifiable, quantitative content because probes create structured observations.

## Why the Baseline Still Wins Breadth

The baseline synthesis is a single narrative written in one pass. This naturally produces cross-theme interactions because the author is simultaneously aware of all themes. The engine's multi-probe architecture produces deep, themed analysis but the synthesis step doesn't sufficiently weave themes together.

The Breadth rubric anchor for 3 is: "6+ themes with explicit cross-theme interactions where the interaction produces a non-obvious conclusion (e.g., 'governance constraints reshape vendor economics' leading to a specific predicted outcome neither theme implies alone)."

The engine needs: (1) more than 5 themes, and (2) explicit cross-theme reasoning that produces novel conclusions.

## Recommended Changes for Next Iteration

**Priority 1:** Add a cross-theme interaction section to the synthesis template. After listing Key Findings and Directions by theme, the synthesizer should produce 3-5 explicit cross-theme interactions, each stating: "Theme A + Theme B implies [non-obvious conclusion]." This addresses the Breadth rubric requirement directly.

**Priority 2:** If Breadth doesn't improve, add a 6th theme category to the probe set (e.g., "governance/policy" or "labor-market/workforce") to increase coverage from 5 to 6+ themes.

Per meta-loop rules: make ONE targeted change per iteration.

## Orchestrator Behavior

The orchestrator session failed on turn 1 with a WASM memory error (user_message was 14,782 bytes). This is the same overflow issue seen in Run 003. Synthesis was manually delegated to a new session, which produced 38KB of output across 5 turns. The orchestrator failure is a recurring technical issue, not a strategic one — the WASM context buffer size needs to be increased, or the orchestrator's instructions need to be shortened.

## Theme Enforcement Validation

| Probe Pair | Assigned Theme | Direction Themes Produced | Match? |
|------------|---------------|--------------------------|--------|
| Practitioner S0/S1 | technical-architecture OR evaluation/testing | technical-architecture (2), evaluation/testing (2) | Yes |
| Critic S0/S1 | economics/market OR organizational/adoption | economics/market (2), organizational/adoption (2) | Yes |
| Adjacent S0/S1 | cross-domain | cross-domain (4) | Yes |

All 6 probes respected their theme constraints. The WASM-level enforcement is verified working.
