#!/usr/bin/env bash
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel)"
WORKER_BIN="${ROOT}/target/release/paw-codex-worker"
WORKER_ID="${WORKER_ID:-mac-mini-codex-prod}"
TEMPER_TENANT="${TEMPER_TENANT:-default}"
REPO_ROOT="${REPO_ROOT:-$ROOT}"
WORKSPACE_ROOT="${WORKSPACE_ROOT:-$(dirname "$ROOT")}"
CODEX_BIN="${CODEX_BIN:-codex}"
# Default PAW_CODEX_ENABLE_EXECUTION=0 keeps production activation in observe-only
# mode until the operator deliberately enables local Codex execution.
PAW_CODEX_ENABLE_EXECUTION="${PAW_CODEX_ENABLE_EXECUTION:-0}"
PAW_CODEX_POLL_ON_START="${PAW_CODEX_POLL_ON_START:-1}"
PAW_CODEX_DOCTOR_EXEC_SMOKE="${PAW_CODEX_DOCTOR_EXEC_SMOKE:-0}"
PAW_CODEX_EXEC_TIMEOUT_SECS="${PAW_CODEX_EXEC_TIMEOUT_SECS:-1200}"
MAX_CONCURRENT_RUNS="${MAX_CONCURRENT_RUNS:-1}"
WRITE_LAUNCHD_PLIST="${WRITE_LAUNCHD_PLIST:-0}"
INSTALL_LAUNCHD="${INSTALL_LAUNCHD:-0}"
LAUNCHD_PLIST="${LAUNCHD_PLIST:-$HOME/Library/LaunchAgents/com.temperpaw.paw-codex-worker.plist}"
REQUIRE_MAIN_ANCESTRY="${REQUIRE_MAIN_ANCESTRY:-1}"

log() {
  printf '[paw-codex-production] %s\n' "$*"
}

fail() {
  log "$*"
  exit 1
}

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || fail "missing required command: $1"
}

require_env() {
  local key="$1"
  if [[ -z "${!key:-}" ]]; then
    fail "${key} is required"
  fi
}

worker_env() {
  env \
    TEMPER_URL="$TEMPER_URL" \
    TEMPER_TENANT="$TEMPER_TENANT" \
    WORKER_ID="$WORKER_ID" \
    WORKER_TOKEN="$WORKER_TOKEN" \
    REPO_ROOT="$REPO_ROOT" \
    WORKSPACE_ROOT="$WORKSPACE_ROOT" \
    CODEX_BIN="$CODEX_BIN" \
    PAW_CODEX_ENABLE_EXECUTION="$PAW_CODEX_ENABLE_EXECUTION" \
    PAW_CODEX_POLL_ON_START="$PAW_CODEX_POLL_ON_START" \
    PAW_CODEX_DOCTOR_EXEC_SMOKE="$PAW_CODEX_DOCTOR_EXEC_SMOKE" \
    PAW_CODEX_EXEC_TIMEOUT_SECS="$PAW_CODEX_EXEC_TIMEOUT_SECS" \
    MAX_CONCURRENT_RUNS="$MAX_CONCURRENT_RUNS" \
    "$@"
}

require_cmd cargo
require_cmd git
require_env TEMPER_URL
require_env WORKER_TOKEN

if [[ "$REQUIRE_MAIN_ANCESTRY" == "1" ]]; then
  bash "${ROOT}/crates/paw-codex-worker/scripts/production-git-ancestry-guard.sh"
fi

log "repo root: ${ROOT}"
log "temper url: ${TEMPER_URL}"
log "tenant: ${TEMPER_TENANT}"
log "worker id: ${WORKER_ID}"
log "repo checkout: ${REPO_ROOT}"
log "worktree root: ${WORKSPACE_ROOT}"
log "codex binary: ${CODEX_BIN}"
log "execution enabled: ${PAW_CODEX_ENABLE_EXECUTION}"
log "codex exec smoke: ${PAW_CODEX_DOCTOR_EXEC_SMOKE}"
log "codex exec timeout seconds: ${PAW_CODEX_EXEC_TIMEOUT_SECS}"
log "building paw-codex-worker release binary"
cargo build -p paw-codex-worker --release

log "running paw-codex-worker doctor"
worker_env "$WORKER_BIN" doctor

if [[ "$WRITE_LAUNCHD_PLIST" == "1" ]]; then
  log "rendering launchd-plist to ${LAUNCHD_PLIST}"
  mkdir -p "$(dirname "$LAUNCHD_PLIST")"
  worker_env "$WORKER_BIN" launchd-plist >"$LAUNCHD_PLIST"
  chmod 600 "$LAUNCHD_PLIST"
elif [[ "$INSTALL_LAUNCHD" == "1" ]]; then
  fail "INSTALL_LAUNCHD=1 requires WRITE_LAUNCHD_PLIST=1 so the exact plist is rendered first"
fi

if [[ "$INSTALL_LAUNCHD" == "1" ]]; then
  require_cmd launchctl
  log "loading launchd agent ${LAUNCHD_PLIST}"
  launchctl bootout "gui/$(id -u)" "$LAUNCHD_PLIST" >/dev/null 2>&1 || true
  launchctl bootstrap "gui/$(id -u)" "$LAUNCHD_PLIST"
  launchctl kickstart -k "gui/$(id -u)/com.temperpaw.paw-codex-worker"
fi

log "production readiness check passed"
