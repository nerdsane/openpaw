#!/bin/bash
# Run 000: Score the clean engine without changes.
# Establishes the starting point for evolution.
set -e
cd "$(dirname "$0")/../../.."

KEY=$(cat ~/.local/share/openpaw/api.key 2>/dev/null)
LOGDIR="os-apps/paw-foresight/meta/logs"
mkdir -p "$LOGDIR"

PROMPT="You are running the FIRST evaluation of the foresight engine under rubric v4.

## YOUR TASK: Score the engine as-is. Do NOT change any engine code.

This is Run 000 — the initial scoring run before any evolution begins.

## Steps

1. Read os-apps/paw-foresight/meta/program.md — the evaluation rubric (IMMUTABLE)
2. Read os-apps/paw-foresight/meta/progress.md — empty score table, you'll add the first row
3. Read .claude/skills/foresight-meta.md for operational instructions

4. Create run directory: os-apps/paw-foresight/meta/runs/000_initial/
   with subdirs: engine-output/ transcripts/

5. Run the engine:
   - Check for existing ForesightModel (DSE essay knowledge graph)
   - Create a Projection, Configure with foresight_model_id and horizon '1 year', Start
   - IMPORTANT: Poll for status 'Complete' (NOT 'Completed') — the Projection entity uses 'Complete'
   - The engine takes 10-20 min. spawn_orchestrator WASM creates 1 orchestrator session which runs the full loop.
   - Extract: synthesis narrative, observations JSON, directions JSON

6. Extract ALL session transcripts via sqlite3 (orchestrator, every probe, any analysts)
   Then convert for analysis: python3 meta/tools/jsonl_to_ots.py transcripts/ --summary

7. Score with 3 Claude Code subagent judges ('claude -p'):
   - Each judge sees BOTH engine output AND baseline SIDE-BY-SIDE
   - Baseline: os-apps/paw-foresight/meta/baseline/synthesis.md
   - Randomize which is X vs Y per judge
   - Include FULL rubric from program.md (all 12 criteria with anchor tables)
   - Judges return JSON with scores for both outputs per criterion

8. Aggregate via Borda count. Write scores.json and borda.json.

9. Write diagnosis.md: which criteria are weakest, root causes in the engine,
   what should the first evolution iteration target.

10. Update progress.md with the first row.

11. Git commit and push.

## Run Details
- Run: 000
- Directory: os-apps/paw-foresight/meta/runs/000_initial/
- Server: http://localhost:3467
- API key: $KEY
- Tenant: rita-agents

## CRITICAL
- Do NOT modify SKILL.md, WASM, or any engine code
- Do NOT modify program.md
- Score what the engine produces, warts and all
- If a component fails, note it in diagnosis — do NOT fix or work around it

Execute now."

echo "=== RUN 000 — $(date) ==="
claude --dangerously-skip-permissions -p "$PROMPT" \
    2>&1 | tee "$LOGDIR/run_000_$(date +%Y%m%dT%H%M%S).log"
echo "=== RUN 000 COMPLETE — $(date) ==="
