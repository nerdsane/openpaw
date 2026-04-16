# Run 004 Changelog

## Changed File
`os-apps/paw-foresight/system/skills/orchestrate-projection/SKILL.md`

## What Changed
Added an observation deduplication phase in Step 4 (Convergence) that runs after cross-probe confirmation and before synthesis.

## Before
Step 4 only did cross-probe confirmation (confirm semantically similar observations from different probes) and contradiction detection. All observations — even redundant ones — passed through to synthesis unchanged.

## After
After confirmation, a new deduplication phase:
1. Groups all live (non-Faded) observations by semantic theme
2. For each theme with 3+ observations, ranks by quality (external evidence > specificity > importance)
3. Keeps the 2 strongest observations per theme, Fades the rest using the existing `Observation.Fade` entity action
4. Targets ≤15 total observations after deduplication
5. Logs dedup results for diagnosis

## Rationale
Run 003 produced 24 observations from 2 steps × 3 probes. Without deduplication, the synthesis contained massive thematic overlap:
- "Dark factory won't happen" in observations 8, 13, 20 and Direction 6
- "CI-governed patch factory" in observations 5, 7, 17, 18 and Directions 3, 5
- "Coordination bottleneck" in observations 1, 21, 22 and Directions 2, 4
- "Rigor ≠ autonomy" in observations 10, 13, 14 and Direction 1

All 3 judges scored Information Density at 1/4, costing -3 Borda. The deduplication step addresses this by pruning redundant observations before they reach synthesis.

## Diff Summary
Added ~50 lines to Step 4 (Convergence) in SKILL.md, between the confirmation loop and the `ConvergenceComplete` action dispatch.
