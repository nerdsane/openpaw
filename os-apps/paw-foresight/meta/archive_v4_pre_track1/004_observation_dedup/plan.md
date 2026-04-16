# Run 004 Plan

## Context

Run 003 converged the loop (streak 2/2), but convergence is arguably premature. The adversarial critic change in Run 003 hit its target (Challenge 2→3, unanimous), but the orchestrator session failed before synthesis due to a transient `resolve_provider_api_key` error. The assembled synthesis lacked key structured sections, causing 6 criteria to regress (-14 Borda). A secondary root cause: 24 observations from 2 steps × 3 probes with no deduplication created massive redundancy (Information Density scored 1/4).

Run 004 overrides premature convergence and targets the secondary root cause: observation redundancy.

## Target Criteria
- **Information Density**: Run 003 challenger scored 1.0/4. Root cause: 24 observations with 4+ thematic clusters repeating the same claims. Convergence step only confirms/contradicts — it does NOT merge or prune semantically overlapping observations.
- **Spillover targets**: Completeness (fewer, higher-quality observations → cleaner synthesis), Decision Clarity (less noise for the orchestrator to cut through during synthesis).

## Planned Change
**File:** `os-apps/paw-foresight/system/skills/orchestrate-projection/SKILL.md`
**Section:** Step 4: Convergence

**What changes:** After the existing cross-probe confirmation loop, add an explicit observation deduplication phase that:
1. Groups all observations by semantic theme (observations describing the same phenomenon)
2. For each cluster of 3+ overlapping observations, keeps the 2 strongest (best external evidence, most specific claims, highest importance) and Fades the rest using the existing `Observation.Fade` action
3. Targets ≤15 total observations after deduplication
4. Records which observations were faded and why

This is a structural change because it uses existing entity state machine transitions (Fade) to prune the data pipeline between convergence and synthesis. It modifies the data flow, not just prose instructions.

## Why NOT Re-Apply the Adversarial Critic
The adversarial critic was reverted per tournament protocol when the incumbent won Run 003. Per the rules, the change lost — even though the loss was caused by infrastructure failure, not the change quality. Run 004 makes a different change. If deduplication alone improves scores, the adversarial critic can be retried in Run 005.

## Expected Impact
- **Information Density**: 1 → 2+ (fewer redundant observations = less repetition in synthesis)
- **Completeness**: 1.7 → 2+ (orchestrator has cleaner data for structured sections)
- **Decision Clarity**: 1.7 → 2 (less noise to cut through)
- **Other criteria**: neutral (deduplication doesn't affect probe quality, only post-processing)
- **Net Borda delta**: +3 to +6 if Information Density and Completeness improve
