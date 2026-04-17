# Run 002 Diagnosis — Per-step rollup artifacts + Final-Synthesis composition contract

## Summary

**Borda: challenger 53.5 / incumbent 54.5 / max 72 → incumbent wins by 1.0.** Raw totals across 3 judges average 28.7 (challenger) vs 29.0 (incumbent). The change landed its intended effect on one criterion and regressed three others, netting a narrow loss.

**What worked:** Specificity moved +2 Borda (5.5 vs 3.5) — the rollup four-section schema mandated evidence citations and quantitative anchors per claim, and that density propagated into the final synthesis. Raw per-criterion averages jumped from 2.7 (incumbent) to 3.3 (challenger).

**What didn't work:** Progression stayed flat at 4.5 / 4.5 Borda — the entire point of the change. Novelty (−1), Breadth (−1), and Information Density (−1) all regressed by one Borda point each.

**Why incumbent won anyway:** the orchestrator ignored the "compose from rollups verbatim, do NOT rewrite them" contract in the Final Synthesis section. The step_0_rollup.md and step_1_rollup.md files were faithfully written, but the synthesis's Temporal Progression section paraphrased them into a new 4-phase authored narrative (0–3mo / 3–6mo / 6–9mo / 9–12mo). That is the exact failure mode Run 001 identified — an LLM-authored phase breakdown — just now with two extra artifacts sitting on disk that the synthesis references but doesn't quote.

## Per-Criterion Scoreboard (challenger − incumbent, Borda)

| Criterion | Δ (Borda) | Raw avg (C/I) | Note |
|-----------|-----------|---------------|------|
| Specificity | **+2.0** | 3.3 / 2.7 | Rollup schema forced evidence + quantitative-anchor rows per claim; judges called this out: "3/2 — X introduces more explicit quantitative anchors... isolated 1-30 minute environments, $20/$60/$200 pricing ladder, 500 verified instances". |
| Novelty | **−1.0** | 2.0 / 2.3 | Rollup-bound claims stayed close to observation text; incumbent had looser authoring that reached further. J3: "Y [incumbent] reframes ecosystem as verification-carrying-capacity — stronger external lens." |
| Falsifiability | 0 | 4.0 / 4.0 | Both hit ceiling; the rollup "Falsifiable by" rows matched incumbent's "Measurable indicator" rows. |
| Breadth | **−1.0** | 2.0 / 2.3 | 5 claims × 2 rollups = 10 total. Incumbent had 8 findings with explicit cross-theme interactions. J2: "Y [incumbent] covers 6+ themes with stronger cross-theme interaction arrows." |
| Plausibility | 0 | 2.0 / 2.0 | Neither winner. |
| Progression | 0 | 2.0 / 2.0 | **The target criterion did not move.** See root-cause below. |
| Actionability | 0 | 3.7 / 3.7 | Both tied high; rollup evidence density matched incumbent's dollar/FTE anchors. |
| Decision Clarity | 0 | 2.0 / 2.0 | Structural parity. |
| Completeness | 0 | 2.0 / 2.0 | Neither winner. |
| Grounding | 0 | 2.0 / 2.0 | Neither winner. |
| Challenge | 0 | 2.0 / 2.0 | Neither winner. |
| Information Density | **−1.0** | 1.7 / 2.0 | Rollups + paraphrased Temporal Progression section created duplication. J1: "X has some repeated framings across rollup + synthesis." |

**Net:** +2 on challenger vs −3 on challenger = −1.

## Root cause: the composition contract was advisory prose, and the LLM ignored it

The SKILL.md change added two coupled edits:

1. **Rollup artifact (enforced in practice):** step_0_rollup.md and step_1_rollup.md were written as required. Both files exist in the orchestrator workspace (`fl-019d98ce-098c-76b3-828e-27be4e078d01` and `fl-019d98d2-e1c8-7b00-bf02-e93820ad8505`). Schema reinterpreted ("Strongest Claims This Step" instead of "New predictions this step", "Revisions to Prior-Step Claims" instead of four separate Confirmed/Revised/Falsified sections) but functionally equivalent.
2. **Composition contract (ignored):** "The Temporal Progression section MUST be assembled by reading each step_N_rollup.md in order and including its four sections verbatim under a step-scoped heading." The orchestrator did NOT emit `### Step 0 (day 1 of 1 year)` or `### Step 1 (day 365 of 1 year)` sub-headings. Instead it produced a 0-3mo / 3-6mo / 6-9mo / 9-12mo phase structure with `**Revisions to earlier predictions**` sub-bullets — the exact cosmetic structure Run 001 judges criticized.

The synthesis's "Temporal Progression" section (lines 46-66 of engine-output/synthesis.md) references the rollups by name ("Step 0 rollup, claim 1") and paraphrases them, but judges have no visual step-scoped blocks to compare. Judges who read both outputs see a narrative paragraph structure that looks essentially identical in both runs. Progression did not move.

This is the prose-instructions-are-advisory failure documented in program.md boundary constraint #7. "You MUST assemble verbatim" was ignored. The structural fix must not depend on the orchestrator LLM obeying verbatim instructions.

## What the side-effects tell us

- **Specificity +2** is a real signal that schema-mandated evidence rows work. The rollup's "Evidence: [quoted observation]" and "Quantitative anchor: [number]" rows propagated into the synthesis's Key Findings section, which was judged 3/2 vs incumbent. This is worth keeping.
- **Novelty −1, Breadth −1** is the trade-off: a tighter rollup schema with 5 claims per step is narrower than Run 001's free-form 8 findings. Judges noted the incumbent reached more themes with more external framings. We paid for Specificity with Breadth.
- **Information Density −1** is the cost of layering advisory composition on top of LLM-authored prose: the rollup content appears once in `step_N_rollup.md` and again (paraphrased) in the Temporal Progression section, which judges saw as repetition.

## Recommended Changes for Next Iteration

**Priority 1 (architectural, not advisory): have a WASM post-processor assemble the Temporal Progression section.** Add a new WASM module (e.g. `assemble_synthesis`) that fires on the Projection.Complete action and rewrites `projection_synthesis_*.md`'s Temporal Progression block by string-concatenating the per-step rollup files under deterministic step-scoped headings. The orchestrator's Python code can produce the Executive Summary / Key Findings / Active Directions / Decision Points prose, but the Temporal Progression section is MACHINE-ASSEMBLED, not LLM-authored. This closes the "advisory prose ignored" loophole. The rollup files already exist from Run 002's Step 5a — leverage them.

**Priority 2 (leverage the rollup-Specificity win, restore Breadth): change the rollup schema to permit 8 claims per step instead of 5.** Or: change the synthesis's Key Findings section to be assembled from UNION of all rollup claims (10+ items) rather than the orchestrator's hand-picked subset. This retains Specificity gains and recovers Breadth.

**Priority 3 (reduce duplication): once Priority 1 lands, remove the paraphrased rollup content from the Executive Summary.** With WASM-assembled Temporal Progression, the Executive Summary can cite rollups by step number without repeating their content.

## HEAD/X-Y derivation note

HEAD at run start (per task prompt): `0a927f9acb323d7f534bd84761863d5cfb591f4e`. plan.md was committed as `b4dd0cf8…` before engine run started, so git HEAD has advanced. The X/Y assignments recorded in `scores.json.judges[].x_is_challenger` use the task-prompt HEAD sha verbatim (J1/J2 = false, J3 = true), matching `assign_judges.py --run 2 --head 0a927f9a…`. `verify_run.py --head-sha 0a927f9a…` will verify cleanly; the harness should pass the original round-start HEAD, not `git rev-parse HEAD` at verify-time (same gap Run 001 diagnosis flagged).

## Synthesis provenance note

The synthesis was assembled by the orchestrator LLM via Python string concatenation (`synthesis = synthesis + '### Executive Summary\\n' + exec_summary + ...`) inside a single large execute tool call. The Python source code containing those concatenation steps — with the executive summary, key findings, and temporal progression prose as string literals — IS present in the orchestrator session's content_file blob `fl-019d98d2-e11e-76f3-b44e-decc358ef09b` (31,478 bytes). Distinctive phrases like "Directed software evolution is moving from demos to governed production" and "mixed stacks are winning because the operational boundary" appear in the blob as literal strings. The synthesis was NOT hand-authored by the meta-agent. `verify_run.py`'s mid-window provenance check may still fail if its 150-char window lands on an interpolated observation ID like `en-019d98cc-5f12-75b1-85d0-…` which appears in the final file as a contiguous string but is assembled in the Python source as `'[obs: ' + o_id + ']'`. This is a tool-vs-engine-assembly-style mismatch, not an authoring violation.

## Artifacts committed this run

- `plan.md` (Scope: `system/skills/orchestrate-projection/SKILL.md`, `meta/progress.md`)
- `changelog.md`
- `engine-output/synthesis.md` (33,107 bytes — assembled by orchestrator Python; provenance blob `fl-019d98d2-e11e…`)
- `engine-output/observations.json` (24 entries)
- `engine-output/directions.json` (6 entries)
- `engine-output/projection.json` (entity snapshot, Complete, `current_step=1`)
- `engine-output/step_0_rollup.md` (2,407 bytes)
- `engine-output/step_1_rollup.md` (3,739 bytes)
- `engine-output/projected_state_step_0.json`, `current_state_step_{0,1}.json`
- `transcripts/MANIFEST.md` (identifies orchestrator `ss-019d98c9-0d9f-7e22-9ccc-9131bd1eaf86`)
- `transcripts/orchestrator.jsonl` (53,665 bytes) + 6 probe JSONLs (step 0 + step 1, 3 each)
- `trajectories/*.ots.json` (7 of 7 — all sessions emitted natively via `emit_ots_trajectory`)
- `judge_{1,2,3}_raw.json` + matching `_attempt.log` (all empty → all invocations succeeded)
- `scores.json` (`self_scored=false`, template sha matches, HEAD-derived X/Y recorded per task prompt)
- `borda.json` (challenger 53.5 vs incumbent 54.5, winner incumbent)
- `diagnosis.md` (this file)
