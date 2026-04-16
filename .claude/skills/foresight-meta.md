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

Use curl for all Temper API interactions. The API key is at `~/.local/share/openpaw/api.key`.
Headers: `Authorization: Bearer <key>`, `x-temper-tenant: rita-agents`.

1. Ensure the server is running on port 3467: `curl http://localhost:3467/tdata/ForesightModels`
2. Check the ForesightModel entity is Active with the DSE knowledge graph
   - If no ForesightModel exists (server was restarted), recreate it:
     a. `POST /tdata/ForesightModels` with `{"Name": "Directed Software Evolution v2", "ModelType": "knowledge_domain"}`
     b. Seed it with the knowledge graph (check `meta/runs/000_initial/engine-output/` for reference)
   - The knowledge graph content persists in the database blobs table even if entities are cleared
3. Create a new Projection entity: `POST /tdata/Projections`
4. Configure it: `POST /tdata/Projections('{id}')/Temper.Configure` with:
   - foresight_model_id: the DSE model ID
   - horizon: "1 year"
   - (Do NOT prescribe steps, probes, or schedule — the engine decides)
5. Start the projection: `POST /tdata/Projections('{id}')/Temper.Start`
6. Poll until Complete or Failed: `GET /tdata/Projections('{id}')` in a loop (every 30s, max 15min)
7. Save all artifacts to `meta/runs/{NNN}/engine-output/`:
   - synthesis.md (from the synthesis session's result or projected state)
   - observations: `GET /tdata/Observations?$filter=ProjectionId eq '{id}'`
   - directions: `GET /tdata/Directions?$filter=ProjectionId eq '{id}'`
   - projected state files
   - event trail
   - Session transcripts: extract from `~/.local/share/openpaw/paw.db` using sqlite3
     (see `meta/runs/000_initial/transcripts/MANIFEST.md` for the extraction pattern)

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

### Current Method: Meta-Agent Scoring

Automated 3-judge sessions failed in Run 000 (file delivery bug + processing timeouts).
Until fixed, the meta-agent scores both outputs directly.

Score BOTH the challenger (engine output from this run) and the incumbent
(baseline at `meta/baseline/synthesis.md`, or the previous winner if engine has won before).

For each of the 12 criteria in program.md:
1. Read the criterion anchors carefully
2. Apply the calibration: 2 = competent, 3 = genuinely impressive, 4 = exceptional
3. Score both outputs independently
4. Write reasoning and evidence for each score

### Compute Borda

1. Per criterion: higher score = Rank 1 (2 Borda points), lower = Rank 2 (1 point)
2. If scores tied on a criterion: each gets 1.5 points
3. Sum across 12 criteria (max 24 Borda points per output for 1 judge)
4. If overall tied: incumbent wins (conservative)
5. Save raw scores to `meta/runs/{NNN}/scores.json`
6. Save Borda aggregation to `meta/runs/{NNN}/borda.json`

### Future: Automated Judges

When paw-agent file delivery is fixed, switch to 3 independent blind judge sessions.
See program.md Judge Protocol for the full specification.

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
