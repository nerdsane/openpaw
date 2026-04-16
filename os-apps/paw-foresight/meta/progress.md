# Foresight Meta-Improvement Progress

## Score Table

| Run | Tag | Engine Score | Baseline Score | Delta | Engine Borda | Baseline Borda | Winner | Streak | Key Change | Key Insight |
|-----|-----|-------------|---------------|-------|-------------|---------------|--------|--------|------------|-------------|
| 000 | foresight-v000 | 23.7/48 | 27.0/48 | -3.3 | 49.0/72 | 59.0/72 | Baseline | 0 | Hybrid restructure (skill-based orchestration) | Rubric v3 (tightened 3-level anchors + 3+ cap rule). 3 independent paw-agent judges. Engine weakest on Transparency (1.0) and Quant Precision (1.0). Baseline wins on Specificity, Falsifiability, Actionability. ~24 pts engine headroom. |
| 001 | — | 25.4/48 | 27.0/48 | -1.6 | 51.5/72 | 56.5/72 | Baseline | 1 | Synthesis template quality mandates | Gap narrowed (-3.3→-1.6 raw, -10→-5 Borda). Template mandates NOT followed by orchestrator. Novelty +0.7, Challenge +0.4 (from probes). Specificity/Quant/Actionability unchanged. Root cause: prose mandates are advisory, not enforced. |
| 002 | — | 27.0/48 | 27.0/48 | 0.0 | 54.0/72 | 54.0/72 | Baseline (tie) | 2 | Data-driven synthesis + WASM rebuild | Gap closed to 0 (from -5 Borda Run 001). Root cause fix: orchestrator never read SKILL.md (not in TemperFS); rebuilt WASM embeds 6.5KB instructions. Engine wins Decision Clarity (+2), Falsifiability (+2), Transparency (+1), Novelty (+1). Baseline wins Breadth (+3), Actionability (+2), Specificity (+1). Remaining gap: content diversity, not structure. |
| 003 | foresight-v003 | 27.0/48 | 25.7/48 | +1.3 | 56.0/72 | 52.0/72 | Engine | 0 | Diversity constraints in synthesis template | First engine win. Added theme diversity mandate (4+ themes, max 2 per theme), obs dedup, cross-probe requirements, actionability specificity. Falsifiability (+3), Transparency (+2), Quant Precision (+2), Completeness (+1). Breadth gap halved (-3→-1). Orchestrator crashed (WASM context overflow); synthesis via dedicated session. |
| 004 | — | 27.0/48 | 26.0/48 | +1.0 | 55.5/72 | 52.5/72 | Engine | 0 | Synthesis delegation + progression/challenge fixes | Second engine win. Split WASM instructions into orchestration + synthesis template. Orchestrator completed in 13 turns (no crash). Progression flipped from loss to win (+1.5→+2.0 delta). Challenge moved from loss to tie. Breadth regressed (-1→-3 gap). No malformed citations. Delegation architecture coded but not exercised — orchestrator fit in context. |
| 005 | — | 28.3/48 | 26.3/48 | +2.0 | 56.0/72 | 52.0/72 | Engine | 0 | Direction diversity constraint (not followed) | Third engine win. Added Step C direction selection/consolidation to synthesis template (select 5 directions spanning 4+ themes, cap governance at 1). Constraint NOT followed — orchestrator included all 12 directions (same as Run 004). Breadth unchanged (-3.0). Specificity jumped (+3.0 Borda, all judges scored engine 3-4). Falsifiability jumped (+3.0 Borda). Plausibility recovered to tie. Prose constraints remain advisory; structural enforcement needed. |
| 006 | — | 27.7/48 | 26.0/48 | +1.7 | 55.0/72 | 53.0/72 | Engine | 0 | Direction consolidation (not followed) | Fourth engine win. Added direction consolidation step to orchestrator convergence (archive excess directions by theme, keep max 5). NOT followed — orchestrator completed in 7 turns, skipped convergence/consolidation/delegation. All 12 directions active (0 archived), but synthesis used only 6. Breadth unchanged (-3.0). Falsifiability strong (+3.0). Specificity regressed to tie (was +3.0). Margin narrowing (Borda +2.0 vs +4.0 Run 005). Prose instructions proven unreliable across 5 runs. |

## Version History

- **foresight-v000** (2025-04-15): Initial hybrid architecture. 1 WASM (spawn_orchestrator) + orchestration skill. 3 probes, 2 steps, gpt-5.4.
- **rubric-v3** (2025-04-16): Tightened 3-level anchors on Novelty (require external evidence), Breadth (require non-obvious conclusions), Progression (require revision of earlier predictions), Challenge (require failure mechanism). Added 3+ cap rule (max 3 criteria at 3+ per output per judge). Rescored Run 000 with 3 independent paw-agent judges.

## Version History

- **foresight-v003** (2026-04-16): Added content diversity constraints to WASM-embedded synthesis instructions. Theme diversity (4+ themes), observation deduplication, cross-probe requirements, actionability specificity. First engine win.

## Convergence Status

**Status:** In progress
**Current incumbent:** Engine (Run 006)
**A-wins streak:** 0 (engine won again — four consecutive engine wins, Runs 003-006)
**Converged:** No
**Judge infrastructure:** Operational — 3 independent paw-agent sessions per scoring round (split-session: 6 total, one per output per judge, to stay under 32KB WASM field limit). Engine output must be condensed to <29KB for judging due to 32KB WASM field limit.
**Key persistent deficit:** Breadth (E=3.0 B=6.0 Borda, unchanged across Runs 004-006). Root cause: prose-based constraints are advisory — proven across 5 runs (001, 005, 006). Structural enforcement (WASM or probe-level theme mandates) needed.
**Margin trend:** Engine Borda margin narrowing: +4.0 (Run 003) → +3.0 (Run 004) → +4.0 (Run 005) → +2.0 (Run 006).
