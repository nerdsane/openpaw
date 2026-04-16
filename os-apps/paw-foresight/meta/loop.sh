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

set -e
cd "$(dirname "$0")/../../.."  # project root (openpaw-codex)

MAX_ROUNDS="${1:-50}"
ROUND=1
LOGDIR="os-apps/paw-foresight/meta/logs"
META_DIR="os-apps/paw-foresight/meta"
SERVER_PORT=3467

mkdir -p "$LOGDIR"

# ─── Helpers ───────────────────────────────────────────────────────────

api_key() {
    cat ~/.local/share/openpaw/api.key 2>/dev/null || echo ""
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
        echo "[$(date +%H:%M)] Server healthy on port $SERVER_PORT"
        return 0
    fi

    echo "[$(date +%H:%M)] Server not responding. Starting..."
    PAW_TENANT=rita-agents ./target/release/openpaw-server run &
    SERVER_PID=$!
    sleep 15

    if check_server; then
        echo "[$(date +%H:%M)] Server started (PID $SERVER_PID)"
    else
        echo "[$(date +%H:%M)] FATAL: Server failed to start."
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
3. The most recent run's diagnosis.md — what to fix and why
4. The most recent run's transcripts/orchestrator.jsonl — raw agent reasoning trace
5. os-apps/paw-foresight/meta/baseline/synthesis.md — the incumbent to beat
6. os-apps/paw-foresight/meta/baseline/prompt.md — the baseline prompt (learn from its structure)

## Server
- Temper API: http://localhost:$SERVER_PORT
- API key: $KEY
- Tenant: rita-agents
- All API calls use: -H 'Authorization: Bearer <key>' -H 'x-temper-tenant: rita-agents'

## This Run
- Run number: $RUN_NUM
- Run directory: os-apps/paw-foresight/meta/runs/$(printf '%03d' $RUN_NUM)_<description>/

NOTE: The server may have been restarted. Entities (ForesightModels, Projections) may
be empty. Check first. If the DSE ForesightModel doesn't exist, recreate it — the
knowledge graph blob persists in ~/.local/share/openpaw/paw.db even if the entity is gone.
See the skill file for the exact recreation procedure.

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
13. Git tag (foresight-vNNN) if challenger wins

JUDGES: You MUST create 3 independent paw-agent judge sessions (see Step 5 in the skill).
Use SPLIT-SESSION approach: 6 sessions total (one per output per judge) to stay under 32KB WASM
field limit. Use compact rubric (criteria + anchors only). Use Python urllib.request for HTTP
(shell curl has JSON encoding issues with large prompts). Extract scores from session result
field (sessions may stay in Steering state). Aggregate via Borda.
Only fall back to self-scoring if ALL 3 judge sessions fail.

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
