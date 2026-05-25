#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

LAPDOG_APM_PORT="${LAPDOG_APM_PORT:-8126}"
LAPDOG_OTLP_HTTP_PORT="${LAPDOG_OTLP_HTTP_PORT:-24318}"
LAPDOG_OTLP_GRPC_PORT="${LAPDOG_OTLP_GRPC_PORT:-24317}"

PRINT_ENV=0
LAPDOG_ONLY=0
TEMPERPAW_ARGS=()

usage() {
  cat <<'EOF'
Usage: scripts/run-lapdog-local.sh [options] [--] [temperpaw-server args...]

Starts or discovers a local Lapdog agent, then runs TemperPaw with OTLP and
direct LLMObs pointed at Lapdog.

Options:
  --lapdog-only  Start/discover Lapdog and print the local endpoints.
  --print-env    Print the local observability environment, then exit.
  -h, --help     Show this help.

Environment overrides:
  LAPDOG_APM_PORT        Default: 8126
  LAPDOG_OTLP_HTTP_PORT  Default: 24318
  LAPDOG_OTLP_GRPC_PORT  Default: 24317
  LAPDOG_LLMOBS_ENDPOINT Default: http://127.0.0.1:${LAPDOG_APM_PORT}/evp_proxy/v2/api/v2/llmobs
EOF
}

while (($#)); do
  case "$1" in
    -h|--help)
      usage
      exit 0
      ;;
    --lapdog-only)
      LAPDOG_ONLY=1
      shift
      ;;
    --print-env)
      PRINT_ENV=1
      shift
      ;;
    --)
      shift
      TEMPERPAW_ARGS+=("$@")
      break
      ;;
    *)
      TEMPERPAW_ARGS+=("$1")
      shift
      ;;
  esac
done

require_command() {
  local name="$1"
  if ! command -v "$name" >/dev/null 2>&1; then
    echo "error: $name is required" >&2
    return 1
  fi
}

extract_port_from_command() {
  local flag="$1"
  local command_line="$2"
  local re

  re="${flag}=([0-9]+)"
  if [[ "$command_line" =~ $re ]]; then
    printf '%s\n' "${BASH_REMATCH[1]}"
    return 0
  fi

  re="${flag}[[:space:]]+([0-9]+)"
  if [[ "$command_line" =~ $re ]]; then
    printf '%s\n' "${BASH_REMATCH[1]}"
    return 0
  fi

  return 1
}

probe_otlp_http_port() {
  local candidate
  for candidate in "$@"; do
    [[ -n "$candidate" ]] || continue
    if curl -fsS "http://127.0.0.1:${candidate}/test/session/traces" >/dev/null 2>&1; then
      printf '%s\n' "$candidate"
      return 0
    fi
  done
  return 1
}

lapdog_status_output() {
  lapdog status 2>&1 || true
}

lapdog_pid_from_status() {
  local status="$1"
  sed -nE 's/.*pid=([0-9]+).*/\1/p' <<<"$status" | head -n 1
}

lapdog_apm_port_from_status() {
  local status="$1"
  sed -nE 's#.*http://127\.0\.0\.1:([0-9]+)/info.*#\1#p' <<<"$status" | head -n 1
}

start_lapdog_if_needed() {
  local status
  status="$(lapdog_status_output)"
  if grep -q "Lapdog running" <<<"$status"; then
    printf '%s\n' "$status" >&2
    return 0
  fi

  echo "Starting Lapdog on APM port ${LAPDOG_APM_PORT}, OTLP/HTTP ${LAPDOG_OTLP_HTTP_PORT}, OTLP/gRPC ${LAPDOG_OTLP_GRPC_PORT}..." >&2
  env PORT="$LAPDOG_APM_PORT" lapdog start \
    --port "$LAPDOG_APM_PORT" \
    --otlp-http-port "$LAPDOG_OTLP_HTTP_PORT" \
    --otlp-grpc-port "$LAPDOG_OTLP_GRPC_PORT" >&2
}

decorate_resource_attributes() {
  local attrs="${OTEL_RESOURCE_ATTRIBUTES:-}"
  local env_name="${DD_ENV:-local}"
  local required=("deployment.environment.name=${env_name}" "dd_llmobs_enabled=false")
  local item

  for item in "${required[@]}"; do
    local key="${item%%=*}"
    if [[ -z "$attrs" ]]; then
      attrs="$item"
    elif [[ "$attrs" != *"${key}="* ]]; then
      attrs="${attrs},${item}"
    fi
  done

  printf '%s\n' "$attrs"
}

warn_if_dashboard_build_missing() {
  if [[ -f "$ROOT_DIR/dashboard/build/index.html" ]]; then
    return 0
  fi

  cat >&2 <<'EOF'
warning: dashboard/build/index.html is missing.
The embedded dashboard will not open from the Rust server until you build it:
  cd dashboard && npm run build
Then restart this command so /dashboard is mounted at startup.
EOF
}

require_command lapdog
require_command curl
require_command cargo

start_lapdog_if_needed

STATUS="$(lapdog_status_output)"
LAPDOG_PID="$(lapdog_pid_from_status "$STATUS")"
DETECTED_APM_PORT="$(lapdog_apm_port_from_status "$STATUS")"
if [[ -n "$DETECTED_APM_PORT" ]]; then
  LAPDOG_APM_PORT="$DETECTED_APM_PORT"
fi

DETECTED_OTLP_HTTP_PORT=""
if [[ -n "$LAPDOG_PID" ]]; then
  LAPDOG_COMMAND="$(ps -p "$LAPDOG_PID" -o command= 2>/dev/null || true)"
  DETECTED_OTLP_HTTP_PORT="$(extract_port_from_command "--otlp-http-port" "$LAPDOG_COMMAND" || true)"
fi

if [[ -z "$DETECTED_OTLP_HTTP_PORT" ]]; then
  DETECTED_OTLP_HTTP_PORT="$(probe_otlp_http_port "$LAPDOG_OTLP_HTTP_PORT" 4318 24318 || true)"
fi

if [[ -z "$DETECTED_OTLP_HTTP_PORT" ]]; then
  echo "error: could not find Lapdog's OTLP/HTTP port." >&2
  echo "Try restarting Lapdog with: lapdog stop && LAPDOG_OTLP_HTTP_PORT=${LAPDOG_OTLP_HTTP_PORT} scripts/run-lapdog-local.sh --lapdog-only" >&2
  exit 1
fi

LAPDOG_OTLP_HTTP_PORT="$DETECTED_OTLP_HTTP_PORT"
LAPDOG_OTLP_ENDPOINT="http://127.0.0.1:${LAPDOG_OTLP_HTTP_PORT}"
LAPDOG_LLMOBS_ENDPOINT="${LAPDOG_LLMOBS_ENDPOINT:-http://127.0.0.1:${LAPDOG_APM_PORT}/evp_proxy/v2/api/v2/llmobs}"

export OTEL_ENABLED=true
export OTLP_ENDPOINT="$LAPDOG_OTLP_ENDPOINT"
export OTEL_EXPORTER_OTLP_ENDPOINT="$LAPDOG_OTLP_ENDPOINT"
export DD_SERVICE="${DD_SERVICE:-temperpaw}"
export DD_ENV="${DD_ENV:-local}"
export DD_API_KEY="${DD_API_KEY:-lapdog-local}"
export DD_LLMOBS_ENABLED="${DD_LLMOBS_ENABLED:-true}"
export DD_LLMOBS_API_ENABLED="${DD_LLMOBS_API_ENABLED:-true}"
export DD_LLMOBS_ENDPOINT="$LAPDOG_LLMOBS_ENDPOINT"
export DD_TRACE_AGENT_URL="${DD_TRACE_AGENT_URL:-http://127.0.0.1:${LAPDOG_APM_PORT}}"
export DD_TRACE_AGENT_HOST="${DD_TRACE_AGENT_HOST:-127.0.0.1}"
export DD_TRACE_AGENT_PORT="${DD_TRACE_AGENT_PORT:-$LAPDOG_APM_PORT}"
export RUST_LOG="${RUST_LOG:-info,hyper=warn,h2=warn,tonic=warn,opentelemetry=warn}"
export OTEL_RESOURCE_ATTRIBUTES="$(decorate_resource_attributes)"

if [[ "$PRINT_ENV" == "1" ]]; then
  cat <<EOF
export OTEL_ENABLED=true
export OTLP_ENDPOINT=$OTLP_ENDPOINT
export OTEL_EXPORTER_OTLP_ENDPOINT=$OTEL_EXPORTER_OTLP_ENDPOINT
export OTEL_RESOURCE_ATTRIBUTES=$OTEL_RESOURCE_ATTRIBUTES
export DD_SERVICE=$DD_SERVICE
export DD_ENV=$DD_ENV
export DD_LLMOBS_ENABLED=$DD_LLMOBS_ENABLED
export DD_LLMOBS_API_ENABLED=$DD_LLMOBS_API_ENABLED
export DD_LLMOBS_ENDPOINT=$DD_LLMOBS_ENDPOINT
export DD_TRACE_AGENT_URL=$DD_TRACE_AGENT_URL
export DD_TRACE_AGENT_HOST=$DD_TRACE_AGENT_HOST
export DD_TRACE_AGENT_PORT=$DD_TRACE_AGENT_PORT
export RUST_LOG=$RUST_LOG
EOF
  if [[ "$DD_API_KEY" == "lapdog-local" ]]; then
    echo "export DD_API_KEY=lapdog-local"
  else
    echo "# DD_API_KEY is set and will be sent only to the local Lapdog endpoint"
  fi
  exit 0
fi

cat >&2 <<EOF
Lapdog is ready.
  Hosted UI:      https://lapdog.datadoghq.com/
  Local agent:    http://127.0.0.1:${LAPDOG_APM_PORT}/info
  OTLP traces:    ${LAPDOG_OTLP_ENDPOINT}/test/session/traces
  OTLP logs:      ${LAPDOG_OTLP_ENDPOINT}/test/session/logs
  OTLP metrics:   ${LAPDOG_OTLP_ENDPOINT}/test/session/metrics
  LLMObs intake:  ${DD_LLMOBS_ENDPOINT}

TemperPaw OTLP endpoint:   ${OTLP_ENDPOINT}
TemperPaw LLMObs endpoint: ${DD_LLMOBS_ENDPOINT}
EOF

if [[ "$LAPDOG_ONLY" == "1" ]]; then
  exit 0
fi

cd "$ROOT_DIR"
warn_if_dashboard_build_missing

if ((${#TEMPERPAW_ARGS[@]})); then
  cargo run -p temperpaw --bin temperpaw-server -- "${TEMPERPAW_ARGS[@]}"
else
  cargo run -p temperpaw --bin temperpaw-server
fi
