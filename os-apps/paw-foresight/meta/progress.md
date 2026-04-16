# Foresight Meta-Improvement Progress

## Rubric & Methodology

- **Rubric:** v4 — 12 criteria (Grounding replaces Transparency, Information Density replaces Quantitative Precision)
- **Judges:** 3 Claude Code subagents (`claude -p`), side-by-side comparison, randomized X/Y
- **Baseline (locked):** 25.0/48 raw, 51.0/72 Borda (scored 2026-04-16)
- **Constraints:** domain-agnostic, no-authoring, prefer-architecture
- **Convergence:** 2 consecutive incumbent wins

## Score Table

| Run | Tag | Engine Score | Baseline Score | Delta | Engine Borda | Baseline Borda | Winner | Streak | Key Change | Key Insight |
|-----|-----|-------------|---------------|-------|-------------|---------------|--------|--------|------------|-------------|
| 000 | foresight-v000 | 21.3/48 | 27.0/48 | -5.7 | 48.0/72 | 60.0/72 | baseline | 0 | Initial scoring (no changes) | Orchestrator WASM fuel exhaustion after 3 turns — no synthesis, convergence, or step 1. Engine wins only on Specificity (+2). Biggest gaps: Progression, Breadth, Decision Clarity, Completeness (all -3). |
| 001 | foresight-v001 | 27.0/48 | 24.0/48 | +3.0 | 58.5/72 | 49.5/72 | engine | 0 | Increase WASM fuel budget 50B→500B | Engine completes full pipeline; wins on Specificity (+3), Falsifiability (+3), Actionability (+2), Progression (+1). 8 criteria tied at 2. |
| 002 | foresight-v002 | 27.3/48 | 24.0/48 | +3.3 | 58.5/72 | 49.5/72 | engine | 1 | Add web search to probes (temper_web_search, temper_web_fetch) | Novelty breaks tie → engine +2, but Actionability regresses to tied. Net Borda unchanged from Run 001 (58.5 vs 49.5). Incumbent wins (no improvement). |
| 003 | — | 23.7/48 | 27.0/48 | -3.3 | 51.0/72 | 57.0/72 | incumbent | 2 | Adversarial critic probe (require contradiction, not enrichment) | Challenge hits target (2→3, unanimous), Novelty +2, Breadth +2, but orchestrator failure caused severe regressions: Falsifiability -3, Actionability -3, Info Density -3, Progression -2, Completeness -2. Change reverted. |

## Convergence Status

**Status:** CONVERGED. Run 003 incumbent wins — second consecutive incumbent win triggers convergence.
**Converged:** Yes (streak 2/2)
**A-wins streak:** 2
**Final engine version:** foresight-v002 (web search probes, original critic)
**Note:** Convergence may be premature — the adversarial critic change improved Challenge (its target) unanimously, but the orchestrator session failed before synthesis, causing structural regressions in 6 criteria unrelated to the prompt change. A clean engine run might have produced a different result.
