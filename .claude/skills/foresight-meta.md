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

### Convert to OTS (structured analysis)

After extracting raw JSONL transcripts, convert them for structured analysis:

```bash
# Diagnostic summary — shows all sessions, turns, errors, what each agent did
python3 meta/tools/jsonl_to_ots.py "meta/runs/{NNN}/transcripts/" --summary

# Full OTS output — structured turns with decisions, tool calls, consequences
python3 meta/tools/jsonl_to_ots.py "meta/runs/{NNN}/transcripts/" -o "meta/runs/{NNN}/trajectories.json"
```

The summary shows per-session: role, outcome, turn count, token usage, error rate,
top temper methods called, first/last actions. Use this to quickly identify which
sessions failed, which were most active, and where errors occurred.

The full OTS output gives turn-by-turn detail: each LLM call → tool selection →
code executed → result/error. Use this for root cause analysis when a specific
criterion scores poorly (e.g., "why is Breadth weak?" → check what probes actually
observed → find the probe transcripts → see what web searches they ran and what
entities they created).

## Step 5: Score — 3 Independent Blind Judges (Claude Code Subagents)

This follows the Judge Protocol in program.md exactly. DO NOT skip judges and self-score.

### Why Claude Code Subagents

Paw-agent sessions route user_message through WASM (32KB field limit). Two foresight
outputs + rubric exceed 32KB. Claude Code subagents (`claude -p`) have no size limit
and can receive BOTH outputs side-by-side — which is required for valid comparison.

### 5a. Read Both Outputs

```bash
CHALLENGER=$(cat "meta/runs/{NNN}/engine-output/synthesis.md")
INCUMBENT=$(cat "meta/baseline/synthesis.md")  # or previous winner
```

Read `meta/program.md` to get the full rubric with all 12 criteria and anchors.

### 5b. Build the Judge Prompt

Each judge sees the FULL rubric + BOTH outputs side-by-side. Randomize the
assignment of incumbent/challenger to X/Y for each judge.

```bash
# Randomize assignment per judge (flip a coin)
# Judge 1: X=engine, Y=baseline
# Judge 2: X=baseline, Y=engine
# Judge 3: X=engine, Y=baseline
# (or use: $((RANDOM % 2)) to decide per judge)
```

The prompt template:

```
You are an independent evaluator of two foresight projection outputs.
Score BOTH outputs on all 12 criteria (0-4). Be strict. Enforce the 3+ cap rule.

## Evaluation Criteria (0-4 scale, 12 criteria, max 48)

Calibration: 2=competent median, 3=genuinely impressive, 4=exceptional and rare.

**3+ CAP RULE:** No more than 3 criteria may score 3+ for any single output.
If more than 3 qualify, demote the weakest to 2. Document demotions.

[Full 12 criteria with anchors from program.md — include the complete anchor
tables, not abbreviated versions. The prompt has no size limit.]

## Output X

[full text of output X]

## Output Y

[full text of output Y]

## Task

Score both outputs on all 12 criteria. For each criterion, explain your
reasoning with specific evidence from both outputs. Return JSON only:

{"criteria": [
  {
    "criterion": "Specificity",
    "output_x_score": 2,
    "output_y_score": 3,
    "reasoning": "Output Y names 4 specific actors with dates while Output X uses generic categories...",
    "evidence_x": ["Section 2 says 'companies will...' without naming any"],
    "evidence_y": ["Section 3 names 'GitHub Copilot by Q3 2026' and 'Vercel integration by...'"]
  },
  ...
]}
```

### 5c. Launch 3 Judge Subagents

```bash
# Write the judge prompt to a temp file (avoids shell escaping issues)
JUDGE_PROMPT_FILE=$(mktemp)
cat > "$JUDGE_PROMPT_FILE" << 'PROMPT_EOF'
[full judge prompt with rubric + both outputs]
PROMPT_EOF

# Launch 3 judges sequentially (each is a fresh Claude Code session)
for JUDGE_NUM in 1 2 3; do
    RESULT_FILE="meta/runs/{NNN}/judge_${JUDGE_NUM}_raw.json"

    # Swap X/Y assignment for even-numbered judges
    # Build the specific prompt with the right X/Y mapping

    claude -p "$(cat $JUDGE_PROMPT_FILE)" \
        --output-format json \
        2>/dev/null | python3 -c "
import sys, json
data = json.load(sys.stdin)
# Extract the JSON from the result field
print(data.get('result', ''))
" > "$RESULT_FILE"

    echo "Judge $JUDGE_NUM complete: $RESULT_FILE"
done
```

**Note:** Each `claude -p` call is a fresh session with no shared context. This ensures
independence. If a judge fails (exits non-zero, empty output, unparseable JSON), retry
once. If still fails, log it and continue with remaining judges. 2 of 3 is acceptable.

### 5d. Parse and Combine Scores

```python
import json, os

criteria_names = [
    "Specificity", "Novelty", "Falsifiability", "Breadth", "Plausibility",
    "Progression", "Actionability", "Decision Clarity", "Completeness",
    "Grounding", "Challenge", "Information Density"
]

judges = {}
for judge_num in [1, 2, 3]:
    raw_file = f"meta/runs/{{NNN}}/judge_{judge_num}_raw.json"
    if not os.path.exists(raw_file):
        continue
    with open(raw_file) as f:
        data = json.load(f)

    # Map X/Y back to engine/baseline based on this judge's assignment
    x_is_engine = (judge_num % 2 == 1)  # odd judges: X=engine
    judges[f"judge_{judge_num}"] = {
        "x_is_engine": x_is_engine,
        "criteria": data["criteria"]
    }

# Compute Borda per criterion per judge
borda = {"engine": 0, "baseline": 0, "per_criterion": []}
for crit in criteria_names:
    crit_engine_borda = 0
    crit_baseline_borda = 0
    for jname, jdata in judges.items():
        for c in jdata["criteria"]:
            if c["criterion"] == crit:
                x_score = c["output_x_score"]
                y_score = c["output_y_score"]
                if jdata["x_is_engine"]:
                    e_score, b_score = x_score, y_score
                else:
                    e_score, b_score = y_score, x_score

                if e_score > b_score:
                    crit_engine_borda += 2; crit_baseline_borda += 1
                elif b_score > e_score:
                    crit_baseline_borda += 2; crit_engine_borda += 1
                else:
                    crit_engine_borda += 1.5; crit_baseline_borda += 1.5
    borda["engine"] += crit_engine_borda
    borda["baseline"] += crit_baseline_borda
    borda["per_criterion"].append({
        "criterion": crit,
        "engine": crit_engine_borda,
        "baseline": crit_baseline_borda,
        "delta": crit_engine_borda - crit_baseline_borda
    })
```

### 5e. Fallback: Meta-Agent Self-Scoring

If ALL 3 judge subagents fail (crashes, empty output, unparseable):
1. Log the failure in scores.json methodology_note
2. Fall back to meta-agent (you) scoring directly
3. Note this as a methodology limitation — results are less trustworthy

### scores.json Format

```json
{
  "methodology": "3 independent Claude Code subagent judges (claude -p). Side-by-side comparison. Rubric v4 (Grounding, Information Density). 3+ cap rule.",
  "rubric_version": "v4",
  "judges": {
    "judge_1": {
      "x_is_engine": true,
      "criteria": [{"criterion": "...", "output_x_score": 2, "output_y_score": 3, "reasoning": "...", "evidence_x": [...], "evidence_y": [...]}]
    }
  }
}
```

### borda.json Format

```json
{
  "run": "NNN",
  "rubric_version": "v4",
  "methodology_note": "3 Claude Code subagent judges, side-by-side. Borda: winner=2pts, loser=1pt, tie=1.5. Max 6 per criterion, 72 total.",
  "engine_borda": 42,
  "baseline_borda": 30,
  "max_per_output": 72,
  "winner": "engine",
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

- **Read ALL transcripts** for diagnosis, not just the orchestrator. Read every JSONL
  in the transcripts/ directory: orchestrator, probes, analysts, synthesizer. Each
  reveals different failure modes. The orchestrator shows dispatch logic, probes show
  observation quality, analysts show reasoning, synthesizer shows final composition.
  Root causes often live in probe or analyst transcripts, not the orchestrator.
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
- **The 12 criteria** (from program.md v4): Specificity, Novelty, Falsifiability,
  Breadth, Plausibility, Progression, Actionability, Decision Clarity, Completeness,
  Grounding, Challenge, Information Density.
- **Domain-agnostic.** Do NOT hard-code domain-specific logic. Changes must generalize.
- **No authoring.** Do NOT pre-compute or inject content the engine failed to generate.
  Score what the engine actually produced.
- **Prefer architecture.** Try structural changes (WASM, entities, sessions, data flows)
  before prompt edits. Prose instructions are advisory; LLMs ignore them ~80% of the time.

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
| Grounding | 1 | 2 |
| Challenge | 2 | 3 |
| Info. Density | 0 | 1 |
| **Total** | **18** | **27** |

Engine weakest: Info. Density (0), Falsifiability (1), Grounding (1), Actionability (1), Progression (1).

**Note:** These are rubric v2/v3 scores. Under rubric v4, criterion 10 (Transparency→Grounding)
and criterion 12 (Quantitative Precision→Information Density) have changed anchors. Baseline
will be re-scored ONCE under v4. Use v4 scores going forward.
