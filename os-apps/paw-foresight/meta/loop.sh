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

MAX_ROUNDS="${1:-10}"
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

## Instructions

Read these files in order, then execute one full iteration:

1. .claude/skills/foresight-meta.md — your process (the skill)
2. os-apps/paw-foresight/meta/program.md — the evaluation rubric (immutable, do not modify)
3. os-apps/paw-foresight/meta/progress.md — score history and convergence status
4. The most recent run's diagnosis.md and transcripts/ — what went wrong and why

## Server

- Temper API at http://localhost:$SERVER_PORT
- Auth header: Authorization: Bearer $KEY
- Tenant header: x-temper-tenant: rita-agents
- Use curl for all Temper API calls (creating projections, polling sessions, reading entities)

## Current State

This is iteration $ROUND. The next run directory should be: meta/runs/$(printf '%03d' $RUN_NUM)_<short_description>/

NOTE: The server may have been restarted since the last run. Entities (Projections,
ForesightModels, Sessions) may be empty. If the ForesightModel for DSE doesn't exist,
you'll need to recreate it — check the orchestration skill and previous run artifacts
for the knowledge graph file. The knowledge graph content is likely still in the blobs
table even if the entity is gone.

## Process

1. Read the diagnosis from the last run — what's the weakest criterion and root cause?
2. Plan ONE targeted change. Write it to meta/runs/{NNN}/plan.md BEFORE implementing.
3. Implement the change (edit skill text, entity specs, WASM, etc.)
4. If WASM changed: recompile (cargo build --target wasm32-unknown-unknown --release)
5. Reinstall the app if needed
6. Run the foresight engine on the DSE essay (create Projection, Start, poll until Complete)
7. Extract all artifacts (synthesis, observations, directions, transcripts)
8. Score the engine output against the rubric (all 12 criteria, 0-4 scale with strict calibration)
9. Compare against the incumbent output (read from meta/baseline/synthesis.md or the previous run's winner)
10. Record: scores.json, borda.json, diagnosis.md → meta/runs/{NNN}/
11. Update progress.md
12. If challenger wins: git tag foresight-v{NNN}, reset streak. If incumbent wins: increment streak.
13. Git commit and push all changes.

Do the full iteration now. Do not stop early or ask for clarification."

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
