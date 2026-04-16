# Run 001 Diagnosis

## Summary

**Engine: 25.4/48 | Baseline: 27.0/48 | Delta: -1.6 | Borda: 51.5 vs 56.5/72**
**Winner: Baseline** (gap narrowed from Run 000's -3.3/-10.0 to -1.6/-5.0)

The synthesis template mandates were added to SKILL.md but the orchestrator agent did NOT follow them. The synthesis output contains no inline observation citations, no quantitative indicators, no named companies/tools, no falsification criteria, and no structured decision points. The gap narrowed slightly due to improved Novelty (+0.7) and Challenge (+0.4) from the probes themselves, not from the synthesis template change.

## Comparison to Run 000

| Criterion | Run 000 Engine | Run 001 Engine | Change | Baseline |
|-----------|---------------|---------------|--------|----------|
| Specificity | 2.0 | 2.0 | 0.0 | 3.0 |
| Novelty | 2.0 | 2.7 | **+0.7** | 2.0 |
| Falsifiability | 2.0 | 2.0 | 0.0 | 2.0 |
| Breadth | 3.0 | 3.0 | 0.0 | 3.0 |
| Plausibility | 2.0 | 2.0 | 0.0 | 2.0 |
| Progression | 2.0 | 2.0 | 0.0 | 2.0 |
| Actionability | 2.0 | 2.0 | 0.0 | 2.7 |
| Decision Clarity | 2.3 | 2.0 | -0.3 | 2.0 |
| Completeness | 2.0 | 2.3 | +0.3 | 2.3 |
| Transparency | 1.0 | 1.7 | **+0.7** | 2.0 |
| Challenge | 2.3 | 2.7 | **+0.4** | 2.0 |
| Quant Precision | 1.0 | 1.0 | 0.0 | 2.0 |
| **Total** | **23.7** | **25.4** | **+1.7** | **27.0** |

## What Improved

- **Novelty (2.0 → 2.7):** 2 of 3 judges gave engine 3. The engine's probes produced original framings ("adjudication capacity as the scarce resource," "governance metabolism of change") that judges recognized as non-obvious. This improvement comes from the probes, not the synthesis template.
- **Challenge (2.3 → 2.7):** 2 of 3 judges gave engine 3. The critic probe's counter-narrative about governance-heavy maintenance plateaus and proxy-driven homeostasis scored well. Again, from probes not template.
- **Transparency (1.0 → 1.7):** Marginal improvement. The synthesis now includes Direction IDs and references the methodology more explicitly, which brought 2 judges from 1 to 2. Still below baseline.
- **Completeness (2.0 → 2.3):** Judge 1 gave engine 3 (noting the multiple active directions create a more complete analytical framework). Marginal.

## What Did NOT Improve (Template Mandates Failed)

### Specificity (Engine: 2.0, Baseline: 3.0) — UNCHANGED

All 3 judges gave engine 2. The synthesis contains ZERO named companies, tools, or specific dates. It uses exclusively generic terms: "vendors," "enterprises," "platform teams," "buyers." The template mandate said "Name real companies... Do NOT use generic categories" — the orchestrator ignored this completely.

**Root cause:** The template mandates are in a `**Quality mandates**` section above the Python template code. The orchestrator LLM reads the SKILL.md as instructions but treats these mandates as suggestions rather than hard requirements. The actual synthesis generation happens in the orchestrator's own reasoning, not by filling in a rigid template.

### Quantitative Precision (Engine: 1.0, Baseline: 2.0) — UNCHANGED

All 3 judges gave engine 1. The synthesis contains no adoption percentages, thresholds, market sizes, or measurable indicators. The template said "Every major prediction must include at least one measurable indicator" — completely ignored.

**Root cause:** Same as Specificity. The orchestrator generates prose from its understanding of the observations, not by mechanically filling in template fields.

### Actionability (Engine: 2.0, Baseline: 2.7) — UNCHANGED

All 3 judges gave engine 2. Decision Points remain flat bullets without timing triggers, options, or tradeoffs. The template mandated trigger → options → tradeoffs structure — not followed.

**Root cause:** Same pattern. The orchestrator produced 4 flat bullet decision points instead of the structured format.

### Falsifiability (Engine: 2.0, Baseline: 2.0) — UNCHANGED

Both tied. No falsification conditions added despite template mandate. The engine didn't include the "Top 5 Predictions with Falsification Criteria" section at all.

## Why the Template Change Failed

The fundamental issue: **SKILL.md template mandates are advisory, not enforced.**

The orchestrator is a GPT-5.4 session that reads the skill file and uses its own judgment about how to produce the synthesis. Adding prose instructions like "Every claim must include an inline [obs: ID] reference" has no enforcement mechanism. The orchestrator:

1. Read the quality mandates (it appeared in its context)
2. Understood them intellectually
3. Chose to produce a coherent narrative instead of mechanically following format requirements
4. Generated a synthesis that matched its own quality judgment, not the template's

This is evident in the output: the synthesis is well-structured and coherent, but it follows the orchestrator's preferred structure (Executive Summary → Key Findings → Active Directions → What Surprised Us → Decision Points → Methodology) rather than the mandated structure (which included Temporal Progression, Top 5 Predictions with Falsification Criteria, and Assumptions & Limitations sections).

## Why the Baseline Still Wins

Same structural advantages as Run 000:
1. **Full context window** — single-shot model uses all tokens for substance, no coordination overhead
2. **Prompt specificity** — the baseline prompt explicitly requested specific entity names, confidence levels, decision frameworks
3. **Single-agent advantage** — one model following a detailed prompt produces more consistent output than a multi-agent pipeline where the final synthesizer may not follow format instructions

## Recommended Changes for Next Iteration

**Priority 1: Move quality mandates from prose instructions INTO the Python template string itself.**

Instead of telling the orchestrator "include observation citations," hardcode the section headers and field markers directly in the template string so the orchestrator MUST fill them in:

```python
synthesis = f"""...
### Key Findings
{for_each_finding}
- **Finding:** [specific claim with named actors]
- **Evidence:** [obs: {obs_id}] — {obs_content[:60]}
- **Indicator:** [quantitative threshold]
{end_for}

### Top 5 Predictions
{for_each_prediction}
1. **Prediction:** {prediction_text}
   - **Measurable indicator:** {indicator}
   - **Falsification:** If {condition} by {date}, wrong because {mechanism}
   - **Supporting:** [obs: {obs_id1}], [obs: {obs_id2}]
{end_for}
...
```

The key insight: the template's Python code block should be the authoritative structure, not prose instructions above it. The orchestrator fills in variables, not generates free-form text.

**Priority 2 (if Priority 1 fails):** Replace the orchestrator's synthesis phase with a dedicated synthesis session that receives observations and directions as structured data and is prompted with an extremely rigid output format with JSON schema validation.

Per meta-loop rules: make ONE targeted change per iteration.
