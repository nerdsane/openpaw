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
  PORT="$(pick_available_port 4400 700)" || {
    printf '[paw-patrol-observe-smoke] could not find free OData port plus implicit webhook trigger port\n' >&2
    exit 1
  }
elif ! port_is_free "$PORT" || ! port_is_free "$((PORT + 12))"; then
  printf '[paw-patrol-observe-smoke] PORT=%s or implicit webhook trigger port %s is already in use\n' "$PORT" "$((PORT + 12))" >&2
  exit 1
fi

TEMPER_URL="${TEMPER_URL:-http://127.0.0.1:${PORT}}"
TENANT="${TEMPER_TENANT:-patrol_observe_smoke}"
API_KEY="${TEMPER_API_KEY:-patrol-observe-smoke}"
DB_PATH="${DB_PATH:-/tmp/paw-patrol-observe-smoke-${PORT}-$$.db}"
WORKER_ID="${WORKER_ID:-mac-mini-codex-prod}"
READY_ATTEMPTS="${READY_ATTEMPTS:-300}"
PROOF_DIR="${PROOF_DIR:-/tmp/paw-patrol-observe-smoke-proof-${PORT}-$$}"
SERVER_LOG="${SERVER_LOG:-/tmp/paw-patrol-observe-smoke-server.log}"
WORKER_LOG="${WORKER_LOG:-/tmp/paw-patrol-observe-smoke-worker.log}"
WASM_BUILD_LOG="${WASM_BUILD_LOG:-/tmp/paw-patrol-observe-smoke-wasm-build.log}"
PATROL_WASM_BUILD="os-apps/paw-patrol/wasm/build.sh"

SERVER_PID=""
WORKER_PID=""

log() {
  printf '[paw-patrol-observe-smoke] %s\n' "$*"
}

fail() {
  log "$*"
  exit 1
}

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || fail "missing required command: $1"
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
}
trap cleanup EXIT

wait_for_metadata() {
  for _ in $(seq 1 "$READY_ATTEMPTS"); do
    if curl -fsS -H "Authorization: Bearer ${API_KEY}" "${TEMPER_URL}/tdata/\$metadata" >/dev/null 2>&1; then
      return 0
    fi
    sleep 1
  done
  fail "server did not become ready at ${TEMPER_URL}"
}

require_cmd cargo
require_cmd curl
require_cmd git
require_cmd jq

mkdir -p "$PROOF_DIR"

log "repo root: ${ROOT}"
log "server: ${TEMPER_URL}"
log "proof dir: ${PROOF_DIR}"

log "building current paw-patrol WASM modules"
(cd "$ROOT/$(dirname "$PATROL_WASM_BUILD")" && bash "$(basename "$PATROL_WASM_BUILD")") \
  >"$WASM_BUILD_LOG" 2>&1

TEMPERPAW_WASM_STARTUP_POLICY=build \
PORT="$PORT" \
TEMPER_API_KEY="$API_KEY" \
PAW_TENANT="$TENANT" \
LOCAL_CODEX_WORKER_ID="$WORKER_ID" \
LOCAL_CODEX_WORKTREE_ROOT="$WORKSPACE_ROOT" \
TURSO_URL="file:${DB_PATH}" \
cargo run -p temperpaw >"$SERVER_LOG" 2>&1 &
SERVER_PID="$!"

wait_for_metadata
log "control plane ready"

TEMPER_URL="$TEMPER_URL" \
TEMPER_TENANT="$TENANT" \
WORKER_ID="$WORKER_ID" \
WORKER_TOKEN="$API_KEY" \
REPO_ROOT="$ROOT" \
WORKSPACE_ROOT="$WORKSPACE_ROOT" \
CODEX_BIN="$ROOT/crates/paw-codex-worker/fixtures/fake-codex.sh" \
PAW_CODEX_ENABLE_EXECUTION=0 \
PAW_CODEX_POLL_ON_START=1 \
cargo run -p paw-codex-worker >"$WORKER_LOG" 2>&1 &
WORKER_PID="$!"

ALLOW_PRODUCTION_WRITE=1 \
CONFIRM_PAW_CODEX_ENABLE_EXECUTION_0=1 \
TEMPER_URL="$TEMPER_URL" \
TEMPER_TENANT="$TENANT" \
PATROL_OPERATOR_TOKEN="$API_KEY" \
EXPECTED_WORKER_ID="$WORKER_ID" \
READY_ATTEMPTS=240 \
PROOF_DIR="$PROOF_DIR" \
"$ROOT/crates/paw-codex-worker/scripts/production-observe-only.sh"

cp "$SERVER_LOG" "$PROOF_DIR/server.log" || true
cp "$WORKER_LOG" "$PROOF_DIR/worker.log" || true
cp "$WASM_BUILD_LOG" "$PROOF_DIR/wasm-build.log" || true

jq -e '
  .status == "passed" and
  .statuses.worker_run == "Done" and
  .statuses.review_run == "Approved" and
  .statuses.evaluation_run == "Passed" and
  .statuses.proof_packet == "Ready" and
  .worker.execution_enabled == false
' "$PROOF_DIR/summary.json" >/dev/null

test -s "$PROOF_DIR/observe-only.svg"

log "production observe-only smoke passed"
