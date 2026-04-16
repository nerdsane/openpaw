#!/bin/bash
# Overnight Foresight Meta-Improvement Loop
#
# Launches fresh Claude Code sessions to iteratively improve the foresight engine.
# Each session: read diagnosis → plan change → implement → run engine → score → record.
# Stops when convergence (2 consecutive incumbent wins) or max rounds.
#
# Usage:
#   tmux new -s foresight './os-apps/paw-foresight/meta/loop.sh'
#
# Modeled after turbomoe/autoresearch.sh (Karpathy pattern).
#
# Server isolation: this loop uses its OWN openpaw-server on port 3468 with
# HOME=/tmp/paw-foresight-home so it doesn't collide with the live :3467
# instance that other agents may be using.

set -e
cd "$(dirname "$0")/../../.."  # project root (openpaw-foresight worktree)

MAX_ROUNDS="${1:-50}"
ROUND=1
LOGDIR="os-apps/paw-foresight/meta/logs"
META_DIR="os-apps/paw-foresight/meta"
SERVER_PORT=3468
FORESIGHT_HOME=/tmp/paw-foresight-home
API_KEY_FILE="$FORESIGHT_HOME/.local/share/openpaw/api.key"

mkdir -p "$LOGDIR"

# ─── Helpers ───────────────────────────────────────────────────────────

api_key() {
    cat "$API_KEY_FILE" 2>/dev/null || echo ""
}

check_server() {
    local key=$(api_key)
    [ -z "$key" ] && return 1
    local code=$(curl -s -o /dev/null -w "%{http_code}" \
        "http://localhost:$SERVER_PORT/tdata/ForesightModels" \
        -H "Authorization: Bearer $key" \
        -H "x-temper-tenant: rita-agents" 2>/dev/null)
    [ "$code" = "200" ]
}

ensure_server() {
    if check_server; then
        echo "[$(date +%H:%M)] Isolated server healthy on port $SERVER_PORT"
        return 0
    fi

    echo "[$(date +%H:%M)] Isolated server not responding. Starting..."
    HOME="$FORESIGHT_HOME" PORT=$SERVER_PORT PAW_TENANT=rita-agents OTEL_ENABLED=false \
        ./target/release/openpaw-server run > "$FORESIGHT_HOME/server.log" 2>&1 &
    SERVER_PID=$!
    sleep 20

    if check_server; then
        echo "[$(date +%H:%M)] Isolated server started (PID $SERVER_PID)"
    else
        echo "[$(date +%H:%M)] FATAL: Isolated server failed to start. See $FORESIGHT_HOME/server.log"
        exit 1
    fi
}

check_converged() {
    grep -q "Converged: Yes" "$META_DIR/progress.md" 2>/dev/null
}

current_run_count() {
    ls -d "$META_DIR/runs"/*/ 2>/dev/null | wc -l | tr -d ' '
}

last_diagnosis() {
    local last_dir=$(ls -d "$META_DIR/runs"/*/ 2>/dev/null | sort | tail -1)
    [ -n "$last_dir" ] && cat "${last_dir}diagnosis.md" 2>/dev/null || echo "(no prior diagnosis)"
}

last_scores() {
    local last_dir=$(ls -d "$META_DIR/runs"/*/ 2>/dev/null | sort | tail -1)
    [ -n "$last_dir" ] && cat "${last_dir}scores.json" 2>/dev/null || echo "{}"
}

# ─── Main ──────────────────────────────────────────────────────────────

echo "========================================"
echo "  FORESIGHT META-IMPROVEMENT LOOP"
echo "  Started: $(date)"
echo "  Max rounds: $MAX_ROUNDS"
echo "  Server: http://localhost:$SERVER_PORT (isolated)"
echo "  HOME:   $FORESIGHT_HOME"
echo "========================================"

ensure_server

while [ $ROUND -le $MAX_ROUNDS ]; do
    if check_converged; then
        echo ""
        echo "[$(date +%H:%M)] CONVERGED — engine has stabilized. Stopping."
        break
    fi

    RUN_NUM=$(current_run_count)
    KEY=$(api_key)
    TIMESTAMP=$(date +%Y%m%dT%H%M%S)

    echo ""
    echo "=== ROUND $ROUND / $MAX_ROUNDS (will be Run $RUN_NUM) — $(date) ==="

    # Re-check server health each round
    if ! check_server; then
        echo "[$(date +%H:%M)] Server down between rounds. Restarting..."
        ensure_server
    fi

    PROMPT="You are the meta-agent for the paw-foresight engine improvement loop.

## Your Process

Read .claude/skills/foresight-meta.md FIRST — it contains your complete instructions
including exact API commands, transcript extraction queries, scoring formats, and
documentation templates. Follow it step by step.

Then read these files:
1. os-apps/paw-foresight/meta/program.md — the evaluation rubric (IMMUTABLE — do not modify)
2. os-apps/paw-foresight/meta/progress.md — score history and convergence status
3. The most recent run's diagnosis.md — what to fix and why (read ALL transcripts, not just orchestrator)
4. os-apps/paw-foresight/meta/baseline/synthesis.md — the single-shot baseline output (reference)

Native OTS trajectories (post-Track-3) are emitted by the emit_ots_trajectory WASM —
query the ots_trajectories table directly. Fall back to the JSONL→OTS converter for
archived pre-Track-3 runs only:
  python3 meta/tools/jsonl_to_ots.py meta/runs/<latest>/transcripts/ --summary

## Server (ISOLATED — do NOT use the live :3467 instance)
- Temper API: http://localhost:$SERVER_PORT
- API key: $KEY  (file: $API_KEY_FILE)
- Tenant: rita-agents
- Data dir: $FORESIGHT_HOME/.local/share/openpaw/
- All API calls use: -H 'Authorization: Bearer <key>' -H 'x-temper-tenant: rita-agents'

## Engine base
- Tag: foresight-v100-base (Post-Track-1-reliability era)
- Orchestrator max_fuel: 120B (Track 1 checkpointing landed; old 500B band-aid removed)
- Native OTS trajectory emission via emit_ots_trajectory WASM

## This Run
- Run number: $RUN_NUM
- Run directory: os-apps/paw-foresight/meta/runs/$(printf '%03d' $RUN_NUM)_<description>/

NOTE: The DSE v2 ForesightModel should already exist on the isolated server (seeded
from a backup of the live DB, runtime tables purged). If it's missing, the knowledge
graph blob is in $FORESIGHT_HOME/.local/share/openpaw/paw.db — see the skill file
for the exact recreation procedure.

## CRITICAL: Documentation Requirements

Everything must be recorded for posterity. Each run MUST produce ALL of these:

1. plan.md — what you're changing and why (BEFORE implementing)
2. changelog.md — what you actually changed (with diff or before/after)
3. engine-output/synthesis.md — the engine's output narrative
4. engine-output/observations.json — raw observations from API
5. engine-output/directions.json — raw directions from API
6. transcripts/*.jsonl — ALL session transcripts (orchestrator, probes, synthesis) via sqlite3
7. transcripts/MANIFEST.md — listing each session, its role, turn count, status
8. scores.json — 3 independent blind judge scores with reasoning + evidence per criterion
9. borda.json — Borda aggregation across 3 judges (max 72 per output)
10. diagnosis.md — root cause analysis tying scores to specific engine components
11. Updated progress.md — new row in score table with all columns filled
12. Git commit with descriptive message + push
13. Git tag (foresight-vNNN) if challenger wins — NNN starts at 101 in this era

JUDGES: You MUST create 3 independent Claude Code subagent judges using 'claude -p'.
Each judge receives BOTH outputs side-by-side plus the full rubric (no size limit).
Randomize X/Y assignment per judge. See Step 5 in the skill for exact commands.
Side-by-side comparison is MANDATORY — a judge seeing only one output is invalid.
Only fall back to self-scoring if ALL 3 judge subagents fail.

CONSTRAINTS:
- Domain-agnostic: do NOT hard-code domain-specific logic. Changes must generalize.
- No authoring: do NOT pre-compute content. Score what the engine actually produces.
- Prefer architecture: try structural changes (WASM, entities, sessions) before prompt edits.
- Rubric v4: criteria 10=Grounding (reasoning chain validity), 12=Information Density (no redundancy).

A run without ALL artifacts is incomplete. Do not skip any step.

Execute the full iteration now. Do not stop early or ask for clarification."

    # Run Claude Code — fresh session, full permissions, all tools
    claude --dangerously-skip-permissions -p "$PROMPT" \
        2>&1 | tee "$LOGDIR/round_${ROUND}_${TIMESTAMP}.log"

    EXIT_CODE=$?
    if [ $EXIT_CODE -ne 0 ]; then
        echo "[$(date +%H:%M)] Claude exited with code $EXIT_CODE — continuing to next round"
    fi

    ROUND=$((ROUND + 1))

    # Pause between rounds (let server settle, avoid rate limits)
    echo "[$(date +%H:%M)] Sleeping 30s before next round..."
    sleep 30
done

echo ""
echo "========================================"
echo "  META-LOOP COMPLETE"
echo "  Finished: $(date)"
echo "  Rounds executed: $((ROUND - 1))"
FINAL_PROGRESS=$(tail -3 "$META_DIR/progress.md" 2>/dev/null)
echo "  Latest: $FINAL_PROGRESS"
echo "========================================"
