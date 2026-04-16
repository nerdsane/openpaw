---
name: foresight-meta
description: Run one iteration of the paw-foresight meta-improvement loop
user_invocable: true
---

# Foresight Meta-Improvement Loop

Run one iteration of the meta-improvement loop for paw-foresight.
Each invocation is a fresh session — read all state from files, leave all state in files.

## Before Starting — Read These Files

1. `os-apps/paw-foresight/meta/program.md` — the immutable rubric (12 criteria, 0-4 anchors)
2. `os-apps/paw-foresight/meta/progress.md` — score history, convergence status
3. `os-apps/paw-foresight/meta/DESIGN.md` — system architecture
4. The most recent `meta/runs/{N-1}/diagnosis.md` — what to fix
5. The most recent `meta/runs/{N-1}/transcripts/` — raw agent traces (read at least orchestrator.jsonl)
6. `os-apps/paw-foresight/meta/baseline/synthesis.md` — the incumbent output to beat
7. `os-apps/paw-foresight/meta/baseline/prompt.md` — what the baseline prompt looked like (learn from it)

## Step 1: Determine Run Number

```bash
N=$(ls -d os-apps/paw-foresight/meta/runs/*/ 2>/dev/null | wc -l | tr -d ' ')
# Run directory: meta/runs/{NNN}_{short_description}/
# Example: meta/runs/001_synthesis_template/
```

Create the run directory immediately:
```bash
mkdir -p os-apps/paw-foresight/meta/runs/$(printf '%03d' $N)_<description>/engine-output
mkdir -p os-apps/paw-foresight/meta/runs/$(printf '%03d' $N)_<description>/transcripts
```

## Step 2: Plan the Change

Read the previous diagnosis. Identify:
- Which criteria scored 0-1? (highest leverage targets)
- What's the root cause? (cite specific lines from transcripts)
- What ONE change addresses the most criteria?

Write the plan to `meta/runs/{NNN}/plan.md` BEFORE implementing:
```markdown
# Run {NNN} Plan

## Target Criteria
- {criterion}: engine scored {X}, root cause: {specific component}

## Planned Change
{what will change, what file, what the change does}

## Expected Impact
{which scores should improve and why}
```

## Step 3: Implement the Change

The engine is at `os-apps/paw-foresight/`. Key files:

| File | What it controls |
|------|-----------------|
| `system/skills/orchestrate-projection/SKILL.md` | Orchestrator behavior, probe prompts, synthesis template |
| `specs/projection.ioa.toml` | Projection entity states, WASM triggers |
| `specs/observation.ioa.toml` | Observation entity, two-gate confirmation |
| `specs/direction.ioa.toml` | Direction entity |
| `wasm/spawn_orchestrator/src/lib.rs` | WASM that creates the orchestrator session |

Most changes will be to the **orchestration skill** (SKILL.md). This controls:
- How probes are prompted (persona, instructions, what to include)
- How many probes and steps
- How convergence analysis works
- How the final synthesis is structured (this is where most Run 000 weaknesses live)

After making changes:
- If WASM changed: `cd os-apps/paw-foresight/wasm/spawn_orchestrator && cargo build --target wasm32-unknown-unknown --release && cp target/wasm32-unknown-unknown/release/spawn_orchestrator.wasm .`
- Reinstall the app: use curl to POST to the app install endpoint, or restart the server

Save `meta/runs/{NNN}/changelog.md`:
```markdown
# Run {NNN} Changelog

## Changed File
`os-apps/paw-foresight/system/skills/orchestrate-projection/SKILL.md`

## What Changed
{description with before/after}

## Diff
{paste the relevant diff or summarize the key lines changed}
```

## Step 4: Run the Engine

### API Setup

```bash
API_KEY=$(cat ~/.local/share/openpaw/api.key)
AUTH="Authorization: Bearer $API_KEY"
TENANT="x-temper-tenant: rita-agents"
BASE="http://localhost:3467"
```

### Check Server Health

```bash
curl -s "$BASE/tdata/ForesightModels" -H "$AUTH" -H "$TENANT"
```

If HTTP 401/404: the server needs the API key auth. If connection refused: server is down.

### Check/Create ForesightModel

```bash
# List existing models
curl -s "$BASE/tdata/ForesightModels" -H "$AUTH" -H "$TENANT"
```

If the DSE model doesn't exist (server was restarted), you need to recreate it.
The knowledge graph is a large JSON file. Check if it's still in the blobs table:
```bash
sqlite3 ~/.local/share/openpaw/paw.db "SELECT blob_key, size_bytes FROM blobs WHERE size_bytes > 500000 ORDER BY size_bytes DESC LIMIT 5;"
```
The knowledge graph blob is ~720KB. If found, you can create a new ForesightModel and
attach it. If not found, the original essay source file may need to be re-ingested.

### Create and Run Projection

```bash
# Create projection
PROJ=$(curl -s -X POST "$BASE/tdata/Projections" \
  -H "$AUTH" -H "$TENANT" -H "Content-Type: application/json" \
  -d '{}')
PROJ_ID=$(echo "$PROJ" | python3 -c "import sys,json; print(json.load(sys.stdin)['entity_id'])")

# Configure
curl -s -X POST "$BASE/tdata/Projections('$PROJ_ID')/Temper.Configure" \
  -H "$AUTH" -H "$TENANT" -H "Content-Type: application/json" \
  -d "{\"foresight_model_id\": \"$MODEL_ID\", \"horizon\": \"1 year\"}"

# Start
curl -s -X POST "$BASE/tdata/Projections('$PROJ_ID')/Temper.Start" \
  -H "$AUTH" -H "$TENANT" -H "Content-Type: application/json" -d '{}'
```

### Poll for Completion

```bash
# Poll every 30 seconds, max 20 minutes
for i in $(seq 1 40); do
  STATUS=$(curl -s "$BASE/tdata/Projections('$PROJ_ID')" -H "$AUTH" -H "$TENANT" \
    | python3 -c "import sys,json; print(json.load(sys.stdin).get('status','?'))")
  echo "[$i] Projection status: $STATUS"
  [ "$STATUS" = "Completed" ] && break
  [ "$STATUS" = "Failed" ] && echo "FAILED" && break
  sleep 30
done
```

### Extract Artifacts

```bash
RUNDIR="os-apps/paw-foresight/meta/runs/{NNN}/engine-output"

# Observations
curl -s "$BASE/tdata/Observations" -H "$AUTH" -H "$TENANT" \
  | python3 -c "import sys,json; ..." > "$RUNDIR/observations.json"

# Directions
curl -s "$BASE/tdata/Directions" -H "$AUTH" -H "$TENANT" \
  | python3 -c "import sys,json; ..." > "$RUNDIR/directions.json"

# Synthesis — find in the orchestrator session's workspace files or projected state
```

### Extract Session Transcripts

This is CRITICAL for diagnosis. The transcripts are in the SQLite database.

```python
import subprocess, json, os

db = os.path.expanduser("~/.local/share/openpaw/paw.db")

def sql(query):
    r = subprocess.run(["sqlite3", db, query], capture_output=True, text=True)
    return r.stdout.strip()

# Find sessions created after the projection started (by entity_id timestamp prefix)
# The projection ID prefix gives the approximate time
proj_prefix = "PROJ_ID_PREFIX"  # e.g., "019d9350"

# List all sessions in the timeframe
sessions_raw = sql(f"""
    SELECT entity_id, 
           json_extract(snapshot, '$.status') as status,
           json_extract(snapshot, '$.counters.turn_count') as turns,
           json_extract(snapshot, '$.fields.model') as model,
           json_extract(snapshot, '$.fields.session_file_id') as file_id
    FROM snapshots 
    WHERE entity_type = 'Session' 
      AND entity_id > 'ss-{proj_prefix}'
      AND entity_id < 'ss-{next_prefix}'
    ORDER BY entity_id;
""")

# For each session, extract the JSONL transcript:
# 1. Get the session_file_id from the session snapshot
# 2. Get the content_hash from the file entity snapshot  
# 3. Get the blob data using the content_hash as blob_key

for session_id, name in sessions.items():
    file_id = sql(f"SELECT json_extract(snapshot, '$.fields.session_file_id') FROM snapshots WHERE entity_id = '{session_id}';")
    content_hash = sql(f"SELECT json_extract(snapshot, '$.fields.content_hash') FROM snapshots WHERE entity_id = '{file_id}';")
    blob_key = f"temper-fs/{content_hash}"
    data = sql(f"SELECT data FROM blobs WHERE blob_key = '{blob_key}';")
    with open(f"transcripts/{name}.jsonl", 'w') as f:
        f.write(data)
```

Write a MANIFEST.md in the transcripts/ directory listing each session and its role.

## Step 5: Score — This is Where Rigor Matters

Read BOTH outputs completely:
- Challenger: `meta/runs/{NNN}/engine-output/synthesis.md`
- Incumbent: `meta/baseline/synthesis.md` (or previous winner)

Read `meta/program.md` for the 12 criteria with anchors.

### Calibration (from program.md)
- **2 = competent** — most outputs should land here
- **3 = genuinely impressive** — most outputs will NOT reach this
- **4 = exceptional** — requires something a well-prompted single model would rarely produce

### Score Each Criterion

For EACH of the 12 criteria, for EACH output, write:
- **Score** (0-4, justified by the anchor definitions)
- **Reasoning** (2-3 sentences explaining WHY this score, not higher or lower)
- **Evidence** (specific quotes or structural observations from the output)

### scores.json Format

```json
{
  "run": "NNN",
  "engine_version": "foresight-v{NNN}",
  "rubric_version": "v2",
  "methodology_note": "Meta-agent (Claude Code) single evaluator. Calibration: 2=competent, 3=impressive, 4=exceptional.",
  "scores": [
    {
      "criterion": "Specificity",
      "engine_score": 2,
      "baseline_score": 2,
      "engine_reasoning": "Has approximate timelines (90 days, 1 year) but no named companies...",
      "baseline_reasoning": "Names specific companies (Anthropic, OpenAI, ...) and has phased timelines...",
      "engine_evidence": "\"By 90 days... By 1 year\"; mechanisms: \"incident-to-eval loops\"",
      "baseline_evidence": "\"Anthropic, OpenAI, Cursor, Cognition/Devin...\"; \"20-30% of engineering tasks\""
    }
  ],
  "summary": {
    "engine_total": 18,
    "baseline_total": 27,
    "max_possible": 48,
    "engine_wins": 0,
    "baseline_wins": 9,
    "ties": 3,
    "delta": -9
  }
}
```

### borda.json Format

```json
{
  "run": "NNN",
  "rubric_version": "v2",
  "methodology_note": "Single evaluator. Borda: winner=2pts, loser=1pt, tie=1.5 each. Max 24 per output.",
  "engine_borda": 13.5,
  "baseline_borda": 22.5,
  "max_per_output": 24,
  "winner": "baseline",
  "per_criterion": [
    {"criterion": "Specificity", "engine": 1.5, "baseline": 1.5, "delta": 0},
    {"criterion": "Novelty", "engine": 1.5, "baseline": 1.5, "delta": 0}
  ]
}
```

## Step 6: Write Diagnosis

`meta/runs/{NNN}/diagnosis.md` — this is the most important artifact for the NEXT iteration.

Structure:
```markdown
# Run {NNN} Diagnosis

## Summary
**Engine: {X}/48 | Baseline: {Y}/48 | Delta: {Z}**
{1-2 sentence summary of what happened}

## Lowest-Scoring Criteria
For each criterion where engine scored 0-1:
### {Criterion Name} (Engine: {X}/4, Baseline: {Y}/4)
- What the engine output lacks
- **Root cause in engine:** {which file, which section, what's missing}
- **Fix:** {specific change that would address this}

## Why the {Winner} Wins
{Structural analysis — what advantage does the winner have?}

## Recommended Changes for Next Iteration
**Priority 1:** {the ONE change to make}
**Priority 2:** {backup if priority 1 doesn't work}

Per meta-loop rules: make ONE targeted change per iteration.
```

## Step 7: Update Progress

Add a row to the score table in `meta/progress.md`:

```markdown
| {NNN} | foresight-v{NNN} | {engine}/48 | {baseline}/48 | {delta} | {winner} | {streak} | {key change description} | {key insight} |
```

Update convergence status:
- If challenger won: `A-wins streak: 0`
- If incumbent won: increment streak
- If streak = 2: `Converged: Yes`

## Step 8: Decide — Tag or Revert

- **Challenger wins:** `git tag foresight-v{NNN}` on the commit
- **Incumbent wins:** Note the loss in progress.md. Do NOT revert the code — keep it committed for history. The next iteration will try a different approach.

## Step 9: Commit and Push

Stage and commit ALL artifacts from this run:
```bash
git add \
  os-apps/paw-foresight/meta/runs/{NNN}/ \
  os-apps/paw-foresight/meta/progress.md \
  os-apps/paw-foresight/system/skills/orchestrate-projection/SKILL.md \
  # ... any other changed files

git commit -m "$(cat <<'EOF'
feat: paw-foresight Run {NNN} — {short description}

{What changed in the engine}
Engine: {X}/48, Baseline: {Y}/48, Delta: {Z}
Winner: {winner}. {Key insight}.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>
EOF
)"

git push
```

If challenger won, also tag:
```bash
git tag foresight-v{NNN}
git push --tags
```

## Key Reminders

- **Read RAW transcripts** for diagnosis. The orchestrator.jsonl shows exactly what
  the orchestrator agent did, what tools it called, what it wrote. This is where you
  find the root cause.
- **ONE change per iteration.** Not two, not "a few related changes." One.
- **Score honestly.** The calibration exists to prevent inflation. A 2 is the expected
  score for competent output. Don't give 3s unless genuinely earned.
- **Evidence for every score.** Quote or cite specific content from the output.
  "The output has good breadth" is not evidence. "Covers 6 themes: X, Y, Z..." is.
- **The baseline is your reality check.** If the engine can't beat a single prompt,
  the engine needs fundamental rethinking, not tuning.
- **Simpler wins at equal scores.** If you can remove a component and maintain scores,
  remove it.
- **Document everything for the vlog.** Someone reading `progress.md` + each run's
  `plan.md` + `changelog.md` + `scores.json` + `diagnosis.md` should be able to
  reconstruct the full story of how the engine evolved.
- **The 12 criteria** (from program.md v2): Specificity, Novelty, Falsifiability,
  Breadth, Plausibility, Progression, Actionability, Decision Clarity, Completeness,
  Transparency, Challenge, Quantitative Precision.

## Reference: Run 000 Results

For calibration, here are the Run 000 scores under rubric v2:

| Criterion | Engine | Baseline |
|-----------|--------|----------|
| Specificity | 2 | 2 |
| Novelty | 2 | 2 |
| Falsifiability | 1 | 2 |
| Breadth | 2 | 3 |
| Plausibility | 2 | 3 |
| Progression | 1 | 2 |
| Actionability | 1 | 2 |
| Decision Clarity | 2 | 2 |
| Completeness | 2 | 3 |
| Transparency | 1 | 2 |
| Challenge | 2 | 3 |
| Quant. Precision | 0 | 1 |
| **Total** | **18** | **27** |

Engine weakest: Quant. Precision (0), Falsifiability (1), Transparency (1), Actionability (1), Progression (1).
