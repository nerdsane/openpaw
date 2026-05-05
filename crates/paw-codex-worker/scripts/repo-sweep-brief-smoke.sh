#!/usr/bin/env bash
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel)"
WORKSPACE_ROOT="${WORKSPACE_ROOT:-$(dirname "$ROOT")}"

port_is_free() {
  local port="$1"
  if ! command -v lsof >/dev/null 2>&1; then
    return 0
  fi
  ! lsof -nP -iTCP:"${port}" -sTCP:LISTEN >/dev/null 2>&1
}

pick_available_port() {
  local base="$1"
  local width="$2"
  local candidate
  for _ in $(seq 1 120); do
    candidate=$((base + RANDOM % width))
    if port_is_free "$candidate" && port_is_free "$((candidate + 12))"; then
      printf '%s\n' "$candidate"
      return 0
    fi
  done
  return 1
}

if [[ -z "${PORT:-}" ]]; then
  PORT="$(pick_available_port 3800 700)" || {
    printf '[paw-patrol-repo-smoke] could not find free OData port plus implicit webhook trigger port\n' >&2
    exit 1
  }
elif ! port_is_free "$PORT" || ! port_is_free "$((PORT + 12))"; then
  printf '[paw-patrol-repo-smoke] PORT=%s or implicit webhook trigger port %s is already in use\n' "$PORT" "$((PORT + 12))" >&2
  exit 1
fi

TEMPER_URL="${TEMPER_URL:-http://127.0.0.1:${PORT}}"
TENANT="${TEMPER_TENANT:-patrol_repo_smoke}"
API_KEY="${TEMPER_API_KEY:-patrol-repo-smoke}"
DB_PATH="${DB_PATH:-/tmp/paw-patrol-repo-smoke-${PORT}-$$.db}"
WORKER_ID="${WORKER_ID:-mac-mini-codex-prod}"
READY_ATTEMPTS="${READY_ATTEMPTS:-300}"
PROOF_DIR="${PROOF_DIR:-/tmp/paw-patrol-repo-smoke-proof-${PORT}-$$}"
PATROL_WASM_BUILD="os-apps/paw-patrol/wasm/build.sh"

SERVER_PID=""
WORKER_PID=""
WORKTREE_PATH=""
BRANCH_NAME=""

log() {
  printf '[paw-patrol-repo-smoke] %s\n' "$*"
}

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    log "missing required command: $1"
    exit 1
  fi
}

cleanup() {
  if [[ -n "${WORKER_PID}" ]] && kill -0 "$WORKER_PID" >/dev/null 2>&1; then
    kill "$WORKER_PID" >/dev/null 2>&1 || true
    wait "$WORKER_PID" >/dev/null 2>&1 || true
  fi
  if [[ -n "${SERVER_PID}" ]] && kill -0 "$SERVER_PID" >/dev/null 2>&1; then
    kill "$SERVER_PID" >/dev/null 2>&1 || true
    wait "$SERVER_PID" >/dev/null 2>&1 || true
  fi
  if [[ -n "${WORKTREE_PATH}" && -d "${WORKTREE_PATH}" ]]; then
    git -C "$ROOT" worktree remove --force "$WORKTREE_PATH" >/dev/null 2>&1 || true
  fi
  if [[ -n "${BRANCH_NAME}" ]]; then
    git -C "$ROOT" branch -D "$BRANCH_NAME" >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT

curl_json() {
  curl -fsS \
    -H "Authorization: Bearer ${API_KEY}" \
    -H "Content-Type: application/json" \
    "$@"
}

post_json() {
  local url="$1"
  local body="$2"
  curl_json -X POST "$url" -d "$body"
}

entity_url() {
  local set="$1"
  local id="$2"
  printf "%s/tdata/%s('%s')" "$TEMPER_URL" "$set" "$id"
}

field() {
  local key="$1"
  jq -r --arg key "$key" '.fields[$key] // .fields[($key | ascii_downcase)] // ""'
}

wait_for_metadata() {
  for _ in $(seq 1 "$READY_ATTEMPTS"); do
    if curl -fsS -H "Authorization: Bearer ${API_KEY}" "${TEMPER_URL}/tdata/\$metadata" >/dev/null 2>&1; then
      return 0
    fi
    sleep 1
  done
  log "server did not become ready at ${TEMPER_URL}"
  exit 1
}

wait_for_status() {
  local set="$1"
  local id="$2"
  local wanted="$3"
  local attempts="${4:-120}"
  local body status
  for _ in $(seq 1 "$attempts"); do
    body="$(curl_json "$(entity_url "$set" "$id")")"
    status="$(jq -r '.status // .fields.Status // .fields.status // ""' <<<"$body")"
    if [[ "$status" == "$wanted" ]]; then
      printf '%s' "$body"
      return 0
    fi
    sleep 1
  done
  log "${set}('${id}') did not reach ${wanted}"
  curl_json "$(entity_url "$set" "$id")" | jq .
  exit 1
}

wait_for_field() {
  local set="$1"
  local id="$2"
  local field_name="$3"
  local attempts="${4:-120}"
  local body value
  for _ in $(seq 1 "$attempts"); do
    body="$(curl_json "$(entity_url "$set" "$id")")"
    value="$(field "$field_name" <<<"$body")"
    if [[ -n "$value" ]]; then
      printf '%s' "$body"
      return 0
    fi
    sleep 1
  done
  log "${set}('${id}') did not populate ${field_name}"
  curl_json "$(entity_url "$set" "$id")" | jq .
  exit 1
}

decode_svg_data_uri() {
  local visual_summary_url="$1"
  local output_path="$2"
  if [[ "$visual_summary_url" == data:image/svg+xml,* ]]; then
    local encoded_svg="${visual_summary_url#data:image/svg+xml,}"
    printf '%b' "${encoded_svg//%/\\x}" >"$output_path"
  else
    printf '%s\n' "$visual_summary_url" >"${output_path%.svg}-visual-url.txt"
  fi
}

write_proof_bundle() {
  local summary_json="$1"
  local schedule_body="$2"
  local snapshot_body="$3"
  local proof_body="$4"
  local brief_body="$5"
  local graph_json proof_json proof_markdown proof_visual brief_visual brief_summary
  local brief_done brief_risks proof_ids

  mkdir -p "$PROOF_DIR"
  printf '%s\n' "$summary_json" >"$PROOF_DIR/summary.json"
  jq . <<<"$schedule_body" >"$PROOF_DIR/patrol-schedule.json"

  graph_json="$(field graph_json <<<"$snapshot_body")"
  if jq -e . >/dev/null 2>&1 <<<"$graph_json"; then
    jq . <<<"$graph_json" >"$PROOF_DIR/repo-graph.json"
  else
    printf '%s\n' "$graph_json" >"$PROOF_DIR/repo-graph.json"
  fi

  proof_json="$(field proof_json <<<"$proof_body")"
  if jq -e . >/dev/null 2>&1 <<<"$proof_json"; then
    jq . <<<"$proof_json" >"$PROOF_DIR/proof.json"
  else
    printf '%s\n' "$proof_json" >"$PROOF_DIR/proof.json"
  fi

  proof_markdown="$(field summary_markdown <<<"$proof_body")"
  proof_visual="$(field visual_summary_url <<<"$proof_body")"
  brief_visual="$(field visual_summary_url <<<"$brief_body")"
  brief_summary="$(field summary_markdown <<<"$brief_body")"
  brief_done="$(field done_items <<<"$brief_body")"
  brief_risks="$(field open_risks <<<"$brief_body")"
  proof_ids="$(field proof_packet_ids <<<"$brief_body")"

  decode_svg_data_uri "$proof_visual" "$PROOF_DIR/proof.svg"
  decode_svg_data_uri "$brief_visual" "$PROOF_DIR/daily-brief.svg"

  cat >"$PROOF_DIR/proof.md" <<EOF
# Paw Patrol Repo Sweep And Daily Brief Smoke Proof

## Repo Sweep Proof

${proof_markdown}

## Daily Brief

${brief_summary}

## Daily Brief Done Items

\`\`\`json
${brief_done}
\`\`\`

## Daily Brief Open Risks

\`\`\`json
${brief_risks}
\`\`\`

## Daily Brief Proof Packets

\`\`\`json
${proof_ids}
\`\`\`

## OData Links

- PatrolSchedule: ${TEMPER_URL}/tdata/PatrolSchedules('$(jq -r '.entities.default_schedule' <<<"$summary_json")')
- RepoGraphSnapshot: ${TEMPER_URL}/tdata/RepoGraphSnapshots('$(jq -r '.entities.repo_graph_snapshot' <<<"$summary_json")')
- WorkerRun: ${TEMPER_URL}/tdata/WorkerRuns('$(jq -r '.entities.worker_run' <<<"$summary_json")')
- ReviewRun: ${TEMPER_URL}/tdata/ReviewRuns('$(jq -r '.entities.review_run' <<<"$summary_json")')
- EvaluationRun: ${TEMPER_URL}/tdata/EvaluationRuns('$(jq -r '.entities.evaluation_run' <<<"$summary_json")')
- ProofPacket: ${TEMPER_URL}/tdata/ProofPackets('$(jq -r '.entities.proof_packet' <<<"$summary_json")')
- DailyBrief: ${TEMPER_URL}/tdata/DailyBriefs('$(jq -r '.entities.daily_brief' <<<"$summary_json")')

## Trace And Log Evidence

- Server log: /tmp/paw-patrol-repo-smoke-server.log
- Worker log: /tmp/paw-patrol-repo-smoke-worker.log
- WASM build log: /tmp/paw-patrol-repo-smoke-wasm-build.log

## Machine Summary

\`\`\`json
${summary_json}
\`\`\`
EOF

  log "proof bundle: ${PROOF_DIR}"
}

require_cmd cargo
require_cmd curl
require_cmd git
require_cmd jq

log "repo root: ${ROOT}"
log "workspace root: ${WORKSPACE_ROOT}"
log "server: ${TEMPER_URL}"

log "building current paw-patrol WASM modules"
(cd "$ROOT/$(dirname "$PATROL_WASM_BUILD")" && bash "$(basename "$PATROL_WASM_BUILD")") \
  >/tmp/paw-patrol-repo-smoke-wasm-build.log 2>&1

TEMPERPAW_WASM_STARTUP_POLICY=build \
PORT="$PORT" \
TEMPER_API_KEY="$API_KEY" \
PAW_TENANT="$TENANT" \
TURSO_URL="file:${DB_PATH}" \
cargo run -p temperpaw >/tmp/paw-patrol-repo-smoke-server.log 2>&1 &
SERVER_PID="$!"

wait_for_metadata
log "control plane ready"

default_schedule_id="patrol-default-daily-maintenance"
default_schedule_body="$(wait_for_status PatrolSchedules "$default_schedule_id" Active 120)"
default_schedule_body="$(wait_for_field PatrolSchedules "$default_schedule_id" next_run_at 120)"
default_schedule_next_run_at="$(field next_run_at <<<"$default_schedule_body")"
log "Default PatrolSchedule ${default_schedule_id} Active; next run ${default_schedule_next_run_at}"

snapshot_id="$(post_json "${TEMPER_URL}/tdata/RepoGraphSnapshots" '{}' | jq -r '.entity_id')"
post_json \
  "$(entity_url RepoGraphSnapshots "$snapshot_id")/TemperPaw.Patrol.StartScan" \
  "$(jq -n --arg commit "repo-smoke-$(git -C "$ROOT" rev-parse --short HEAD)" '{commit_sha:$commit}')" \
  >/dev/null

snapshot_scanning_body="$(wait_for_field RepoGraphSnapshots "$snapshot_id" worker_run_id 120)"
work_cycle_id="$(field work_cycle_id <<<"$snapshot_scanning_body")"
worker_run_id="$(field worker_run_id <<<"$snapshot_scanning_body")"
worker_allowed_id="$(curl_json "$(entity_url WorkerRuns "$worker_run_id")" | field allowed_worker_id)"
if [[ "$worker_allowed_id" != "$WORKER_ID" ]]; then
  log "WorkerRun allowed_worker_id was '${worker_allowed_id}', expected '${WORKER_ID}'"
  exit 1
fi

log "RepoGraphSnapshot ${snapshot_id}"
log "WorkCycle ${work_cycle_id}"
log "WorkerRun ${worker_run_id}"
log "Allowed worker ${worker_allowed_id}"

TEMPER_URL="$TEMPER_URL" \
TEMPER_TENANT="$TENANT" \
WORKER_ID="$WORKER_ID" \
WORKER_TOKEN="$API_KEY" \
REPO_ROOT="$ROOT" \
WORKSPACE_ROOT="$WORKSPACE_ROOT" \
CODEX_BIN="$ROOT/crates/paw-codex-worker/fixtures/fake-codex.sh" \
PAW_CODEX_ENABLE_EXECUTION=0 \
PAW_CODEX_POLL_ON_START=1 \
cargo run -p paw-codex-worker >/tmp/paw-patrol-repo-smoke-worker.log 2>&1 &
WORKER_PID="$!"

worker_body="$(wait_for_status WorkerRuns "$worker_run_id" Done 180)"
WORKTREE_PATH="$(field worktree_path <<<"$worker_body")"
BRANCH_NAME="$(field branch_name <<<"$worker_body")"
snapshot_ready_body="$(wait_for_status RepoGraphSnapshots "$snapshot_id" Ready 60)"
work_cycle_body="$(wait_for_status WorkCycles "$work_cycle_id" Complete 180)"

review_run_id="$(field reviewer_run_id <<<"$work_cycle_body")"
evaluation_run_id="$(field evaluation_run_id <<<"$work_cycle_body")"
proof_packet_id="$(field proof_packet_id <<<"$work_cycle_body")"

review_status="$(curl_json "$(entity_url ReviewRuns "$review_run_id")" | jq -r '.status')"
evaluation_status="$(curl_json "$(entity_url EvaluationRuns "$evaluation_run_id")" | jq -r '.status')"
proof_body="$(curl_json "$(entity_url ProofPackets "$proof_packet_id")")"
proof_status="$(jq -r '.status' <<<"$proof_body")"
finding_count="$(field finding_count <<<"$snapshot_ready_body")"

brief_id="$(post_json "${TEMPER_URL}/tdata/DailyBriefs" '{}' | jq -r '.entity_id')"
post_json \
  "$(entity_url DailyBriefs "$brief_id")/TemperPaw.Patrol.Start" \
  "$(jq -n --arg brief_date "$(date +%F)" '{brief_date:$brief_date}')" \
  >/dev/null
brief_body="$(wait_for_status DailyBriefs "$brief_id" Ready 90)"
brief_status="$(jq -r '.status' <<<"$brief_body")"

summary_json="$(jq -n \
  --arg default_schedule "$default_schedule_id" \
  --arg repo_graph_snapshot "$snapshot_id" \
  --arg work_cycle "$work_cycle_id" \
  --arg worker_run "$worker_run_id" \
  --arg review_run "$review_run_id" \
  --arg evaluation_run "$evaluation_run_id" \
  --arg proof_packet "$proof_packet_id" \
  --arg daily_brief "$brief_id" \
  --arg default_schedule_status "$(jq -r '.status' <<<"$default_schedule_body")" \
  --arg default_schedule_next_run_at "$default_schedule_next_run_at" \
  --arg snapshot_status "$(jq -r '.status' <<<"$snapshot_ready_body")" \
  --arg work_cycle_status "$(jq -r '.status' <<<"$work_cycle_body")" \
  --arg review_status "$review_status" \
  --arg evaluation_status "$evaluation_status" \
  --arg proof_status "$proof_status" \
  --arg brief_status "$brief_status" \
  --arg finding_count "$finding_count" \
  --arg branch "$BRANCH_NAME" \
  --arg worktree "$WORKTREE_PATH" \
  --arg allowed_worker "$worker_allowed_id" \
  '{
    statuses: {
      default_schedule: $default_schedule_status,
      repo_graph_snapshot: $snapshot_status,
      work_cycle: $work_cycle_status,
      review: $review_status,
      evaluation: $evaluation_status,
      proof: $proof_status,
      daily_brief: $brief_status
    },
    entities: {
      default_schedule: $default_schedule,
      repo_graph_snapshot: $repo_graph_snapshot,
      work_cycle: $work_cycle,
      worker_run: $worker_run,
      review_run: $review_run,
      evaluation_run: $evaluation_run,
      proof_packet: $proof_packet,
      daily_brief: $daily_brief
    },
    repo_sweep: {
      finding_count: $finding_count
    },
    default_schedule: {
      next_run_at: $default_schedule_next_run_at
    },
    git: {
      branch: $branch,
      worktree: $worktree
    },
    worker: {
      allowed_worker_id: $allowed_worker
    }
  }')"

printf '%s\n' "$summary_json"
write_proof_bundle "$summary_json" "$default_schedule_body" "$snapshot_ready_body" "$proof_body" "$brief_body"

log "repo sweep and daily brief smoke passed"
