#!/usr/bin/env bash
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel)"

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
  PORT="$(pick_available_port 4000 700)" || {
    printf '[paw-codex-production-smoke] could not find free OData port plus implicit webhook trigger port\n' >&2
    exit 1
  }
elif ! port_is_free "$PORT" || ! port_is_free "$((PORT + 12))"; then
  printf '[paw-codex-production-smoke] PORT=%s or implicit webhook trigger port %s is already in use\n' "$PORT" "$((PORT + 12))" >&2
  exit 1
fi

TEMPER_URL="${TEMPER_URL:-http://127.0.0.1:${PORT}}"
TENANT="${TEMPER_TENANT:-patrol_production_smoke}"
API_KEY="${TEMPER_API_KEY:-patrol-production-smoke}"
DB_PATH="${DB_PATH:-/tmp/paw-patrol-production-smoke-${PORT}-$$.db}"
WORKER_ID="${WORKER_ID:-mac-mini-codex-prod}"
READY_ATTEMPTS="${READY_ATTEMPTS:-480}"
PROOF_DIR="${PROOF_DIR:-/tmp/paw-patrol-production-readiness-proof-${PORT}-$$}"
LAUNCHD_PLIST="${LAUNCHD_PLIST:-${PROOF_DIR}/com.temperpaw.paw-codex-worker.plist}"
READINESS_LOG="${READINESS_LOG:-${PROOF_DIR}/production-readiness.log}"
SERVER_LOG="${SERVER_LOG:-/tmp/paw-patrol-production-readiness-server.log}"

SERVER_PID=""

log() {
  printf '[paw-codex-production-smoke] %s\n' "$*"
}

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    log "missing required command: $1"
    exit 1
  fi
}

cleanup() {
  if [[ -n "${SERVER_PID}" ]] && kill -0 "$SERVER_PID" >/dev/null 2>&1; then
    kill "$SERVER_PID" >/dev/null 2>&1 || true
    wait "$SERVER_PID" >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT

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

assert_contains() {
  local path="$1"
  local needle="$2"
  if ! grep -Fq "$needle" "$path"; then
    log "${path} did not contain expected text: ${needle}"
    exit 1
  fi
}

assert_not_contains() {
  local path="$1"
  local needle="$2"
  if grep -Fq "$needle" "$path"; then
    log "${path} unexpectedly contained sensitive text: ${needle}"
    exit 1
  fi
}

write_proof_bundle() {
  local summary_json="$1"

  cat >"${PROOF_DIR}/proof.md" <<EOF
# Paw Codex Production Readiness Smoke Proof

## Summary

The guarded production-readiness script was exercised against a live local
TemperPaw control plane. It built the release worker, ran
\`paw-codex-worker doctor\`, verified OData plus event-stream access, rendered a
launchd plist into a temporary proof directory, and did not install or load
launchd.

## Guardrails Checked

- \`PAW_CODEX_ENABLE_EXECUTION=0\` kept the worker in observe-only mode.
- \`PAW_CODEX_DOCTOR_EXEC_SMOKE=1\` proved \`codex exec\` can start in the
  worker environment.
- \`WRITE_LAUNCHD_PLIST=1\` rendered the plist for review.
- \`INSTALL_LAUNCHD=0\` prevented launchd mutation.
- The fake worker token did not appear in the readiness stdout/stderr log.

## Files

- Readiness log: ${READINESS_LOG}
- Rendered plist: ${LAUNCHD_PLIST}
- Server log: ${SERVER_LOG}

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

mkdir -p "$PROOF_DIR"

log "repo root: ${ROOT}"
log "server: ${TEMPER_URL}"
log "proof dir: ${PROOF_DIR}"

TEMPERPAW_WASM_STARTUP_POLICY=build \
PORT="$PORT" \
TEMPER_API_KEY="$API_KEY" \
PAW_TENANT="$TENANT" \
TURSO_URL="file:${DB_PATH}" \
cargo run -p temperpaw >"$SERVER_LOG" 2>&1 &
SERVER_PID="$!"

wait_for_metadata
log "control plane ready"

set +e
TEMPER_URL="$TEMPER_URL" \
TEMPER_TENANT="$TENANT" \
WORKER_ID="$WORKER_ID" \
WORKER_TOKEN="$API_KEY" \
REPO_ROOT="$ROOT" \
WORKSPACE_ROOT="$(dirname "$ROOT")" \
CODEX_BIN="$ROOT/crates/paw-codex-worker/fixtures/fake-codex.sh" \
PAW_CODEX_ENABLE_EXECUTION=0 \
PAW_CODEX_DOCTOR_EXEC_SMOKE=1 \
PAW_CODEX_POLL_ON_START=1 \
WRITE_LAUNCHD_PLIST=1 \
INSTALL_LAUNCHD=0 \
LAUNCHD_PLIST="$LAUNCHD_PLIST" \
crates/paw-codex-worker/scripts/production-readiness.sh >"$READINESS_LOG" 2>&1
readiness_status="$?"
set -e

if [[ "$readiness_status" != "0" ]]; then
  log "production readiness script failed with status ${readiness_status}"
  tail -120 "$READINESS_LOG" || true
  exit "$readiness_status"
fi

assert_contains "$READINESS_LOG" "paw-codex-worker doctor"
assert_contains "$READINESS_LOG" "[pass] worker_token: WORKER_TOKEN is set"
assert_contains "$READINESS_LOG" "[pass] codex_bin:"
assert_contains "$READINESS_LOG" "[pass] codex_exec_smoke:"
assert_contains "$READINESS_LOG" "[pass] odata:"
assert_contains "$READINESS_LOG" "[pass] event_stream:"
assert_contains "$READINESS_LOG" "production readiness check passed"
assert_not_contains "$READINESS_LOG" "$API_KEY"

test -s "$LAUNCHD_PLIST"
assert_contains "$LAUNCHD_PLIST" "com.temperpaw.paw-codex-worker"
assert_contains "$LAUNCHD_PLIST" "$TEMPER_URL"
assert_contains "$LAUNCHD_PLIST" "$WORKER_ID"
assert_contains "$LAUNCHD_PLIST" "PAW_CODEX_ENABLE_EXECUTION"
assert_contains "$LAUNCHD_PLIST" "<string>0</string>"
assert_contains "$LAUNCHD_PLIST" "PAW_CODEX_DOCTOR_EXEC_SMOKE"
assert_contains "$LAUNCHD_PLIST" "<string>1</string>"

summary_json="$(jq -n \
  --arg temper_url "$TEMPER_URL" \
  --arg worker_id "$WORKER_ID" \
  --arg readiness_log "$READINESS_LOG" \
  --arg launchd_plist "$LAUNCHD_PLIST" \
  --arg server_log "$SERVER_LOG" \
  '{
    status: "passed",
    control_plane: {
      temper_url: $temper_url,
      metadata: "ready",
      event_stream: "doctor pass"
    },
    worker: {
      worker_id: $worker_id,
      execution_enabled: false,
      codex_binary: "fake-codex fixture",
      codex_exec_smoke: "doctor pass"
    },
    launchd: {
      rendered: true,
      installed: false,
      plist: $launchd_plist
    },
    logs: {
      production_readiness: $readiness_log,
      server: $server_log
    },
    guardrails: {
      token_not_printed_to_readiness_log: true,
      production_database_not_shared: true,
      launchd_not_loaded: true
    }
  }')"

printf '%s\n' "$summary_json" >"${PROOF_DIR}/summary.json"
printf '%s\n' "$summary_json"
write_proof_bundle "$summary_json"

log "production readiness smoke passed"
