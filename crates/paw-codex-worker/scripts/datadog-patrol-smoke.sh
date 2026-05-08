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
    printf '[paw-patrol-datadog-smoke] could not find free OData port plus implicit webhook trigger port\n' >&2
    exit 1
  }
elif ! port_is_free "$PORT" || ! port_is_free "$((PORT + 12))"; then
  printf '[paw-patrol-datadog-smoke] PORT=%s or implicit webhook trigger port %s is already in use\n' "$PORT" "$((PORT + 12))" >&2
  exit 1
fi

is_local_temper_url() {
  case "$1" in
    http://127.0.0.1:* | http://localhost:*)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

if [[ -n "${TEMPER_URL:-}" && "${ALLOW_REMOTE_TEMPER_URL:-0}" != "1" ]] && ! is_local_temper_url "$TEMPER_URL"; then
  printf '[paw-patrol-datadog-smoke] refusing non-local TEMPER_URL=%s; unset TEMPER_URL for local smoke or set ALLOW_REMOTE_TEMPER_URL=1 for an intentional remote run\n' "$TEMPER_URL" >&2
  exit 1
fi

TEMPER_URL="${TEMPER_URL:-http://127.0.0.1:${PORT}}"
TENANT="${TEMPER_TENANT:-patrol_datadog_smoke}"
API_KEY="${TEMPER_API_KEY:-patrol-datadog-smoke}"
DB_PATH="${DB_PATH:-/tmp/paw-patrol-datadog-smoke-${PORT}-$$.db}"
WORKER_ID="${WORKER_ID:-mac-mini-codex-prod}"
READY_ATTEMPTS="${READY_ATTEMPTS:-300}"
PROOF_DIR="${PROOF_DIR:-/tmp/paw-patrol-datadog-smoke-proof-${PORT}-$$}"
if [[ -z "${RUNTIME_ROOT:-}" ]]; then
  RUNTIME_ROOT="$(mktemp -d "/tmp/paw-patrol-datadog-smoke-runtime-${PORT}-XXXXXX")"
  RUNTIME_ROOT_OWNED="1"
else
  RUNTIME_ROOT_OWNED="0"
fi

SERVER_PID=""
WORKER_PID=""
WORKTREE_PATH=""
BRANCH_NAME=""

log() {
  printf '[paw-patrol-datadog-smoke] %s\n' "$*"
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
  if [[ "${KEEP_RUNTIME_ROOT:-0}" != "1" && "${RUNTIME_ROOT_OWNED}" == "1" && -n "${RUNTIME_ROOT}" && -d "${RUNTIME_ROOT}" ]]; then
    rm -rf "$RUNTIME_ROOT" >/dev/null 2>&1 || true
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
  jq -r --arg key "$key" '
    def norm_key: ascii_downcase | gsub("_"; "");
    def top_level: . as $root | ($root[$key]
      // $root[($key | ascii_downcase)]
      // ($root | to_entries[]? | select(.key | norm_key == ($key | norm_key)) | .value)
      // empty);
    (top_level
      // .fields[$key]
      // .fields[($key | ascii_downcase)]
      // (.fields // {} | to_entries[]? | select(.key | norm_key == ($key | norm_key)) | .value)
      // "")
  '
}

first_json_id() {
  jq -Rr 'fromjson? | if type == "array" then .[0] // "" else "" end'
}

json_count() {
  jq -Rr 'fromjson? | if type == "array" then length else 0 end'
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

prepare_runtime_root() {
  mkdir -p "$RUNTIME_ROOT"
  if command -v rsync >/dev/null 2>&1; then
    rsync -a --delete --exclude '*/target/' "$ROOT/os-apps/" "$RUNTIME_ROOT/os-apps/"
  else
    rm -rf "$RUNTIME_ROOT/os-apps"
    cp -R "$ROOT/os-apps" "$RUNTIME_ROOT/os-apps"
    find "$RUNTIME_ROOT/os-apps" -type d -name target -prune -exec rm -rf {} +
  fi
}

write_proof_bundle() {
  local summary_json="$1"
  local patrol_body="$2"
  local proof_body="$3"
  local finding_body="$4"
  local proof_json proof_markdown proof_visual finding_evidence

  mkdir -p "$PROOF_DIR"
  printf '%s\n' "$summary_json" | jq . >"$PROOF_DIR/summary.json"
  jq . <<<"$patrol_body" >"$PROOF_DIR/patrol-run.json"
  jq . <<<"$proof_body" >"$PROOF_DIR/proof-packet.json"
  jq . <<<"$finding_body" >"$PROOF_DIR/observability-finding.json"

  proof_json="$(field proof_json <<<"$proof_body")"
  if jq -e . >/dev/null 2>&1 <<<"$proof_json"; then
    jq . <<<"$proof_json" >"$PROOF_DIR/proof.json"
  else
    printf '%s\n' "$proof_json" >"$PROOF_DIR/proof.json"
  fi

  proof_markdown="$(field summary_markdown <<<"$proof_body")"
  proof_visual="$(field visual_summary_url <<<"$proof_body")"
  finding_evidence="$(field evidence_json <<<"$finding_body")"
  decode_svg_data_uri "$proof_visual" "$PROOF_DIR/datadog-patrol.svg"

  cat >"$PROOF_DIR/proof.md" <<EOF
# Datadog Patrol Smoke Proof

${proof_markdown}

## Finding Evidence

\`\`\`json
${finding_evidence}
\`\`\`

## OData Links

- PatrolRun: ${TEMPER_URL}/tdata/PatrolRuns('$(jq -r '.entities.patrol_run' <<<"$summary_json")')
- WorkerRun: ${TEMPER_URL}/tdata/WorkerRuns('$(jq -r '.entities.worker_run' <<<"$summary_json")')
- Signal: ${TEMPER_URL}/tdata/Signals('$(jq -r '.entities.signal' <<<"$summary_json")')
- ObservabilityFinding: ${TEMPER_URL}/tdata/ObservabilityFindings('$(jq -r '.entities.observability_finding' <<<"$summary_json")')
- FactoryCase: ${TEMPER_URL}/tdata/FactoryCases('$(jq -r '.entities.factory_case' <<<"$summary_json")')
- WorkCycle: ${TEMPER_URL}/tdata/WorkCycles('$(jq -r '.entities.work_cycle' <<<"$summary_json")')
- ProofPacket: ${TEMPER_URL}/tdata/ProofPackets('$(jq -r '.entities.proof_packet' <<<"$summary_json")')

## Trace And Log Evidence

- Server log: /tmp/paw-patrol-datadog-smoke-server.log
- Worker log: /tmp/paw-patrol-datadog-smoke-worker.log
- Visual proof: ${PROOF_DIR}/datadog-patrol.svg

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
prepare_runtime_root
log "runtime os-apps: ${RUNTIME_ROOT}/os-apps"

(
  cd "$RUNTIME_ROOT"
  TEMPERPAW_WASM_STARTUP_POLICY=build \
  PORT="$PORT" \
  TEMPER_API_KEY="$API_KEY" \
  PAW_TENANT="$TENANT" \
  LOCAL_CODEX_WORKER_ID="$WORKER_ID" \
  LOCAL_CODEX_WORKTREE_ROOT="$WORKSPACE_ROOT" \
  TURSO_URL="file:${DB_PATH}" \
  cargo run --manifest-path "$ROOT/Cargo.toml" -p temperpaw
) >/tmp/paw-patrol-datadog-smoke-server.log 2>&1 &
SERVER_PID="$!"

wait_for_metadata
log "control plane ready"

patrol_run_id="$(post_json "${TEMPER_URL}/tdata/PatrolRuns" '{}' | jq -r '.entity_id')"
post_json \
  "$(entity_url PatrolRuns "$patrol_run_id")/TemperPaw.Patrol.Configure" \
  '{"patrol_kind":"datadog_observability","summary":"Datadog MCP smoke patrol","requested_by":"datadog-patrol-smoke","required_capabilities":"datadog_query"}' \
  >/dev/null
post_json \
  "$(entity_url PatrolRuns "$patrol_run_id")/TemperPaw.Patrol.Start" \
  '{}' \
  >/dev/null

running_body="$(wait_for_status PatrolRuns "$patrol_run_id" Running 120)"
worker_run_id="$(field worker_run_id <<<"$running_body")"
worker_body="$(curl_json "$(entity_url WorkerRuns "$worker_run_id")")"
worker_allowed_id="$(field allowed_worker_id <<<"$worker_body")"
if [[ "$worker_allowed_id" != "$WORKER_ID" ]]; then
  log "WorkerRun allowed_worker_id was '${worker_allowed_id}', expected '${WORKER_ID}'"
  exit 1
fi

log "PatrolRun ${patrol_run_id}"
log "WorkerRun ${worker_run_id}"
log "Allowed worker ${worker_allowed_id}"

# The fake Codex fixture emits DATADOG_PATROL_RESULT_JSON_BEGIN/END just like
# the real Datadog MCP agent contract; paw-codex-worker validates that packet
# and dispatches PatrolRun.RecordEvidence.
TEMPER_URL="$TEMPER_URL" \
TEMPER_TENANT="$TENANT" \
WORKER_ID="$WORKER_ID" \
WORKER_TOKEN="$API_KEY" \
REPO_ROOT="$ROOT" \
WORKSPACE_ROOT="$WORKSPACE_ROOT" \
CODEX_BIN="$ROOT/crates/paw-codex-worker/fixtures/fake-codex.sh" \
PAW_CODEX_WORKER_CAPABILITIES=local_codex,repo_write,review,evaluation,datadog_query \
PAW_CODEX_ENABLE_EXECUTION=1 \
PAW_CODEX_POLL_ON_START=1 \
cargo run --manifest-path "$ROOT/Cargo.toml" -p paw-codex-worker >/tmp/paw-patrol-datadog-smoke-worker.log 2>&1 &
WORKER_PID="$!"

patrol_complete_body="$(wait_for_status PatrolRuns "$patrol_run_id" Complete 180)"
worker_done_body="$(wait_for_status WorkerRuns "$worker_run_id" Done 60)"
WORKTREE_PATH="$(field worktree_path <<<"$worker_done_body")"
BRANCH_NAME="$(field branch_name <<<"$worker_done_body")"

signal_ids="$(field signal_ids <<<"$patrol_complete_body")"
finding_ids="$(field observability_finding_ids <<<"$patrol_complete_body")"
case_ids="$(field factory_case_ids <<<"$patrol_complete_body")"
work_cycle_ids="$(field work_cycle_ids <<<"$patrol_complete_body")"
proof_packet_id="$(field proof_packet_id <<<"$patrol_complete_body")"

signal_count="$(json_count <<<"$signal_ids")"
finding_count="$(json_count <<<"$finding_ids")"
case_count="$(json_count <<<"$case_ids")"
work_cycle_count="$(json_count <<<"$work_cycle_ids")"
if [[ "$signal_count" -lt 1 || "$finding_count" -lt 1 || "$case_count" -lt 1 || "$work_cycle_count" -lt 1 || -z "$proof_packet_id" ]]; then
  log "PatrolRun did not create expected fanout entities"
  jq . <<<"$patrol_complete_body"
  exit 1
fi

signal_id="$(first_json_id <<<"$signal_ids")"
finding_id="$(first_json_id <<<"$finding_ids")"
case_id="$(first_json_id <<<"$case_ids")"
work_cycle_id="$(first_json_id <<<"$work_cycle_ids")"

finding_body="$(curl_json "$(entity_url ObservabilityFindings "$finding_id")")"
work_cycle_body="$(curl_json "$(entity_url WorkCycles "$work_cycle_id")")"
proof_body="$(curl_json "$(entity_url ProofPackets "$proof_packet_id")")"

finding_source="$(field source <<<"$finding_body")"
work_cycle_status="$(jq -r '.status' <<<"$work_cycle_body")"
proof_status="$(jq -r '.status' <<<"$proof_body")"
if [[ "$finding_source" != "datadog_mcp" || "$proof_status" != "Ready" ]]; then
  log "Unexpected finding/proof states: source=${finding_source}, proof=${proof_status}"
  exit 1
fi

summary_json="$(jq -n \
  --arg patrol_run "$patrol_run_id" \
  --arg worker_run "$worker_run_id" \
  --arg signal "$signal_id" \
  --arg observability_finding "$finding_id" \
  --arg factory_case "$case_id" \
  --arg work_cycle "$work_cycle_id" \
  --arg proof_packet "$proof_packet_id" \
  --arg patrol_status "$(jq -r '.status' <<<"$patrol_complete_body")" \
  --arg worker_status "$(jq -r '.status' <<<"$worker_done_body")" \
  --arg finding_source "$finding_source" \
  --arg work_cycle_status "$work_cycle_status" \
  --arg proof_status "$proof_status" \
  --arg signal_count "$signal_count" \
  --arg finding_count "$finding_count" \
  --arg case_count "$case_count" \
  --arg work_cycle_count "$work_cycle_count" \
  --arg branch "$BRANCH_NAME" \
  --arg worktree "$WORKTREE_PATH" \
  --arg allowed_worker "$worker_allowed_id" \
  --arg runtime_root "$RUNTIME_ROOT" \
  '{
    statuses: {
      patrol_run: $patrol_status,
      worker_run: $worker_status,
      observability_finding_source: $finding_source,
      work_cycle: $work_cycle_status,
      proof_packet: $proof_status
    },
    counts: {
      signals: ($signal_count | tonumber),
      observability_findings: ($finding_count | tonumber),
      factory_cases: ($case_count | tonumber),
      work_cycles: ($work_cycle_count | tonumber)
    },
    entities: {
      patrol_run: $patrol_run,
      worker_run: $worker_run,
      signal: $signal,
      observability_finding: $observability_finding,
      factory_case: $factory_case,
      work_cycle: $work_cycle,
      proof_packet: $proof_packet
    },
    git: {
      branch: $branch,
      worktree: $worktree
    },
    worker: {
      allowed_worker_id: $allowed_worker
    },
    runtime: {
      root: $runtime_root,
      os_apps: ($runtime_root + "/os-apps")
    }
  }')"

printf '%s\n' "$summary_json"
write_proof_bundle "$summary_json" "$patrol_complete_body" "$proof_body" "$finding_body"

log "datadog patrol smoke passed"
