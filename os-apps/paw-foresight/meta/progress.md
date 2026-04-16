# Foresight Meta-Improvement Progress

## Score Table

| Run | Tag | Engine Score | Baseline Score | Delta | Engine Borda | Baseline Borda | Winner | Streak | Key Change | Key Insight |
|-----|-----|-------------|---------------|-------|-------------|---------------|--------|--------|------------|-------------|
| 000 | foresight-v000 | 23.7/48 | 27.0/48 | -3.3 | 49.0/72 | 59.0/72 | Baseline | 0 | Hybrid restructure (skill-based orchestration) | Rubric v3 (tightened 3-level anchors + 3+ cap rule). 3 independent paw-agent judges. Engine weakest on Transparency (1.0) and Quant Precision (1.0). Baseline wins on Specificity, Falsifiability, Actionability. ~24 pts engine headroom. |
| 001 | — | 25.4/48 | 27.0/48 | -1.6 | 51.5/72 | 56.5/72 | Baseline | 1 | Synthesis template quality mandates | Gap narrowed (-3.3→-1.6 raw, -10→-5 Borda). Template mandates NOT followed by orchestrator. Novelty +0.7, Challenge +0.4 (from probes). Specificity/Quant/Actionability unchanged. Root cause: prose mandates are advisory, not enforced. |

## Version History

- **foresight-v000** (2025-04-15): Initial hybrid architecture. 1 WASM (spawn_orchestrator) + orchestration skill. 3 probes, 2 steps, gpt-5.4.
- **rubric-v3** (2025-04-16): Tightened 3-level anchors on Novelty (require external evidence), Breadth (require non-obvious conclusions), Progression (require revision of earlier predictions), Challenge (require failure mechanism). Added 3+ cap rule (max 3 criteria at 3+ per output per judge). Rescored Run 000 with 3 independent paw-agent judges.

## Convergence Status

**Status:** In progress
**Current incumbent:** Baseline (single-shot)
**A-wins streak:** 1
**Converged:** No
**Judge infrastructure:** Operational — 3 independent paw-agent sessions per scoring round (split-session: 6 total, one per output per judge, to stay under 32KB WASM field limit)
