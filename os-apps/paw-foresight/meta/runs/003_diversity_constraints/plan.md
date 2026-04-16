# Run 003 Plan

## Target Criteria

- **Breadth**: Engine scored E=3.0 B=6.0 Borda (all 3 judges gave baseline 3, engine 2). Root cause: all 8 findings and 5 directions orbit the same thesis ("governed harness bundles"). The synthesis selects the same observations repeatedly.
- **Actionability**: Engine scored E=3.5 B=5.5 Borda. Decision points name abstract strategic choices ("invest in harnesses vs model access") rather than concrete tools, configurations, and organizational actions.
- **Specificity**: Engine scored E=4.0 B=5.0 Borda. The engine repeats the same small set of companies (Anthropic, OpenAI, Cursor, Temper, Cedar) without diversifying across categories.

## Root Cause

The WASM-embedded synthesis instructions enforce structural compliance (citations, falsification dates, decision point format) but do not enforce **content diversity**. The orchestrator is free to:
1. Pick the same observations repeatedly across findings
2. Frame all findings around the same core thesis
3. Write decision points as abstract strategy questions

## Planned Change

**ONE change: Add diversity constraints to the WASM-embedded synthesis instructions.**

Specifically, add these rules to the "Quality Rules" section of `ORCHESTRATION_INSTRUCTIONS` in `wasm/spawn_orchestrator/src/lib.rs`:

1. **Finding theme diversity**: Key Findings must span at least 4 distinct analytical themes. Define theme categories: (a) model/vendor dynamics, (b) governance/policy, (c) organizational/adoption, (d) technical architecture, (e) economics/market, (f) evaluation/testing, (g) cross-domain analogies. No more than 2 findings may share the same primary theme.
2. **Observation deduplication**: Track which observation IDs are cited. No single observation may appear in more than 2 findings. The synthesis must use at least 60% of all available observations (not just the high-importance ones).
3. **Cross-probe mandate**: At least 2 Key Findings must derive from the adjacent-domain probe's observations (not just practitioner/critic probes). At least 1 finding must derive from the critic probe specifically.
4. **Actionability specificity**: Each Decision Point must name a specific tool, configuration, or organizational action (e.g., "deploy Cedar policy gates on CI" not "invest in governance"). Each option must include an estimated cost or effort level.
5. **Specificity diversity**: The Executive Summary must name at least 6 distinct companies/tools across at least 3 categories (vendors, governance tools, open-source projects, enterprise platforms).

## Expected Impact

- **Breadth**: 2 → 3. Theme diversity mandate forces coverage of 4+ themes instead of 1.
- **Actionability**: 2 → 3. Specific tool/config naming replaces abstract strategy.
- **Specificity**: Marginal improvement from diversity requirement.
- **Risk**: Falsifiability and Decision Clarity may regress if the template becomes too constrained and the orchestrator can't fill fields as naturally. Mitigation: constraints are additive guidance, not structural changes.
