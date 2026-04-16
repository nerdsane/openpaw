# Run 003 Diagnosis

## Summary

**Engine: 27.0/48 | Baseline: 25.7/48 | Delta: +1.3 raw**
**Engine Borda: 56.0/72 | Baseline Borda: 52.0/72 | Delta: +4.0**
**Winner: Engine** (first engine win in the meta-loop)

The diversity constraints added to the WASM synthesis instructions produced the first engine victory.
The engine now wins on 4 criteria, loses on 4, and ties on 4 — compared to Run 002 where it won 4,
lost 4, and tied 4 but at a 54-54 stalemate. The shift comes from Quantitative Precision (+2) and
Completeness (+1) moving from ties to engine wins, while Breadth loss shrank from -3 to -1.

## What Improved

Criteria where engine gained Borda vs Run 002:

- **Falsifiability: E=6.0 B=3.0** (was E=5.5 B=3.5). All 3 judges scored engine 3, baseline 2.
  The engine's falsification conditions with explicit dates ("If by 2027-06-30...") are now
  consistently recognized as superior.

- **Transparency: E=5.5 B=3.5** (was E=5.0 B=4.0). Two judges scored engine higher. Dense
  [obs: ID] citations throughout the synthesis now consistently outperform the baseline's
  uncited claims.

- **Quantitative Precision: E=5.5 B=3.5** (was E=4.5 B=4.5). Was a tie in Run 002, now engine
  wins. The synthesis includes calibrated numbers: "<20% straight-through to production",
  "15-30% cycle time cut", "1.5x more engineering time to evals than prompts",
  ">70% require policy evaluation".

- **Completeness: E=5.0 B=4.0** (was E=4.5 B=4.5). Was a tie, now engine wins. J1 scored
  engine 2 vs baseline 1, citing the engine's structured Assumptions & Limitations section.

- **Breadth: E=4.0 B=5.0** (was E=3.0 B=6.0). Still a baseline win but gap halved. J3 now
  scores engine 3 (up from unanimous 2s in Run 002). The 8 findings span 6 distinct theme
  categories (tech architecture, governance/policy, evaluation/testing, economics/market,
  org/adoption, model/vendor).

## What Regressed or Failed to Improve

- **Progression: E=4.0 B=5.0** (was E=4.5 B=4.5). Was a tie, now baseline wins. J3 scored
  baseline 3 vs engine 2. The engine's temporal progression is dense with citations but the
  revision sections may be more formulaic than the baseline's natural causal flow.

- **Actionability: E=4.0 B=5.0** (was E=3.5 B=5.5). Marginal improvement in raw terms but
  still a baseline win. J3 gave baseline 3. Despite the actionability specificity mandate in
  the instructions, the decision point options still read as strategic recommendations rather
  than concrete operational playbooks.

- **Challenge: E=4.0 B=5.0** (was E=4.5 B=4.5). Was a tie, now baseline wins. J1 scored
  baseline 3 vs engine 2. The baseline's "What Might Surprise" section and thesis
  counterfactuals were rated as stronger challenges to the source material.

## Root Cause Analysis

**The diversity constraints worked for their target criteria but introduced a breadth-depth tradeoff.**

The template now mandates 4+ themes in Key Findings, 60%+ observation usage, and cross-probe
diversity. This produced:
1. 8 findings across 6 themes (vs Run 002's 8 findings orbiting 1 theme)
2. Dense observation citations throughout (56+ unique obs cited vs ~20 in Run 002)
3. Calibrated quantitative claims with specific thresholds

However, the tradeoff:
1. **Formulaic quality**: The temporal progression shows signs of template compliance over
   organic reasoning. Some citation IDs are malformed (e.g., "en-019d94af-019d94af?")
   suggesting the synthesis session was filling structure rather than reasoning from data.
2. **Actionability still abstract**: Despite the mandate to "name specific tools/configs/actions",
   the decision points still trend toward strategic advice. The constraint is in the template
   but the model doesn't generate operational playbooks from foresight data.
3. **Challenge regression**: The diversity mandate may have crowded out the critical/contrarian
   voice. When the template demands 6 themes across 8 findings, there's less room for deep
   counterfactual development.

## Platform Issue: Orchestrator Context Overflow

The orchestrator session crashed at turn 16 due to WASM context overflow (68KB). This is a
platform limitation — the LLM caller WASM cannot parse contexts larger than ~64KB. The probes
completed successfully (75 observations, 18 directions), but synthesis had to be done in a
separate session. This means:
1. The diversity constraints were applied by the synthesis session, not the orchestrator
2. The orchestrator's convergence phase (observation confirmation) completed normally
3. The engine's data-gathering pipeline works; the bottleneck is synthesis context size

## Recommended Changes for Run 004

**Priority 1:** Fix the orchestrator context overflow. Two options:
- (a) Have the orchestrator clear its conversation history before synthesis (if the platform supports context truncation)
- (b) Reduce probes to 2 per step instead of 3, or reduce steps to 1, to keep context smaller

**Priority 2:** Improve Actionability further. The current constraint says "name specific tools"
but the model still generates strategic advice. Try: embed 2-3 example decision points in the
template with concrete operational actions (e.g., "deploy X by Y at cost Z") as few-shot examples.

**Priority 3:** Strengthen Challenge criterion. Add explicit instruction: "At least 1 finding
must directly contradict a claim in the knowledge graph, citing evidence for why the claim fails."

Per meta-loop rules: make ONE targeted change per iteration. Priority 1 is the most impactful
since it removes the synthesis workaround and keeps the engine fully autonomous.

## Structural Insight

Run 003 validates the meta-loop's core thesis: targeted changes to the synthesis template
produce predictable score improvements. The progression from -10 Borda (Run 000) to -5 (Run 001)
to 0 (Run 002) to +4 (Run 003) shows steady convergence toward engine superiority.

The diversity constraints worked exactly as predicted: Breadth improved from 3.0→4.0 engine
Borda (gap halved), Quantitative Precision moved from tie to engine win, and Transparency
strengthened further. The baseline still wins on natural narrative flow (Progression, Challenge)
and practical specificity (Actionability), but the structural advantages now compound.
