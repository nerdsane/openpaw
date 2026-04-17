# Run 002 Plan — Structured per-step rollups feed Final Synthesis

## Scope
- os-apps/paw-foresight/system/skills/orchestrate-projection/SKILL.md
- os-apps/paw-foresight/meta/progress.md

## Target Criteria

Run 001 tied 54/54 Borda against Run 000 incumbent. Per-criterion deltas:
- **Progression: −1** (biggest regression target) — judges called temporal revisions "cosmetic"; both outputs shared the same LLM-authored 4-phase structure.
- **Specificity: −1**, **Actionability: −1** — step-1 probes didn't propagate the incumbent's quantitative/dollar anchors.
- Gains: **Novelty +2**, **Challenge +1** from step-1 external signals (METR, OWASP, two-lane model).

Net zero → tie → incumbent wins. Progression is the highest-leverage criterion that didn't move despite the architecture ran two steps for the first time.

## Root cause (from Run 001 diagnosis)

> "The orchestrator's Final Synthesis section (in `orchestrate-projection/SKILL.md`) authors the 3–6/6–9/9–12 month phases in one pass regardless of how many steps ran. The step-1 observations enrich the evidence pool but the phase structure is LLM-authored from scratch at the end. For Progression to move, each step should itself produce a narrative rollup that the next step explicitly revises, not a single end-of-loop synthesis."

The AdvanceStep guard (Run 001) made multi-step physically happen. But the synthesis still collapses everything to one authored pass — step memory is flattened. Judges cannot *see* progression because there is no per-step artifact to compare.

## Planned Change

**One file**: `os-apps/paw-foresight/system/skills/orchestrate-projection/SKILL.md`

Two coupled edits, both to the orchestrate-projection skill:

1. **Step 5 (Write Projected State) — add rollup artifact.** After writing `projected_state_step_{step}.json` and before dispatching `ProjectionUpdated`, the orchestrator MUST also write a `step_{step}_rollup.md` file with exactly four sections:
   - `## New predictions this step` — theses that emerged in step N's observations/directions and were not present in step N-1's rollup.
   - `## Confirmed from prior steps` — prior predictions whose evidence strengthened this step; each item quotes the prior rollup's prediction and cites the new supporting observation.
   - `## Revised from prior steps` — prior predictions whose wording, scope, or threshold changed this step; each item quotes the prior version, states the new version, and names the mechanism that forced the revision.
   - `## Falsified from prior steps` — prior predictions that this step's evidence breaks; each item quotes the original and cites the falsifying observation or external signal.
   - Step 0's rollup has only the first section (no prior steps).

2. **Final Synthesis — compose Temporal Progression from rollups, do not re-author.** The "Temporal Progression" section of the synthesis MUST be assembled by reading each `step_{step}_rollup.md` in order and including its four sections verbatim under a step-scoped heading (`### Step 0 (day {days_offset})`, `### Step 1 (day {days_offset})`, …). The orchestrator does not generate new phase prose here; it aggregates.

The rollup files are written to the orchestrator's workspace so verification tooling and judges can inspect the per-step chain of revisions independently of the final synthesis prose.

## Why this is architectural, not cosmetic

- Introduces a new artifact type (`step_{N}_rollup.md`) in a fixed location with a fixed four-section schema. Downstream consumers (judges, future WASM) can trust the shape.
- Changes the data flow: Final Synthesis becomes a *compose* step over step-scoped rollups, not a single authored pass. This moves the locus of "what changed between steps" from LLM-recalled prose to explicit, step-local artifacts.
- Makes Progression **verifiable from artifacts**: any reader can diff `step_0_rollup.md` against `step_1_rollup.md` to see the revision chain. Run 001's synthesis had "Revisions to earlier predictions" sub-bullets but nothing to diff against.

## Expected Impact

- **Progression (target +1 to +2 Borda)**: each step produces a structured rollup the next step explicitly revises; the Temporal Progression section reads as a composition of step-local claims rather than a single-pass authorial narrative.
- **Falsifiability (possible +)**: the "Falsified from prior steps" section is a forcing function — step-N probes must actively break or preserve prior claims, not just extend them.
- **Information Density (neutral to +)**: quoting prior predictions in Confirmed/Revised/Falsified gives every step-N claim an explicit antecedent, reducing the "each step adds more paragraphs with overlapping content" failure mode.
- **Specificity / Actionability**: unaddressed by this change. Deferred to Run 003 if Run 002 wins. (Per "one change per iteration" rule.)

## Non-goals / explicitly deferred

- No spec invariants added. No WASM recompile. No Cedar policy change. Scope stays in one SKILL.md file + progress.md to keep the blast radius small and the change observable.
- No quantitative-threshold invariant on Direction.Propose (Run 001 diagnosis Priority 2). Deferred to a follow-up iteration so Progression moves independently.
- No ProbeStepDone dedup (Run 001 diagnosis Priority 3). Audit noise only; not scored.

## Verification plan

1. Install the reloaded app via `POST /api/os-apps/paw-foresight/install`.
2. Run a fresh projection against the existing DSE v2 ForesightModel.
3. Confirm `step_0_rollup.md` and `step_1_rollup.md` files exist in the orchestrator workspace after Complete.
4. Confirm the final synthesis's Temporal Progression section contains headings of the form `### Step N (day X)` with the four sub-sections quoted from the rollup files.
5. Score challenger vs Run 001 incumbent with 3 blind Claude Code subagent judges using the locked template.
6. Record scores, compute Borda via the tool, write diagnosis.
