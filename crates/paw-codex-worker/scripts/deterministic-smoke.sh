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
  PORT="$(pick_available_port 3600 700)" || {
    printf '[paw-patrol-smoke] could not find free OData port plus implicit webhook trigger port\n' >&2
    exit 1
  }
elif ! port_is_free "$PORT" || ! port_is_free "$((PORT + 12))"; then
  printf '[paw-patrol-smoke] PORT=%s or implicit webhook trigger port %s is already in use\n' "$PORT" "$((PORT + 12))" >&2
  exit 1
fi

TEMPER_URL="${TEMPER_URL:-http://127.0.0.1:${PORT}}"
TENANT="${TEMPER_TENANT:-patrol_smoke}"
API_KEY="${TEMPER_API_KEY:-patrol-smoke}"
DB_PATH="${DB_PATH:-/tmp/paw-patrol-smoke-${PORT}-$$.db}"
WORKER_ID="${WORKER_ID:-mac-mini-codex-prod}"
REQUEST_TEXT="${REQUEST_TEXT:-Produce a visual proof packet after the worker completes.}"
KEEP_WORKTREE="${KEEP_WORKTREE:-0}"
READY_ATTEMPTS="${READY_ATTEMPTS:-300}"
PROOF_DIR="${PROOF_DIR:-/tmp/paw-patrol-smoke-proof-${PORT}-$$}"
PATROL_WASM_BUILD="os-apps/paw-patrol/wasm/build.sh"

SERVER_PID=""
WORKER_PID=""
WORKTREE_PATH=""
BRANCH_NAME=""

log() {
  printf '[paw-patrol-smoke] %s\n' "$*"
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
  if [[ "${KEEP_WORKTREE}" != "1" && -n "${WORKTREE_PATH}" && -d "${WORKTREE_PATH}" ]]; then
    git -C "$ROOT" worktree remove --force "$WORKTREE_PATH" >/dev/null 2>&1 || true
  fi
  if [[ "${KEEP_WORKTREE}" != "1" && -n "${BRANCH_NAME}" ]]; then
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

write_proof_bundle() {
  local summary_json="$1"
  local proof_body="$2"
  local visual_summary_url summary_markdown proof_json state_diagram_mermaid changed_files_map reviewer_verdict residual_risks

  mkdir -p "$PROOF_DIR"
  printf '%s\n' "$summary_json" >"$PROOF_DIR/summary.json"

  visual_summary_url="$(field visual_summary_url <<<"$proof_body")"
  summary_markdown="$(field summary_markdown <<<"$proof_body")"
  proof_json="$(field proof_json <<<"$proof_body")"
  state_diagram_mermaid="$(field state_diagram_mermaid <<<"$proof_body")"
  changed_files_map="$(field changed_files_map <<<"$proof_body")"
  reviewer_verdict="$(field reviewer_verdict <<<"$proof_body")"
  residual_risks="$(field residual_risks <<<"$proof_body")"

  if [[ "$visual_summary_url" == data:image/svg+xml,* ]]; then
    local encoded_svg="${visual_summary_url#data:image/svg+xml,}"
    printf '%b' "${encoded_svg//%/\\x}" >"$PROOF_DIR/proof.svg"
  else
    printf '%s\n' "$visual_summary_url" >"$PROOF_DIR/proof-visual-url.txt"
  fi

  if jq -e . >/dev/null 2>&1 <<<"$proof_json"; then
    jq . <<<"$proof_json" >"$PROOF_DIR/proof.json"
  else
    printf '%s\n' "$proof_json" >"$PROOF_DIR/proof.json"
  fi

  cat >"$PROOF_DIR/proof.md" <<EOF
# Paw Patrol Deterministic Smoke Proof

## Summary

${summary_markdown}

## Reviewer Verdict

${reviewer_verdict:-"(none recorded)"}

## Residual Risks

${residual_risks:-"(none recorded)"}

## State Diagram

\`\`\`mermaid
${state_diagram_mermaid}
\`\`\`

## Changed Files Map

\`\`\`json
${changed_files_map}
\`\`\`

## OData Links

- PatrolRequest: ${TEMPER_URL}/tdata/PatrolRequests('$(jq -r '.entities.patrol_request' <<<"$summary_json")')
- FactoryCase: ${TEMPER_URL}/tdata/FactoryCases('$(jq -r '.entities.factory_case' <<<"$summary_json")')
- WorkCycle: ${TEMPER_URL}/tdata/WorkCycles('$(jq -r '.entities.work_cycle' <<<"$summary_json")')
- WorkerRun: ${TEMPER_URL}/tdata/WorkerRuns('$(jq -r '.entities.worker_run' <<<"$summary_json")')
- ReviewRun: ${TEMPER_URL}/tdata/ReviewRuns('$(jq -r '.entities.review_run' <<<"$summary_json")')
- EvaluationRun: ${TEMPER_URL}/tdata/EvaluationRuns('$(jq -r '.entities.evaluation_run' <<<"$summary_json")')
- ProofPacket: ${TEMPER_URL}/tdata/ProofPackets('$(jq -r '.entities.proof_packet' <<<"$summary_json")')

## Trace And Log Evidence

- Server log: /tmp/paw-patrol-smoke-server.log
- Worker log: /tmp/paw-patrol-smoke-worker.log
- WASM build log: /tmp/paw-patrol-smoke-wasm-build.log

## Machine Summary

\`\`\`json
${summary_json}
\`\`\`
EOF

  log "proof bundle: ${PROOF_DIR}"
}

wait_for_metadata() {
  local attempts="$READY_ATTEMPTS"
  for _ in $(seq 1 "$attempts"); do
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
  local attempts="${4:-90}"
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

require_cmd cargo
require_cmd curl
require_cmd git
require_cmd jq

log "repo root: ${ROOT}"
log "workspace root: ${WORKSPACE_ROOT}"
log "server: ${TEMPER_URL}"

log "building current paw-patrol WASM modules"
(cd "$ROOT/$(dirname "$PATROL_WASM_BUILD")" && bash "$(basename "$PATROL_WASM_BUILD")") \
  >/tmp/paw-patrol-smoke-wasm-build.log 2>&1

TEMPERPAW_WASM_STARTUP_POLICY=build \
PORT="$PORT" \
TEMPER_API_KEY="$API_KEY" \
PAW_TENANT="$TENANT" \
TURSO_URL="file:${DB_PATH}" \
cargo run -p temperpaw >/tmp/paw-patrol-smoke-server.log 2>&1 &
SERVER_PID="$!"

wait_for_metadata
log "control plane ready"

request_id="$(
  post_json "${TEMPER_URL}/tdata/PatrolRequests" '{}' | jq -r '.entity_id'
)"

post_json \
  "$(entity_url PatrolRequests "$request_id")/TemperPaw.Patrol.Submit" \
  "$(jq -n --arg text "$REQUEST_TEXT" '{source:"codex-smoke", request_text:$text, requester_id:"codex-smoke"}')" \
  >/dev/null

request_body="$(wait_for_status PatrolRequests "$request_id" Linked 90)"
case_id="$(field factory_case_id <<<"$request_body")"
pm_issue_id="$(field pm_issue_id <<<"$request_body")"
work_cycle_id="$(curl_json "$(entity_url FactoryCases "$case_id")" | field work_cycle_id)"
worker_run_id="$(curl_json "$(entity_url WorkCycles "$work_cycle_id")" | field implementer_worker_run_id)"
worker_allowed_id="$(curl_json "$(entity_url WorkerRuns "$worker_run_id")" | field allowed_worker_id)"
if [[ "$worker_allowed_id" != "$WORKER_ID" ]]; then
  log "WorkerRun allowed_worker_id was '${worker_allowed_id}', expected '${WORKER_ID}'"
  exit 1
fi

log "PatrolRequest ${request_id} linked"
log "FactoryCase ${case_id}"
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
PAW_CODEX_ENABLE_EXECUTION=1 \
PAW_CODEX_POLL_ON_START=1 \
PAW_CODEX_EVAL_COMMANDS='test -f .paw-fake-codex-implementation' \
cargo run -p paw-codex-worker >/tmp/paw-patrol-smoke-worker.log 2>&1 &
WORKER_PID="$!"

worker_body="$(wait_for_status WorkerRuns "$worker_run_id" Done 120)"
WORKTREE_PATH="$(field worktree_path <<<"$worker_body")"
BRANCH_NAME="$(field branch_name <<<"$worker_body")"
work_cycle_body="$(wait_for_status WorkCycles "$work_cycle_id" Complete 120)"
case_body="$(wait_for_status FactoryCases "$case_id" Complete 30)"

review_run_id="$(field reviewer_run_id <<<"$work_cycle_body")"
evaluation_run_id="$(field evaluation_run_id <<<"$work_cycle_body")"
proof_packet_id="$(field proof_packet_id <<<"$work_cycle_body")"

review_status="$(curl_json "$(entity_url ReviewRuns "$review_run_id")" | jq -r '.status')"
evaluation_status="$(curl_json "$(entity_url EvaluationRuns "$evaluation_run_id")" | jq -r '.status')"
proof_status="$(curl_json "$(entity_url ProofPackets "$proof_packet_id")" | jq -r '.status')"
proof_body="$(curl_json "$(entity_url ProofPackets "$proof_packet_id")")"
case_status="$(jq -r '.status' <<<"$case_body")"

summary_json="$(jq -n \
  --arg patrol_request "$request_id" \
  --arg factory_case "$case_id" \
  --arg pm_issue "$pm_issue_id" \
  --arg work_cycle "$work_cycle_id" \
  --arg worker_run "$worker_run_id" \
  --arg review_run "$review_run_id" \
  --arg evaluation_run "$evaluation_run_id" \
  --arg proof_packet "$proof_packet_id" \
  --arg case_status "$case_status" \
  --arg review_status "$review_status" \
  --arg evaluation_status "$evaluation_status" \
  --arg proof_status "$proof_status" \
  --arg branch "$BRANCH_NAME" \
  --arg worktree "$WORKTREE_PATH" \
  --arg allowed_worker "$worker_allowed_id" \
  '{
    statuses: {
      factory_case: $case_status,
      review: $review_status,
      evaluation: $evaluation_status,
      proof: $proof_status
    },
    entities: {
      patrol_request: $patrol_request,
      factory_case: $factory_case,
      pm_issue: $pm_issue,
      work_cycle: $work_cycle,
      worker_run: $worker_run,
      review_run: $review_run,
      evaluation_run: $evaluation_run,
      proof_packet: $proof_packet
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
write_proof_bundle "$summary_json" "$proof_body"

log "deterministic smoke passed"
