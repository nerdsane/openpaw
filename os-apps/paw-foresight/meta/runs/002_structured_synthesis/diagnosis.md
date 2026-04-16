# Run 002 Diagnosis

## Result

- **Engine average:** 27.0/48 (vs 25.4 Run 001, 27.0 baseline)
- **Baseline average:** 27.0/48
- **Engine Borda:** 54.0/72
- **Baseline Borda:** 54.0/72
- **Winner:** Baseline (tie → incumbent wins by convention)
- **Delta:** 0.0 raw, 0.0 Borda (from -1.6 raw, -5.0 Borda in Run 001)

## What Improved

The data-driven synthesis template and embedded WASM instructions worked. The root cause from
Run 001 (orchestrator never read SKILL.md because it wasn't in TemperFS) was correctly
diagnosed and fixed. The engine output now follows all mandated sections.

Criteria that improved vs Run 001:
- **Decision Clarity:** E=5.5 B=3.5 Borda. Structured trigger/options/tradeoffs format followed.
- **Falsifiability:** E=5.5 B=3.5 Borda. Every prediction has dated falsification conditions.
- **Transparency:** E=5.0 B=4.0 Borda. [obs: ID] citations appear throughout.
- **Novelty:** E=5.0 B=4.0 Borda. Portfolio search + evolutionary memory framing scored well.

These gains match the plan predictions exactly. The structural mandates that were previously
ignored (Decision Clarity, Falsifiability, Transparency) became the engine's strongest wins.

## What Regressed or Failed to Improve

- **Breadth:** E=3.0 B=6.0 Borda. All 3 judges gave baseline 3, engine 2. The engine output
  is deep on one thesis (governed harnesses + portfolio search) but reads as thematically
  narrow. 9 directions all orbit the same core argument.
- **Actionability:** E=3.5 B=5.5 Borda. Despite structured decision points, 2 of 3 judges
  gave baseline higher scores. The decision points name abstract choices rather than giving
  the reader concrete next steps.
- **Specificity:** E=4.0 B=5.0 Borda. J1 scored engine 2 vs baseline 3. The engine names
  companies but repeats the same small set (Anthropic, OpenAI, Cursor, Temper, Cedar).
- **Quantitative Precision:** E=4.5 B=4.5 Borda. All judges gave both outputs 2. The
  mandatory "Measurable indicator" field is present but values are generic (35%, 2x, etc.)
  rather than calibrated from data.

## Root Cause Analysis

**The template enforces structure but does not enforce content diversity or calibration.**

The data-driven synthesis construction correctly forces the orchestrator to include citations,
falsification criteria, decision point structure, and temporal phases. But the orchestrator
still controls:
1. **Which observations to highlight** — it picked the same observations repeatedly, resulting
   in 8 findings and 9 directions that all argue the same thesis
2. **How to fill qualitative fields** — "Measurable indicator: 35%+ of tickets" is structurally
   compliant but not analytically calibrated
3. **Specificity depth** — naming 6 companies across 35KB reads as repetitive rather than specific

The structural mandates are now working. The content quality mandates are the remaining gap.

## Recommended Change for Run 003

**Target:** Breadth and Actionability (the two criteria where engine lost most Borda points).

**Proposed change:** Add a **diversity constraint** to the synthesis template:
1. **Observation deduplication:** Require the synthesis to use at least N distinct observations
   (not repeat the same ones). Add a validation step that counts unique obs IDs per section.
2. **Finding diversity mandate:** Require Key Findings to span at least 4 distinct themes
   (e.g., cannot all be about "harness bundles"). Define theme categories from the direction
   titles.
3. **Actionability specificity:** Decision Points must name a specific tool, configuration,
   or organizational action — not abstract choices like "invest in harnesses vs model access."
4. **Cross-domain requirement:** At least 2 findings must come from the adjacent-domain probe,
   not just practitioner/critic probes.

These constraints attack the breadth and actionability gap without changing the structural
template that now correctly produces citations, falsification criteria, and decision structure.

## Alternative Hypotheses

1. **Judge prompt truncation bias:** The engine synthesis was trimmed from 37KB to 31KB for
   the judge prompt (Active Directions reasoning removed). This may have hurt Breadth scores
   by hiding content. Counter: Breadth should be visible in Key Findings and Temporal
   Progression, not just in directions.
2. **Baseline is near-optimal for this rubric:** The baseline averages exactly 27.0/48 with
   a naturally balanced profile. It may be hard to beat a balanced output with a structurally
   stronger but thematically narrower one. Counter: the engine matched on raw score, just
   not on Borda distribution.
3. **3+ cap compression:** With exactly 3 criteria allowed at 3+, the engine's structural
   wins (Decision Clarity, Falsifiability) compete with the baseline's traditional strengths
   (Breadth, Specificity, Actionability). The cap forces a zero-sum tradeoff.

## Structural Insight

Run 002 validates the meta-loop hypothesis: the synthesis template change produced exactly
the predicted improvements in the predicted criteria. The gap has closed from -10 Borda
(Run 000) to -5 (Run 001) to 0 (Run 002). The next iteration needs to shift from
structural compliance to content quality: diversity, calibration, and specificity depth.
