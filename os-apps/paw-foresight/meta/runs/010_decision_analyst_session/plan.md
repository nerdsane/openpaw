# Run 010 Plan

## Target Criteria

- **Actionability**: engine scored 2.0/4 avg (Borda E=3.0, B=6.0, delta -3.0). Root cause: synthesis template Decision Points section produces formulaic tradeoffs (engineering-weeks, dollar amounts) without strategic cost-benefit-risk framing. Baseline integrates actionable recommendations naturally into the narrative with clearer "so what" framing.
- **Completeness** (secondary): engine scored 1.3/4 avg (Borda E=3.5, B=5.5, delta -2.0). Partially an artifact of output trimming for the 32KB WASM judge field limit — the untrimmed output includes Assumptions & Limitations sections that were removed for judging.

## Planned Change

**Add a dedicated WASM-created "decision analyst" session** that runs between probe completion and synthesis, following the same structural pattern that resolved the Breadth deficit in Run 009 (cross-theme analyst).

### What changes in the WASM (`spawn_orchestrator/src/lib.rs`):

1. **New constant `DECISION_ANALYST_PROMPT`** — prompt for a dedicated decision analyst session that:
   - Waits for all probe sessions to complete (same pattern as cross-theme analyst)
   - Reads all observations and directions from the API
   - Identifies the top 3 decision points from the observation/direction data
   - For each decision point, produces a rich decision framework:
     - WHO decides (named role: VP Engineering, Platform Lead, etc.)
     - WHEN to decide (observable trigger with approximate date)
     - 3 concrete options naming specific tools/platforms/configs
     - For each option: quantified cost, specific risk, opportunity cost, and strategic consequence
     - Comparative analysis: "Option A is best when X, Option B when Y"
     - Recommended option with quantified justification
   - Writes output to a workspace file (`decision_analysis.md`)

2. **Create the decision analyst session** in the WASM `run()` function, after probe creation but alongside the cross-theme analyst (they can run concurrently since both wait for probes).

3. **Update orchestrator instructions** to:
   - Wait for the decision analyst session (in addition to probes and cross-theme analyst)
   - Read the decision analyst's output from its workspace
   - Inject the pre-computed decision content into the synthesis template via `===DECISION_CONTENT===` placeholder

4. **Update synthesis template** Step E (Decision Points) to use pre-computed content from the decision analyst, same pattern as Step C4 (Cross-Theme Interactions).

### What does NOT change:
- Probe configuration (6 probes, theme-constrained)
- Cross-theme analyst (unchanged)
- All other synthesis template sections
- Entity specs, Cedar policies

## Expected Impact

| Criterion | Current Borda | Expected Direction | Mechanism |
|-----------|--------------|-------------------|-----------|
| Actionability | E=3.0, B=6.0 (-3.0) | Improve to tie or win | Pre-computed decision frameworks with strategic framing replace formulaic template output |
| Completeness | E=3.5, B=5.5 (-2.0) | Slight improvement | Better-structured decision content adds pipeline completeness |
| Progression | E=6.0, B=3.0 (+3.0) | Hold | No change to temporal structure |
| Breadth | E=4.5, B=4.5 (tie) | Hold | Cross-theme analyst unchanged |

Risk: Decision analyst session may fail at runtime (same `resolve_provider_api_key` WASM error that hit the cross-theme analyst in Run 009). Mitigation: if the analyst fails, the synthesizer falls back to generating its own decision points (current behavior).
