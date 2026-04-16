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
#
# Integrity enforcement (what the loop does that the meta-agent cannot bypass):
#   * before each round: if the prior run's borda shows incumbent won, revert
#     the change commits mechanically. Agent cannot forget.
#   * after each round: run verify_run.py. If any of the 12 invariants fails,
#     the loop stops. Next round will NOT start.
#   * tags are created HERE (loop.sh), not by the agent, using diagnosis.md as
#     annotation. Tag name = foresight-v(100 + run_num) for run 001+.
#   * git push uses plain push (never --force); verify_run.py checks reflog.

set -e
cd "$(dirname "$0")/../../.."  # project root (openpaw-foresight worktree)

MAX_ROUNDS="${1:-50}"
ROUND=1
LOGDIR="os-apps/paw-foresight/meta/logs"
META_DIR="os-apps/paw-foresight/meta"
TOOLS_DIR="$META_DIR/tools"
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

latest_run_dir() {
    ls -d "$META_DIR/runs"/*/ 2>/dev/null | sort | tail -1
}

prior_run_dir_for() {
    # $1 = current run number (integer); find dir matching (N-1)_*
    local prev_num=$(printf '%03d' $(( $1 - 1 )))
    ls -d "$META_DIR/runs/${prev_num}_"*/ 2>/dev/null | head -1
}

# ─── Post-round enforcement ────────────────────────────────────────────
# gap #6 (revert-on-loss), #8 (borda reproduction), #11 (tag from artifact),
# and the umbrella verify_run.py (#1–#12) all run here.

enforce_post_round() {
    local run_dir="$1"
    local run_num=$(basename "$run_dir" | cut -d_ -f1 | sed 's/^0*//')
    [ -z "$run_num" ] && run_num=0

    local head_sha=$(git rev-parse HEAD)
    echo "[$(date +%H:%M)] Running post-round verification on $run_dir (HEAD=$head_sha)..."
    if ! python3 "$TOOLS_DIR/verify_run.py" --run-dir "$run_dir" --head-sha "$head_sha"; then
        echo "[$(date +%H:%M)] VERIFICATION FAILED — loop stopping. Fix invariants in $run_dir and re-launch."
        exit 2
    fi

    # Run 000 is measurement-only; no borda, no tag, no revert
    if [ "$run_num" -eq 0 ]; then
        echo "[$(date +%H:%M)] Run 000 is measurement-only — skipping tournament-dependent enforcement"
        return 0
    fi

    local borda="$run_dir/borda.json"
    if [ ! -f "$borda" ]; then
        echo "[$(date +%H:%M)] FATAL: $borda missing after verification passed — this should be impossible"
        exit 2
    fi

    local winner=$(python3 -c "import json; print(json.load(open('$borda'))['winner'])")
    echo "[$(date +%H:%M)] Run $run_num winner: $winner"

    if [ "$winner" = "challenger" ]; then
        # Create the tag from the run's diagnosis.md (gap #11)
        local tag_num=$(( 100 + run_num ))
        local tag_name="foresight-v${tag_num}"
        local diag="$run_dir/diagnosis.md"
        if [ -f "$diag" ]; then
            echo "[$(date +%H:%M)] Tagging $tag_name from diagnosis.md"
            git tag -a "$tag_name" -F "$diag" HEAD 2>&1 | head -3 || true
            git push origin "$tag_name" 2>&1 | tail -3 || true
        else
            echo "[$(date +%H:%M)] WARN: diagnosis.md missing — skipping tag creation"
        fi
    elif [ "$winner" = "incumbent" ] || [ "$winner" = "tie" ]; then
        # Revert the change commits so the next challenger starts from the last winning state (gap #6).
        # The meta-agent is instructed to make all their code changes between the plan commit and the final docs commit.
        # We revert everything from (plan commit exclusive) to HEAD that touches files outside meta/runs/.
        echo "[$(date +%H:%M)] Incumbent/tie — reverting non-artifact changes from this round"
        # Find the plan commit (first commit of this run dir)
        local plan_commit=$(git log --diff-filter=A --format="%H" -- "$run_dir/plan.md" 2>/dev/null | tail -1)
        if [ -n "$plan_commit" ]; then
            # Revert commits touching code outside meta/runs/. Use a single revert commit.
            local code_changes=$(git diff --name-only "${plan_commit}..HEAD" | grep -v "^os-apps/paw-foresight/meta/runs/" || true)
            if [ -n "$code_changes" ]; then
                echo "[$(date +%H:%M)] Reverting engine changes (files outside meta/runs/):"
                echo "$code_changes" | sed 's/^/    /'
                # Checkout those files from the plan commit's parent (= pre-change state)
                git checkout "${plan_commit}^" -- $code_changes 2>&1 | head -3 || true
                git add $code_changes
                git commit -m "revert: Run $run_num change reverted (incumbent/tie won)

Automatic revert by loop.sh post-round enforcement. See:
  $run_dir/diagnosis.md
  $run_dir/borda.json

Reverted files:
$(echo "$code_changes" | sed 's/^/  /')" 2>&1 | tail -3 || true
                git push origin claude/paw-foresight-meta 2>&1 | tail -3 || true
            else
                echo "[$(date +%H:%M)] No code changes to revert (run only touched meta/runs/)"
            fi
        else
            echo "[$(date +%H:%M)] WARN: could not find plan commit for $run_dir — skipping revert"
        fi
    else
        echo "[$(date +%H:%M)] WARN: unrecognized winner '$winner' — no tag, no revert"
    fi
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
    HEAD_SHA=$(git rev-parse HEAD)

    echo ""
    echo "=== ROUND $ROUND / $MAX_ROUNDS (will be Run $RUN_NUM) — $(date) ==="

    if ! check_server; then
        echo "[$(date +%H:%M)] Server down between rounds. Restarting..."
        ensure_server
    fi

    # Run 000 is special: measurement-only. No tournament, no judges.
    SCORING_MODE="tournament"
    if [ "$RUN_NUM" -eq 0 ]; then
        SCORING_MODE="measurement-only"
    fi

    # Emit the deterministic X/Y assignments for the agent to read (it MUST use these,
    # not invent its own). verify_run.py checks scores.json matches.
    ASSIGN_JSON=$(python3 "$TOOLS_DIR/assign_judges.py" --run $RUN_NUM --head $HEAD_SHA 2>/dev/null || echo '{}')

    PROMPT="You are the meta-agent for the paw-foresight engine improvement loop.

## Your Process

Read .claude/skills/foresight-meta.md FIRST — it contains your complete instructions
including exact API commands, transcript extraction queries, scoring formats, and
documentation templates. Follow it step by step.

## Temper context — READ BEFORE touching the engine

The paw-foresight engine is a Temper app. You cannot improve it competently without
understanding Temper's primitives (entities, WASM integrations, Cedar policies, triggers).
The 'foresight-meta' skill tells you this too, but surfacing it here so you don't skip it:

- ~/.claude/skills/temper-agent/SKILL.md — 'Temper is your operating layer' deep primer
- AGENTS.md (in the repo root) — OpenPaw's Entity-First Rule, Trigger Boundary, anti-patterns
- CLAUDE.md (in the repo root) — worktree rules, TDD, E2E verification

Skim temper-agent through the 'Sandbox Environment' section at minimum; read AGENTS.md
in full. Without this context you will default to prompt edits and miss the actual
editable surface (state machines, WASM logic, Cedar authorization, entity invariants).

## Foresight-specific reading

1. os-apps/paw-foresight/meta/program.md — the evaluation rubric (IMMUTABLE — do not modify)
2. os-apps/paw-foresight/meta/progress.md — score history and convergence status
3. The most recent run's diagnosis.md — what to fix and why (read ALL transcripts, not just orchestrator)
4. os-apps/paw-foresight/meta/baseline/scores.json — the single-shot baseline (reference only)

Post-Track-3: OTS trajectories emit natively into the ots_trajectories table via the
emit_ots_trajectory WASM. Query that table first. Fall back to the JSONL→OTS
converter only for archived pre-Track-3 runs.

## Server (ISOLATED — do NOT use the live :3467 instance)
- Temper API: http://localhost:$SERVER_PORT
- API key: $KEY  (file: $API_KEY_FILE)
- Tenant: rita-agents
- Data dir: $FORESIGHT_HOME/.local/share/openpaw/

## This Run
- Run number: $RUN_NUM
- Run directory: os-apps/paw-foresight/meta/runs/$(printf '%03d' $RUN_NUM)_<description>/
- Scoring mode: $SCORING_MODE
- git HEAD sha: $HEAD_SHA
- Deterministic X/Y assignments (use these, DO NOT randomize yourself):
$ASSIGN_JSON

## Integrity requirements — your run WILL be rejected by verify_run.py if ANY of these fails

1. plan.md MUST contain a '## Scope' bullet list of every file you will modify outside meta/runs/.
   Files changed but not in scope → run rejected.
2. engine-output/synthesis.md must be copied VERBATIM from the orchestrator session's final
   output. The synthesis content must be found as a substring in the orchestrator blob. Hand-
   written content → run rejected.
3. Build judge prompts by substituting {{RUBRIC}}, {{OUTPUT_X}}, {{OUTPUT_Y}} into
   meta/judge_prompt_template.md WITHOUT altering any other text. Record
   judge_prompt_template_sha256 in scores.json (sha256 of the template file). Mismatch → rejected.
4. Use the deterministic X/Y assignments above verbatim; write them into scores.json.judges[i].x_is_challenger.
   verify_run.py re-derives from (run, judge, HEAD sha) and compares.
5. Compute Borda ONLY by invoking 'python3 $TOOLS_DIR/borda.py --run-dir <rundir>'. Do not reimplement.
   verify_run.py re-runs and must match.
6. If a judge invocation fails, capture the failure output to judge_N_attempt.log in the run dir
   and set scores.json.self_scored=true. Only then may you self-score.
7. If Run > 000 and the prior run directory has no engine-output/synthesis.md, HALT — do not
   fall back to baseline. Report the missing incumbent.
8. Do NOT create the foresight-v{NNN} tag. loop.sh does that from diagnosis.md after verification passes.
9. Do NOT force-push. Plain 'git push origin claude/paw-foresight-meta' only.

## Run 000 / measurement-only mode ($SCORING_MODE)

If this is measurement-only (Run 000 of the era), you:
  - produce plan.md, changelog.md, engine-output/*, transcripts/*, diagnosis.md
  - do NOT run judges, do NOT produce scores.json or borda.json
  - update progress.md with engine absolute score left blank (measurement recorded as the
    first incumbent for Run 001 to score against)

## Documentation Required (each run MUST produce)

1. plan.md (with '## Scope' bullet list)
2. changelog.md
3. engine-output/synthesis.md  (provenance-verified)
4. engine-output/observations.json
5. engine-output/directions.json
6. transcripts/*.jsonl  (all sessions)
7. transcripts/MANIFEST.md  (MUST identify orchestrator session id as 'orchestrator ss-<uuid>')
8. scores.json  (tournament runs only)
9. borda.json  (tournament runs only, via borda.py)
10. diagnosis.md (rich enough that the tag annotation reads cleanly from it)
11. Updated progress.md
12. Git commit + plain push

CONSTRAINTS:
- Domain-agnostic: do NOT hard-code domain-specific logic.
- No authoring: do NOT pre-compute engine content. Score what the engine actually produces.
- Prefer architecture: try structural changes (WASM, entities, sessions) before prompt edits.
- Rubric v4: criteria 10=Grounding, 12=Information Density.

Execute the full iteration now. Do not stop early or ask for clarification."

    # Run Claude Code — fresh session, full permissions, all tools
    claude --dangerously-skip-permissions -p "$PROMPT" \
        2>&1 | tee "$LOGDIR/round_${ROUND}_${TIMESTAMP}.log"

    EXIT_CODE=${PIPESTATUS[0]}
    if [ $EXIT_CODE -ne 0 ]; then
        echo "[$(date +%H:%M)] Claude exited with code $EXIT_CODE"
    fi

    # Locate this run's directory — must match the expected run number
    CURRENT_RUN_DIR=$(ls -d "$META_DIR/runs/$(printf '%03d' $RUN_NUM)_"*/ 2>/dev/null | head -1)
    if [ -z "$CURRENT_RUN_DIR" ]; then
        echo "[$(date +%H:%M)] FATAL: agent did not create a run directory for run $RUN_NUM — stopping"
        exit 3
    fi

    # Post-round enforcement: verify invariants, tag (or revert), then advance
    enforce_post_round "$CURRENT_RUN_DIR"

    ROUND=$((ROUND + 1))

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
