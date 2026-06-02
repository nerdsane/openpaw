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
WORKER_SLOT_COUNT="${WORKER_SLOT_COUNT:-$MAX_CONCURRENT_RUNS}"
LAUNCHD_STALE_SLOT_SCAN_MAX="${LAUNCHD_STALE_SLOT_SCAN_MAX:-64}"
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
  local worker_id="$1"
  local launchd_label="$2"
  shift 2
  env \
    TEMPER_URL="$TEMPER_URL" \
    TEMPER_TENANT="$TEMPER_TENANT" \
    WORKER_ID="$worker_id" \
    WORKER_TOKEN="$WORKER_TOKEN" \
    REPO_ROOT="$REPO_ROOT" \
    WORKSPACE_ROOT="$WORKSPACE_ROOT" \
    CODEX_BIN="$CODEX_BIN" \
    PAW_CODEX_ENABLE_EXECUTION="$PAW_CODEX_ENABLE_EXECUTION" \
    PAW_CODEX_POLL_ON_START="$PAW_CODEX_POLL_ON_START" \
    PAW_CODEX_DOCTOR_EXEC_SMOKE="$PAW_CODEX_DOCTOR_EXEC_SMOKE" \
    PAW_CODEX_EXEC_TIMEOUT_SECS="$PAW_CODEX_EXEC_TIMEOUT_SECS" \
    MAX_CONCURRENT_RUNS="1" \
    PAW_CODEX_LAUNCHD_LABEL="$launchd_label" \
    "$@"
}

slot_worker_id() {
  local slot="$1"
  if [[ "$slot" == "1" ]]; then
    printf '%s\n' "$WORKER_ID"
  else
    printf '%s-slot-%02d\n' "$WORKER_ID" "$slot"
  fi
}

slot_launchd_label() {
  local slot="$1"
  if [[ "$WORKER_SLOT_COUNT" == "1" ]]; then
    printf '%s\n' "com.temperpaw.paw-codex-worker"
  else
    slot_launchd_label_numbered "$slot"
  fi
}

slot_launchd_label_numbered() {
  local slot="$1"
  printf 'com.temperpaw.paw-codex-worker.slot-%02d\n' "$slot"
}

slot_launchd_plist() {
  local slot="$1"
  if [[ "$WORKER_SLOT_COUNT" == "1" ]]; then
    printf '%s\n' "$LAUNCHD_PLIST"
  else
    slot_launchd_plist_numbered "$slot"
  fi
}

slot_launchd_plist_numbered() {
  local slot="$1"
  local dir
  dir="$(dirname "$LAUNCHD_PLIST")"
  printf '%s/com.temperpaw.paw-codex-worker.slot-%02d.plist\n' "$dir" "$slot"
}

launchd_bootout_agent() {
  local label="$1"
  local plist="$2"
  launchctl bootout "gui/$(id -u)/${label}" >/dev/null 2>&1 || true
  if [[ -f "$plist" ]]; then
    launchctl bootout "gui/$(id -u)" "$plist" >/dev/null 2>&1 || true
  fi
}

unload_stale_launchd_agents() {
  local max_slot="$LAUNCHD_STALE_SLOT_SCAN_MAX"
  if [[ "$WORKER_SLOT_COUNT" -gt "$max_slot" ]]; then
    max_slot="$WORKER_SLOT_COUNT"
  fi

  log "unloading stale paw-codex-worker launchd agents"
  launchd_bootout_agent "com.temperpaw.paw-codex-worker" "$LAUNCHD_PLIST"
  for slot in $(seq 1 "$max_slot"); do
    launchd_bootout_agent "$(slot_launchd_label_numbered "$slot")" "$(slot_launchd_plist_numbered "$slot")"
  done

  local launchd_dir
  launchd_dir="$(dirname "$LAUNCHD_PLIST")"
  for plist in "${launchd_dir}"/com.temperpaw.paw-codex-worker.slot-*.plist; do
    [[ -e "$plist" ]] || continue
    launchctl bootout "gui/$(id -u)" "$plist" >/dev/null 2>&1 || true
  done
}

require_cmd cargo
require_cmd git
require_env TEMPER_URL
require_env WORKER_TOKEN

if ! [[ "$WORKER_SLOT_COUNT" =~ ^[0-9]+$ ]] || [[ "$WORKER_SLOT_COUNT" -lt 1 ]]; then
  fail "WORKER_SLOT_COUNT must be a positive integer"
fi
if ! [[ "$LAUNCHD_STALE_SLOT_SCAN_MAX" =~ ^[0-9]+$ ]] || [[ "$LAUNCHD_STALE_SLOT_SCAN_MAX" -lt 1 ]]; then
  fail "LAUNCHD_STALE_SLOT_SCAN_MAX must be a positive integer"
fi

if [[ "$REQUIRE_MAIN_ANCESTRY" == "1" ]]; then
  bash "${ROOT}/crates/paw-codex-worker/scripts/production-git-ancestry-guard.sh"
fi

log "repo root: ${ROOT}"
log "temper url: ${TEMPER_URL}"
log "tenant: ${TEMPER_TENANT}"
log "worker id: ${WORKER_ID}"
log "worker slot count: ${WORKER_SLOT_COUNT}"
log "repo checkout: ${REPO_ROOT}"
log "worktree root: ${WORKSPACE_ROOT}"
log "codex binary: ${CODEX_BIN}"
log "execution enabled: ${PAW_CODEX_ENABLE_EXECUTION}"
log "codex exec smoke: ${PAW_CODEX_DOCTOR_EXEC_SMOKE}"
log "codex exec timeout seconds: ${PAW_CODEX_EXEC_TIMEOUT_SECS}"
log "building paw-codex-worker release binary"
cargo build -p paw-codex-worker --release

log "running paw-codex-worker doctor"
worker_env "$(slot_worker_id 1)" "$(slot_launchd_label 1)" "$WORKER_BIN" doctor

log "registering worker slot identities"
for slot in $(seq 1 "$WORKER_SLOT_COUNT"); do
  slot_worker="$(slot_worker_id "$slot")"
  slot_label="$(slot_launchd_label "$slot")"
  worker_env "$slot_worker" "$slot_label" "$WORKER_BIN" register-worker-agent
done

if [[ "$WRITE_LAUNCHD_PLIST" == "1" ]]; then
  for slot in $(seq 1 "$WORKER_SLOT_COUNT"); do
    slot_worker="$(slot_worker_id "$slot")"
    slot_label="$(slot_launchd_label "$slot")"
    slot_plist="$(slot_launchd_plist "$slot")"
    log "rendering launchd plist ${slot} for ${slot_worker} to ${slot_plist}"
    mkdir -p "$(dirname "$slot_plist")"
    worker_env "$slot_worker" "$slot_label" "$WORKER_BIN" launchd-plist >"$slot_plist"
    chmod 600 "$slot_plist"
  done
elif [[ "$INSTALL_LAUNCHD" == "1" ]]; then
  fail "INSTALL_LAUNCHD=1 requires WRITE_LAUNCHD_PLIST=1 so the exact plist is rendered first"
fi

if [[ "$INSTALL_LAUNCHD" == "1" ]]; then
  require_cmd launchctl
  unload_stale_launchd_agents
  for slot in $(seq 1 "$WORKER_SLOT_COUNT"); do
    slot_label="$(slot_launchd_label "$slot")"
    slot_plist="$(slot_launchd_plist "$slot")"
    log "loading launchd agent ${slot_label} from ${slot_plist}"
    launchctl bootstrap "gui/$(id -u)" "$slot_plist"
    launchctl kickstart -k "gui/$(id -u)/${slot_label}"
  done
fi

log "production readiness check passed"
