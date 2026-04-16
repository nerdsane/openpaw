# Run 003 Changelog

## Changed File
`os-apps/paw-foresight/wasm/spawn_orchestrator/src/lib.rs`

## What Changed

Added **content diversity constraints** to the WASM-embedded `ORCHESTRATION_INSTRUCTIONS` constant. Three areas modified:

### 1. Step B — Key Findings (theme diversity mandate)

**Before:** Findings required real company names and measurable indicators, but no theme diversity.

**After:** Added a `Theme:` field per finding and a diversity mandate:
- Must span at least 4 distinct themes from 7 categories (model/vendor, governance/policy, organizational/adoption, technical architecture, economics/market, evaluation/testing, cross-domain)
- Max 2 findings per theme
- At least 2 findings from adjacent-domain probe observations
- At least 1 finding from critic probe observations
- No single observation ID in more than 2 findings; must use 60%+ of available observations

### 2. Step E — Decision Points (actionability specificity)

**Before:** Options were described generically (e.g., "invest in governance").

**After:** Each option MUST name a specific tool, configuration, platform, or organizational action. Each tradeoff MUST include estimated effort (engineering-weeks, dollar amount, or team requirement). Added explicit example: "Deploy OPA/Cedar policy-as-code gates on the CI pipeline by Q3 2026."

### 3. Quality Rules — New "Content Diversity Rules" section (rules 8-14)

**Before:** 7 structural quality rules.

**After:** Added 7 content diversity rules:
- Rule 8: 4+ distinct themes in Key Findings, max 2 per theme
- Rule 9: No observation cited in more than 2 findings
- Rule 10: Use 60%+ of available observations
- Rule 11: 2+ findings from adjacent-domain probe
- Rule 12: Decision Point options must name specific tools with effort estimates
- Rule 13: Executive Summary must name 6+ entities across 3+ categories
- Rule 14: Each temporal phase must introduce 1+ new company/tool

Also updated Step G assembly instruction to specify "6+ companies/tools across 3+ categories" for the Executive Summary.

## Diff Summary

The embedded instruction string grew from ~6.5KB to ~7.8KB (within the 32KB WASM field budget). No structural changes to the synthesis template — only additive diversity guidance layered on top of the existing structural mandates from Run 002.
