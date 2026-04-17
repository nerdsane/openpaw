# Run 002 Changelog

## Changed File
`os-apps/paw-foresight/system/skills/orchestrate-projection/SKILL.md`

## What Changed

Two coupled edits to the orchestrator skill:

1. **Step 5 split into 5a (rollup) + 5b (projected state).** The orchestrator is
   now required to write a `step_{step}_rollup.md` file at the end of every step
   (including the final step). The rollup uses a four-section schema — `## New
   predictions this step`, `## Confirmed from prior steps`, `## Revised from prior
   steps`, `## Falsified from prior steps` — with explicit template rows. Step 0
   writes only the first section with the other three marked "None. (no prior
   step to revise.)".

2. **Final Synthesis composes from rollups, does not re-author.** The Temporal
   Progression section of the synthesis is mandated to be assembled by
   concatenating the per-step rollups verbatim under `### Step N (day X of
   {horizon})` sub-headings. The orchestrator instruction reads: "Do NOT
   rewrite them — the rollups ARE the progression record."

## Diff

```diff
 ## Step 5: Write Projected State
+## Step 5: Write Projected State AND the Step Rollup
+
+**You MUST produce TWO artifacts at the end of every step, in this exact order:**
+1. `step_{step}_rollup.md` — a structured per-step narrative rollup (see schema below).
+2. `projected_state_step_{step}.json` — the evolved world state (skipped on the final step).
+
+### Step 5a: Write `step_{step}_rollup.md` (EVERY step)
+
+[...four-section template with explicit per-item rows...]
+
+### Step 5b: Write projected state and dispatch ProjectionUpdated (non-final steps only)
```

```diff
 ## Final Synthesis
+Temporal Progression is COMPOSED from the per-step rollup files you wrote in Step 5 —
+you do NOT author a new phase narrative here.
+
+[...reads step_0_rollup.md, step_1_rollup.md, ..., emits them verbatim under
+### Step N (day X of {horizon}) headings in the Temporal Progression section...]
```

## Rationale

Run 001 Diagnosis Priority 1: "For Progression to move, each step should itself
produce a narrative rollup that the next step explicitly revises, not a single
end-of-loop synthesis." This change implements that priority as a SKILL.md edit:
per-step rollup artifacts + composition (not authoring) of the Temporal
Progression section. Scoped to one file per the "one change per iteration" rule.
