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

## Convergence Status

**Status:** Run 002 complete — engine beats baseline but Borda unchanged from Run 001. Incumbent wins (challenger did not improve).
**Converged:** No (streak 1/2)
**A-wins streak:** 1
