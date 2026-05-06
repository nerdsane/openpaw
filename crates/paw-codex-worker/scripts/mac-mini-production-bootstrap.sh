#!/usr/bin/env bash
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel)"
PROJECT_ID="ad7f8977-cf48-43ef-b129-ba1e17896ae4"
ENVIRONMENT="production"
SERVICE_ID="4a8dedaa-8a2e-4cdd-945b-e06c781bb3f0"
TEMPER_URL="${TEMPER_URL:-https://openpaw-production.up.railway.app}"
TEMPER_TENANT="${TEMPER_TENANT:-default}"
WORKER_ID="${WORKER_ID:-mac-mini-codex-prod}"
REPO_ROOT="${REPO_ROOT:-$ROOT}"
WORKSPACE_ROOT="${WORKSPACE_ROOT:-$(dirname "$ROOT")}"
CODEX_BIN="${CODEX_BIN:-$(command -v codex || true)}"
PAW_CODEX_ENABLE_EXECUTION="${PAW_CODEX_ENABLE_EXECUTION:-0}"
PAW_CODEX_DOCTOR_EXEC_SMOKE="${PAW_CODEX_DOCTOR_EXEC_SMOKE:-1}"
PAW_CODEX_POLL_ON_START="${PAW_CODEX_POLL_ON_START:-1}"
INSTALL_LAUNCHD="${INSTALL_LAUNCHD:-0}"
RUN_OBSERVE_ONLY="${RUN_OBSERVE_ONLY:-0}"

log() {
  printf '[paw-codex-mac-mini-bootstrap] %s\n' "$*"
}

fail() {
  log "$*"
  exit 1
}

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || fail "missing required command: $1"
}

require_cmd cargo
require_cmd git
require_cmd jq
require_cmd railway

computer_name="$(scutil --get ComputerName 2>/dev/null || hostname)"
if ! printf '%s' "$computer_name" | grep -qi 'mac mini'; then
  if [[ "${CONFIRM_RUN_ON_THIS_HOST:-0}" != "1" ]]; then
    fail "this host is '${computer_name}', not a Mac mini; set CONFIRM_RUN_ON_THIS_HOST=1 only if this is the intended worker host"
  fi
fi

if [[ "$WORKER_ID" != "mac-mini-codex-prod" ]]; then
  fail "WORKER_ID must be mac-mini-codex-prod for production v1"
fi

if [[ -z "$CODEX_BIN" ]]; then
  fail "codex binary not found; install/sign in to Codex on the Mac mini first"
fi
if ! command -v "$CODEX_BIN" >/dev/null 2>&1 && [[ ! -x "$CODEX_BIN" ]]; then
  fail "CODEX_BIN is not executable: ${CODEX_BIN}"
fi

log "linking Railway project openpaw-seshendranalla / production / openpaw"
railway link --project "$PROJECT_ID" --environment "$ENVIRONMENT" --service "$SERVICE_ID" --json >/dev/null

log "reading production worker token from Railway env without printing it"
WORKER_TOKEN="$(
  railway run --service openpaw --environment production sh -lc 'printf %s "$TEMPER_API_KEY"'
)"
if [[ -z "$WORKER_TOKEN" ]]; then
  fail "Railway TEMPER_API_KEY is empty; cannot render worker launchd plist"
fi

log "running readiness with execution=${PAW_CODEX_ENABLE_EXECUTION}, install_launchd=${INSTALL_LAUNCHD}"
TEMPER_URL="$TEMPER_URL" \
TEMPER_TENANT="$TEMPER_TENANT" \
WORKER_ID="$WORKER_ID" \
WORKER_TOKEN="$WORKER_TOKEN" \
REPO_ROOT="$REPO_ROOT" \
WORKSPACE_ROOT="$WORKSPACE_ROOT" \
CODEX_BIN="$CODEX_BIN" \
PAW_CODEX_ENABLE_EXECUTION="$PAW_CODEX_ENABLE_EXECUTION" \
PAW_CODEX_DOCTOR_EXEC_SMOKE="$PAW_CODEX_DOCTOR_EXEC_SMOKE" \
PAW_CODEX_POLL_ON_START="$PAW_CODEX_POLL_ON_START" \
WRITE_LAUNCHD_PLIST=1 \
INSTALL_LAUNCHD="$INSTALL_LAUNCHD" \
crates/paw-codex-worker/scripts/production-readiness.sh

if [[ "$RUN_OBSERVE_ONLY" == "1" ]]; then
  if [[ "$INSTALL_LAUNCHD" != "1" ]]; then
    fail "RUN_OBSERVE_ONLY=1 requires INSTALL_LAUNCHD=1 so the worker can claim queued WorkerRuns"
  fi
  if [[ "$PAW_CODEX_ENABLE_EXECUTION" != "0" ]]; then
    fail "RUN_OBSERVE_ONLY=1 requires PAW_CODEX_ENABLE_EXECUTION=0"
  fi

  log "running production observe-only proof"
  ALLOW_PRODUCTION_WRITE=1 \
  CONFIRM_PAW_CODEX_ENABLE_EXECUTION_0=1 \
  TEMPER_URL="$TEMPER_URL" \
  TEMPER_TENANT="$TEMPER_TENANT" \
  PATROL_OPERATOR_TOKEN="$WORKER_TOKEN" \
  EXPECTED_WORKER_ID="$WORKER_ID" \
  crates/paw-codex-worker/scripts/production-observe-only.sh
fi

log "Mac mini bootstrap gate finished"
