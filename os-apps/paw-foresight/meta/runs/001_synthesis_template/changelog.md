# Run 001 Changelog

## Changed File
`os-apps/paw-foresight/system/skills/orchestrate-projection/SKILL.md`

## What Changed
Rewrote the **Final Synthesis** section to add 7 quality mandates that address the 5 weakest engine criteria from Run 000.

### Before
The synthesis template was a bare markdown skeleton with generic placeholders like `[2-3 paragraph synthesis of the most important findings]` and `[Actionable recommendations with timing triggers]`. No instructions for citations, quantitative indicators, named actors, falsification criteria, or structured decision points.

### After
Added 7 numbered quality mandates that the synthesis MUST satisfy:

1. **Transparency**: Every claim must include inline `[obs: OBS_ID]` or `[signal: name]` references
2. **Quantitative Precision**: Every major prediction must include a measurable indicator (%, threshold, timeline, proxy metric)
3. **Specificity**: Must name real companies, tools, projects, dates — not generic categories
4. **Falsifiability**: Top 5 predictions must include explicit falsification conditions with dates and mechanisms
5. **Actionability**: Decision Points must follow trigger → options → tradeoffs structure
6. **Progression**: Temporal Progression expanded to 4 quarterly phases with mandatory "Revisions to earlier predictions" subsections
7. **Completeness**: Added Assumptions & Limitations section with confidence levels and what-would-change-my-mind

Also added:
- Observation index builder (`obs_index`) for citation references
- New "Top 5 Predictions with Falsification Criteria" section
- New "Assumptions & Limitations" section
- Template markers now reference `{obs_index.keys()}` in Methodology

## No Other Changes
- No probe prompt changes
- No entity spec changes
- No WASM changes
- No architecture changes
