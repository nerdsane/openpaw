#!/usr/bin/env bash
set -euo pipefail

# Bootstrap the Mac mini worker under the interactive Codex user. Do not run
# this as root: Codex subscription auth and Datadog MCP auth live in openclaw.

MAC_MINI_HOST="${MAC_MINI_HOST:-openclaw@100.124.111.105}"
REMOTE_USER="${REMOTE_USER:-openclaw}"
REMOTE_HOME="/Users/${REMOTE_USER}"
REMOTE_REPO="${REMOTE_REPO:-${REMOTE_HOME}/Development/temperpaw-worktrees/paw-codex-worker-prod}"
REMOTE_WORKTREES="${REMOTE_WORKTREES:-${REMOTE_HOME}/Development/temperpaw-worktrees}"
REMOTE_ENV_DIR="${REMOTE_HOME}/.config/temperpaw"
REMOTE_ENV_FILE="${REMOTE_ENV_DIR}/paw-codex-worker.env"
REMOTE_PLIST="${REMOTE_HOME}/Library/LaunchAgents/com.temperpaw.paw-codex-worker.plist"
REMOTE_BIN="${REMOTE_REPO}/target/release/paw-codex-worker"

required_vars=(
  TEMPER_API_KEY
  DD_API_KEY
  DD_APP_KEY
  DD_SITE
  PATROL_DATADOG_WEBHOOK_SECRET
)

tmp_vars="$(mktemp)"
trap 'rm -f "${tmp_vars}"' EXIT

railway variables --json > "${tmp_vars}"

extract_var() {
  local key="$1"
  jq -r --arg key "${key}" '.[$key] // empty' "${tmp_vars}"
}

write_env_payload() {
  umask 077
  temper_url="$(extract_var TEMPER_URL)"
  if [[ -z "${temper_url}" ]]; then
    temper_url="$(extract_var RAILWAY_SERVICE_OPENPAW_URL)"
  fi
  if [[ -z "${temper_url}" ]]; then
    public_domain="$(extract_var RAILWAY_PUBLIC_DOMAIN)"
    if [[ -n "${public_domain}" ]]; then
      temper_url="https://${public_domain}"
    fi
  fi
  if [[ -n "${temper_url}" && "${temper_url}" != http://* && "${temper_url}" != https://* ]]; then
    temper_url="https://${temper_url}"
  fi
  temper_tenant="$(extract_var TEMPER_TENANT)"
  if [[ -z "${temper_tenant}" ]]; then
    temper_tenant="default"
  fi
  printf 'TEMPER_URL=%s\n' "${temper_url}"
  printf 'TEMPER_TENANT=%s\n' "${temper_tenant}"
  for key in "${required_vars[@]}"; do
    value="$(extract_var "${key}")"
    if [[ -n "${value}" ]]; then
      printf '%s=%s\n' "${key}" "${value}"
    fi
  done
  printf 'WORKER_ID=%s\n' 'mac-mini-codex-prod'
  printf 'WORKER_TOKEN=%s\n' "$(extract_var TEMPER_API_KEY)"
  printf 'WORKSPACE_ROOT=%s\n' "${REMOTE_WORKTREES}"
  printf 'REPO_ROOT=%s\n' "${REMOTE_REPO}"
  printf 'CODEX_BIN=%s\n' '/opt/homebrew/bin/codex'
  printf 'PATH=%s\n' '/opt/homebrew/bin:/opt/homebrew/sbin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:/Users/openclaw/.cargo/bin'
  printf 'PAW_CODEX_ENABLE_EXECUTION=1\n'
  printf 'PAW_CODEX_POLL_ON_START=1\n'
  printf 'PAW_CODEX_DOCTOR_EXEC_SMOKE=1\n'
  printf 'PAW_CODEX_WORKER_CAPABILITIES=%s\n' 'local_codex,repo_write,review,evaluation,datadog_query'
  printf 'PAW_CODEX_WORKER_ENV_FILE=%s\n' "${REMOTE_ENV_FILE}"
  printf 'PAW_CODEX_EVAL_COMMANDS=%s\n' 'cargo fmt --check && git diff --check && cargo test -p paw-codex-worker && cargo test -p temperpaw --test paw_patrol_foundation -- --nocapture'
}

ssh "${MAC_MINI_HOST}" "install -d -m 700 '${REMOTE_ENV_DIR}' '${REMOTE_WORKTREES}'"
write_env_payload | ssh "${MAC_MINI_HOST}" "cat > '${REMOTE_ENV_FILE}' && chmod 600 '${REMOTE_ENV_FILE}'"

ssh "${MAC_MINI_HOST}" "cd '${REMOTE_REPO}' && cargo build -p paw-codex-worker --release"
ssh "${MAC_MINI_HOST}" "cd '${REMOTE_REPO}' && PAW_CODEX_WORKER_ENV_FILE='${REMOTE_ENV_FILE}' '${REMOTE_BIN}' launchd-plist > '${REMOTE_PLIST}'"
ssh "${MAC_MINI_HOST}" "launchctl bootout gui/\$(id -u) '${REMOTE_PLIST}' >/dev/null 2>&1 || true"
ssh "${MAC_MINI_HOST}" "launchctl bootstrap gui/\$(id -u) '${REMOTE_PLIST}'"
ssh "${MAC_MINI_HOST}" "launchctl kickstart -k gui/\$(id -u)/com.temperpaw.paw-codex-worker"
ssh "${MAC_MINI_HOST}" "cd '${REMOTE_REPO}' && PAW_CODEX_WORKER_ENV_FILE='${REMOTE_ENV_FILE}' '${REMOTE_BIN}' doctor"
# Verification command above is the deployed paw-codex-worker doctor check.

echo "Mac mini paw-codex-worker bootstrap complete for ${REMOTE_USER}. Secret values were not printed."
