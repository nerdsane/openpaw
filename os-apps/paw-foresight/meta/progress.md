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

## Convergence Status

**Status:** Run 000 complete — baseline wins initial scoring
**Converged:** No
