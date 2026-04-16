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

## Step 5: Score — 3 Independent Blind Judges

This follows the Judge Protocol in program.md exactly. DO NOT skip judges and self-score.

### 5a. Read Both Outputs

```bash
CHALLENGER=$(cat "meta/runs/{NNN}/engine-output/synthesis.md")
INCUMBENT=$(cat "meta/baseline/synthesis.md")  # or previous winner
```

Read `meta/program.md` to get the full rubric with all 12 criteria and anchors.

### 5b. Build the Judge Prompt — SPLIT-SESSION APPROACH

**IMPORTANT: 32KB WASM field limit.** The user_message field is truncated when WASM reads entity
state. With both outputs inlined (~30KB combined) plus rubric, the prompt exceeds 32KB and the
session fails with "user_message is empty". 

**Solution: Split-session scoring.** Each judge gets 2 sessions — one per output. Each session
scores one output independently. Combine scores afterward to compute Borda.

Build a COMPACT rubric (criteria + anchors only, no preamble/protocol sections from program.md).
Include the 3+ cap rule and calibration note. Target: <2KB for the rubric portion.

```python
# Read outputs
CHALLENGER = open("meta/runs/{NNN}/engine-output/synthesis.md").read()
INCUMBENT = open("meta/baseline/synthesis.md").read()   # or previous winner

# Build compact rubric from program.md (criteria + anchors only)
# Include: calibration note, 3+ cap rule, all 12 criteria with anchor tables
# Exclude: preamble, boundary constraints, judge protocol, tournament protocol
# Target: under 2KB

COMPACT_RUBRIC = """## Evaluation Criteria (0-4 scale, 12 criteria, max 48)
Calibration: 2=competent median, 3=genuinely impressive, 4=exceptional and rare.

**3+ CAP RULE:** No more than 3 criteria may score 3+ for any single output.
If more than 3 qualify, demote the weakest to 2. Document demotions.

1. Specificity: 0=none | 1=generic | 2=named actors OR timelines | 3=actors+timelines+mechanisms | 4=dates+thresholds
2. Novelty: 0=restates | 1=extensions | 2=1-2 insights | 3=multiple from OUTSIDE input | 4=reframes domain
... (all 12 with compact anchors) ..."""

def build_prompt(output_text):
    return f"""You are an independent evaluator of a foresight projection output.
Score strictly. Enforce the 3+ cap rule.

{COMPACT_RUBRIC}

## Output to Score

{output_text}

## Task
Score all 12 criteria (0-4). Return JSON only:
{{"criteria": [{{"criterion": "Specificity", "score": 2, "reasoning": "...", "evidence": "..."}}...]}}"""
```

### 5c. Create 6 Judge Sessions (3 judges × 2 outputs)

Use Python `urllib.request` for proper HTTP — shell curl has JSON encoding issues with large prompts.

```python
import json, urllib.request

API_KEY = open("~/.local/share/openpaw/api.key").read().strip()
BASE = "http://localhost:3467"
HEADERS = {"Authorization": f"Bearer {API_KEY}", "x-temper-tenant": "rita-agents", "Content-Type": "application/json"}

def api_post(path, data):
    req = urllib.request.Request(f"{BASE}{path}", json.dumps(data).encode(), HEADERS, method='POST')
    return json.loads(urllib.request.urlopen(req).read())

sessions = {}
for judge_num in [1, 2, 3]:
    for output_name, output_text in [("engine", CHALLENGER), ("baseline", INCUMBENT)]:
        label = f"judge{judge_num}_{output_name}"
        result = api_post("/tdata/Sessions", {})
        sess_id = result["entity_id"]
        sessions[label] = sess_id
        
        prompt = build_prompt(output_text)
        api_post(f"/tdata/Sessions('{sess_id}')/Temper.Configure", {
            "user_message": prompt,
            "model": "gpt-5.4",
            "provider": "openai_codex",
            "max_turns": "5"
        })
        print(f"{label}: {sess_id} ({len(prompt)} bytes)")
```

### 5d. Poll for Results

Sessions may stay in "Steering" state but still have results in the `result` field.
Poll for the `result` field rather than waiting for "Completed" status.

```python
import time

for attempt in range(40):
    time.sleep(30)
    all_have_results = True
    for label, sid in sessions.items():
        data = api_get(f"/tdata/Sessions('{sid}')")
        result = data.get("fields", {}).get("result", "")
        if not result or len(result) < 50:
            all_have_results = False
    if all_have_results:
        break
```

### 5e. Extract and Combine Scores

```python
# Extract from result field (NOT transcript — sessions may not finalize JSONL)
for label, sid in sessions.items():
    data = api_get(f"/tdata/Sessions('{sid}')")
    result_str = data["fields"]["result"]
    scores = json.loads(result_str)
    # scores["criteria"] = [{criterion, score, reasoning, evidence}, ...]

# Combine per-judge: merge engine + baseline sessions
# Compute Borda: per criterion, per judge, higher score = 2 pts, lower = 1, tie = 1.5
```

### 5f. Fallback: Meta-Agent Self-Scoring

If ALL 3 judge sessions fail (stuck, no result field, unparseable output):
1. Log the failure in scores.json methodology_note
2. Fall back to meta-agent (you) scoring directly
3. Note this as a methodology limitation

### 5g. Aggregate via Borda Count

Per criterion, per judge: rank the two outputs by score.
- Winner (higher score) gets 2 Borda points
- Loser gets 1 Borda point
- Tie: 1.5 each

Sum across 3 judges per criterion (max 6 per criterion per output).
Sum across 12 criteria (max 72 Borda points per output).
Ties in overall Borda: incumbent wins (conservative).

### scores.json Format

```json
{
  "methodology": "3 independent paw-agent judges (gpt-5.4), split-session (one per output per judge). Rubric v3 with tightened anchors + 3+ cap rule.",
  "rubric_version": "v3",
  "session_ids": {"judge1_engine": "ss-...", "judge1_baseline": "ss-...", ...},
  "judges": {
    "judge_1": {"criteria": [{"criterion": "...", "engine_score": 2, "baseline_score": 3, ...}]}
  }
}
```

### borda.json Format (3-judge version)

```json
{
  "run": "NNN",
  "rubric_version": "v2",
  "methodology_note": "3 judges. Borda: winner=2pts, loser=1pt, tie=1.5. Max 6 per criterion, 72 total per output.",
  "engine_borda": 28,
  "baseline_borda": 44,
  "max_per_output": 72,
  "winner": "baseline",
  "per_criterion": [
    {"criterion": "Specificity", "engine": 3.0, "baseline": 6.0, "delta": -3.0},
    {"criterion": "Novelty", "engine": 4.5, "baseline": 4.5, "delta": 0}
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
