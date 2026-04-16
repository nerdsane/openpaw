# Run 005 Plan

## Target Criteria

- **Breadth** (Engine: 3.0/6.0 Borda, Baseline: 6.0/6.0): The engine's largest single deficit (-3.0 Borda). All 3 judges scored baseline 3 vs engine 2 in Run 004. Root cause: 10 of 12 Active Directions repeat governance/controlled-autonomy themes from slightly different angles. Even though Key Findings span 6 themes per the diversity mandate, the massive governance-dominated Directions section creates perceptual convergence across the entire output.

## Root Cause Analysis

The synthesis template (Step C) says "For each active direction, include its FULL reasoning text." This dumps ALL active directions into the output verbatim. In Run 004, the engine produced 12 directions — nearly all governance-themed:

1. "Governed control-plane problem"
2. "Coordination-cost compression"  
3. "CI-governed multi-agent delivery pipelines"
4. "Evaluation debt outruns model gains"
5. "Governed containment"
6. "Governed coordination"
7. "Modular agent stacks with artifact-grade traces"
8. "Governed multi-agent delivery fabric"
9. "Exception ownership vs policy syntax"
10. "Governance-grade evaluation"
11. "Workflow defensibility"

The result: ~6,000 words of directions that all say "governance > autonomy" in different ways. This drowns out the thematic diversity that the Key Findings section establishes. The baseline avoids this by having only 5 focused, thematically distinct Active Directions.

## Planned Change

**File:** `os-apps/paw-foresight/wasm/spawn_orchestrator/src/lib.rs` (SYNTHESIS_TEMPLATE constant)

**What changes:** Replace Step C ("Build Active Directions") with a Direction Selection & Consolidation step that:

1. Groups all active directions by primary theme (governance/policy, technical architecture, economics/market, organizational/adoption, evaluation/testing, cross-domain)
2. Selects at most **5 directions** spanning at least **4 distinct themes**
3. If more than 2 directions share a theme, **merges** them into a single consolidated direction keeping the strongest reasoning and counterfactual
4. Requires at least 1 direction about technology/markets/economics (not governance)
5. For the selected 5, keeps full reasoning — does NOT truncate

This is a single change to one section of the synthesis template. It does not affect Key Findings, Temporal Progression, Predictions, Decision Points, or any other section.

## Expected Impact

- **Breadth:** Should improve from 2→3 per judge. 5 thematically diverse directions (vs 10+ governance-dominated) will read as analytically broad rather than monothematic.
- **Plausibility:** May recover — fewer, more focused directions mean each one's evidence chain is clearer.
- **Actionability, Progression, Challenge, Quant Precision:** Should be unaffected — those sections are controlled by other template steps.
- **Risk:** If the engine's observations are genuinely governance-dominated, forcing theme diversity in directions may produce weaker non-governance directions. But the Key Findings diversity mandate already shows the observations span 6+ themes, so the material exists.

## Why ONE Change

The direction dump is the single biggest structural difference between the engine output and the baseline. The baseline has 5 focused directions; the engine has 10+ repetitive ones. Fixing this one bottleneck should unlock Breadth without regressing the gains from Runs 003-004.
