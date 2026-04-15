---
name: foresight-meta
description: Run one iteration of the paw-foresight meta-improvement loop
user_invocable: true
---

# Foresight Meta-Improvement Loop

Run one iteration of the meta-improvement loop for paw-foresight.
This skill codifies the meta-agent process — what I (Claude Code) do each round.

## Before Starting

1. Read `os-apps/paw-foresight/meta/program.md` — the immutable rubric
2. Read `os-apps/paw-foresight/meta/progress.md` — the score history
3. Read `os-apps/paw-foresight/meta/DESIGN.md` — the full system design
4. If this is run 000: run baseline + engine, score both, record. No modification.
5. If this is run N>0: read previous `meta/runs/{N-1}/diagnosis.md` for what to fix

## Step 1: Determine Run Number

```
N = number of existing directories in meta/runs/ 
Run directory: meta/runs/{NNN}_{short_description}/
```

## Step 2: Plan the Change (skip for run 000)

- Read the previous diagnosis — which criterion scored lowest?
- Read raw session transcripts from the previous engine run (not summaries)
- Identify the root cause in the engine (skill text? entity design? agent count? prompt?)
- Plan ONE targeted change. Write it down BEFORE implementing.
- Save plan to `meta/runs/{NNN}/plan.md`

## Step 3: Implement the Change (skip for run 000)

- Make the change to the engine (skill, spec, WASM, architecture)
- If WASM changed: recompile, reinstall
- If skill changed: reinstall app or PATCH the file entity
- Document exactly what changed in `meta/runs/{NNN}/changelog.md`

## Step 4: Run the Engine

1. Ensure the server is running (openpaw-server on port 3467)
2. Check the ForesightModel entity is Active with the DSE knowledge graph
3. Create a new Projection entity
4. Configure it:
   - foresight_model_id: the DSE model
   - horizon: "1 year"
   - (Do NOT prescribe steps, probes, or schedule — the engine decides)
5. Start the projection
6. Monitor until Complete or Failed
7. Save all artifacts to `meta/runs/{NNN}/engine-output/`:
   - synthesis.md
   - observations (by step)
   - directions (by step)
   - projected state files
   - event trail
   - orchestrator session transcript

## Step 5: Run the Baseline (only for run 000)

1. Create a paw-agent session manually
2. Give it the same knowledge graph file reference
3. Prompt: "You are a foresight analyst. Given this knowledge graph about
   Directed Software Evolution, produce a structured foresight projection
   covering the next 1 year. Include: executive summary, key findings,
   temporal progression, decision points, and confidence levels."
4. Save output to `meta/baseline/`
5. The baseline is fixed — never re-run unless the domain changes

## Step 6: Judge

### Setup Judge Sessions

Create 3 independent paw-agent sessions. Each receives:

1. The rubric from program.md (12 criteria with anchors)
2. Two outputs, anonymized as "Output X" and "Output Y"
3. Randomize which is X and which is Y (flip per judge or per pair)
4. The judge prompt (see below)

### Judge Prompt Template

```
You are an independent evaluator of foresight projections.

Below are two projections about the same domain. Score each against the rubric.

For each criterion (12 total), for each output (X and Y), provide:
- Score (0-4, using the anchors below)
- Reasoning (1-2 sentences explaining the score)
- Evidence (specific quotes or references from the output)

Output your scores as a JSON array.

## Rubric
{paste 12 criteria with anchors from program.md}

## Output X
{anonymized output}

## Output Y
{anonymized output}
```

### Aggregate Scores

1. Collect 3 judge responses
2. Per criterion: rank X vs Y by score. Rank 1 = 2 Borda points, Rank 2 = 1 point.
3. If scores tied on a criterion: each gets 1.5 points.
4. Sum across 3 judges per criterion (max 6 points per criterion per output)
5. Sum across 12 criteria (max 72 Borda points per output)
6. If overall tied: incumbent (A) wins
7. Save raw scores to `meta/runs/{NNN}/scores.json`
8. Save Borda aggregation to `meta/runs/{NNN}/borda.json`

## Step 7: Record Results

### scores.json format
```json
{
  "run": "NNN",
  "engine_version": "foresight-v{NNN}",
  "incumbent": "A" | "B",
  "challenger": "A" | "B",
  "randomization": {"judge_1": {"X": "engine", "Y": "incumbent"}, ...},
  "judges": [
    {
      "judge_id": "ss-...",
      "scores": [
        {"criterion": "Specificity", "output_x": 2, "output_y": 3, "reasoning_x": "...", "reasoning_y": "...", "evidence_x": "...", "evidence_y": "..."},
        ...
      ]
    }
  ]
}
```

### borda.json format
```json
{
  "run": "NNN",
  "engine_borda": 42,
  "incumbent_borda": 38,
  "baseline_borda": 35,
  "winner": "challenger",
  "per_criterion": [
    {"criterion": "Specificity", "engine": 4.5, "incumbent": 1.5, "delta": 3},
    ...
  ]
}
```

### Update progress.md

Add a row to the score table with: run, tag, engine score, baseline score,
delta, winner, streak count, key change description, key insight.

### Diagnosis

Write `meta/runs/{NNN}/diagnosis.md`:
- Which criterion scored lowest?
- Why? (cite specific evidence from the output and session transcripts)
- What part of the engine is responsible?
- What would need to change?

## Step 8: Decide

- If challenger wins: tag `foresight-v{NNN}`, update incumbent, reset streak to 0
- If incumbent wins: revert the change, increment streak
- If streak reaches 2: convergence — report final results and stop

## Step 9: Commit and Document

- Git commit all meta/ changes
- Git tag if new version
- Update progress.md convergence status
- Push

## Key Reminders

- Read RAW transcripts for diagnosis, not summaries. The meta-harness insight:
  full agent traces reveal what went wrong, not the final output.
- ONE change per iteration. Not two, not "a few related changes." One.
- The rubric measures output quality. Don't optimize for process aesthetics.
- Simpler wins at equal scores. If you can remove a component and maintain
  scores, remove it.
- The baseline is your reality check. If the engine can't beat a single prompt,
  the engine needs fundamental rethinking, not tuning.
