# Run 005 Changelog

## Changed File

`os-apps/paw-foresight/wasm/spawn_orchestrator/src/lib.rs` (SYNTHESIS_TEMPLATE constant, Step C)

## What Changed

Replaced the "Build Active Directions" step with a "Select & Consolidate Active Directions" step that enforces thematic diversity.

### Before (Run 004)

Step C instructed: "For each active direction, include its FULL reasoning text from the entity, not a summary."

This dumped ALL active directions (12 in Run 004) into the output verbatim. 10 of 12 were governance-themed, creating a monothematic block of ~6,000 words.

### After (Run 005)

Step C now has three sub-steps:

1. **C1: Classify** — Assign each direction exactly one primary theme from: governance/policy, technical architecture, economics/market, organizational/adoption, evaluation/testing, cross-domain.

2. **C2: Select** — Choose at most 5 directions spanning at least 4 distinct themes:
   - Max 2 per theme; merge if 3+
   - If governance/policy has the most directions, it gets at most 1 slot
   - At least 1 direction must be economics/market or cross-domain
   - At least 1 direction must be technical architecture

3. **C3: Write** — For each selected direction (max 5), include full reasoning, observations, counterfactual, and a theme tag. Merged directions combine reasoning and cite all supporting observation IDs.

## Diff Summary

```
-### Step C: Build Active Directions
-For each active direction, include its FULL reasoning text from the entity, not a summary:
-[simple template with title, ID, reasoning, observations, counterfactual]

+### Step C: Select & Consolidate Active Directions (BREADTH-CRITICAL)
+**DO NOT dump all directions.** Too many directions on the same theme creates perceived
+monothematic output. You MUST select and consolidate.
+[3 sub-steps: classify by theme, select 5 spanning 4+ themes, write with theme tags]
+[governance capped at 1 slot if dominant, economics/cross-domain required]
```

## No Other Files Changed

Only the SYNTHESIS_TEMPLATE constant in `lib.rs` was modified. The ORCHESTRATION_INSTRUCTIONS, WASM logic, entity specs, and all other engine components are unchanged.
