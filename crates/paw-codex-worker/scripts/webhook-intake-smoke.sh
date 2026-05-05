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
  PORT="$(pick_available_port 3900 700)" || {
    printf '[paw-patrol-webhook-smoke] could not find free OData port plus implicit webhook trigger port\n' >&2
    exit 1
  }
elif ! port_is_free "$PORT" || ! port_is_free "$((PORT + 12))"; then
  printf '[paw-patrol-webhook-smoke] PORT=%s or implicit webhook trigger port %s is already in use\n' "$PORT" "$((PORT + 12))" >&2
  exit 1
fi

TEMPER_URL="${TEMPER_URL:-http://127.0.0.1:${PORT}}"
WEBHOOK_PORT="$((PORT + 12))"
WEBHOOK_URL="${WEBHOOK_URL:-http://127.0.0.1:${WEBHOOK_PORT}}"
TENANT="${TEMPER_TENANT:-patrol_webhook_smoke}"
API_KEY="${TEMPER_API_KEY:-patrol-webhook-smoke}"
DB_PATH="${DB_PATH:-/tmp/paw-patrol-webhook-smoke-${PORT}-$$.db}"
READY_ATTEMPTS="${READY_ATTEMPTS:-300}"
PROOF_DIR="${PROOF_DIR:-/tmp/paw-patrol-webhook-smoke-proof-${PORT}-$$}"
INGEST_WASM_BUILD="os-apps/paw-ingest/wasm/build.sh"
PATROL_WASM_BUILD="os-apps/paw-patrol/wasm/build.sh"

SERVER_PID=""

log() {
  printf '[paw-patrol-webhook-smoke] %s\n' "$*"
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
    if [[ "$status" == "Rejected" || "$status" == "Archived" || "$status" == "Failed" ]]; then
      log "${set}('${id}') reached terminal failure status ${status}"
      jq . <<<"$body"
      exit 1
    fi
    sleep 1
  done
  log "${set}('${id}') did not reach ${wanted}"
  curl_json "$(entity_url "$set" "$id")" | jq .
  exit 1
}

register_route() {
  local route_key="$1"
  local source_type="$2"
  local target_entity_type="$3"
  local target_action="$4"
  local route_id

  route_id="$(post_json "${TEMPER_URL}/tdata/WebhookRoutes" '{}' | jq -r '.entity_id')"
  post_json \
    "$(entity_url WebhookRoutes "$route_id")/TemperPaw.Ingest.Register" \
    "$(jq -n \
      --arg route_key "$route_key" \
      --arg source_type "$source_type" \
      --arg target_entity_type "$target_entity_type" \
      --arg target_action "$target_action" \
      '{
        route_key: $route_key,
        source_type: $source_type,
        event_filter: "*",
        target_entity_type: $target_entity_type,
        target_action: $target_action,
        webhook_secret: "",
        monitor_resolution_enabled: "false",
        dedup_enabled: "false",
        dedup_window_minutes: "60"
      }')" \
    >/dev/null
  printf '%s' "$route_id"
}

post_webhook() {
  local route_key="$1"
  local body="$2"
  local attempts="${3:-60}"
  local response

  for _ in $(seq 1 "$attempts"); do
    if response="$(curl -fsS \
      -H "Content-Type: application/json" \
      -X POST \
      "${WEBHOOK_URL}/triggers/webhook/${route_key}" \
      -d "$body" 2>/dev/null)"; then
      printf '%s' "$response"
      return 0
    fi
    sleep 1
  done

  log "webhook trigger did not accept ${route_key} at ${WEBHOOK_URL}"
  exit 1
}

write_proof_bundle() {
  local summary_json="$1"
  local request_event_body="$2"
  local datadog_event_body="$3"
  local github_event_body="$4"
  local discord_event_body="$5"
  local request_body="$6"
  local datadog_body="$7"
  local github_body="$8"
  local discord_body="$9"

  mkdir -p "$PROOF_DIR"
  printf '%s\n' "$summary_json" >"$PROOF_DIR/summary.json"
  jq . <<<"$request_event_body" >"$PROOF_DIR/request-webhook-event.json"
  jq . <<<"$datadog_event_body" >"$PROOF_DIR/datadog-webhook-event.json"
  jq . <<<"$github_event_body" >"$PROOF_DIR/github-webhook-event.json"
  jq . <<<"$discord_event_body" >"$PROOF_DIR/discord-webhook-event.json"
  jq . <<<"$request_body" >"$PROOF_DIR/patrol-request.json"
  jq . <<<"$datadog_body" >"$PROOF_DIR/datadog-signal.json"
  jq . <<<"$github_body" >"$PROOF_DIR/github-signal.json"
  jq . <<<"$discord_body" >"$PROOF_DIR/discord-signal.json"

  cat >"$PROOF_DIR/webhook-intake.svg" <<EOF
<svg xmlns="http://www.w3.org/2000/svg" width="1120" height="820" viewBox="0 0 1120 820">
  <rect width="1120" height="820" fill="#f8fafc"/>
  <text x="48" y="54" font-family="Arial, sans-serif" font-size="30" font-weight="700" fill="#0f172a">Paw Patrol Webhook Intake Smoke</text>
  <text x="48" y="88" font-family="Arial, sans-serif" font-size="16" fill="#475569">External HTTP webhook through paw-ingest into Patrol entities</text>
  <g font-family="Arial, sans-serif" font-size="16" fill="#0f172a">
    <rect x="48" y="124" width="190" height="86" rx="8" fill="#dbeafe" stroke="#2563eb" stroke-width="2"/>
    <text x="78" y="158" font-weight="700">Webhook Trigger</text>
    <text x="78" y="184">POST /patrol-request</text>
    <rect x="318" y="124" width="190" height="86" rx="8" fill="#ecfeff" stroke="#0891b2" stroke-width="2"/>
    <text x="348" y="158" font-weight="700">WebhookEvent</text>
    <text x="348" y="184">Processed</text>
    <rect x="588" y="124" width="190" height="86" rx="8" fill="#dcfce7" stroke="#16a34a" stroke-width="2"/>
    <text x="618" y="158" font-weight="700">PatrolRequest</text>
    <text x="618" y="184">Linked</text>
    <rect x="858" y="124" width="190" height="86" rx="8" fill="#fef3c7" stroke="#d97706" stroke-width="2"/>
    <text x="888" y="158" font-weight="700">FactoryCase</text>
    <text x="888" y="184">WorkCycle queued</text>
    <rect x="48" y="292" width="190" height="86" rx="8" fill="#dbeafe" stroke="#2563eb" stroke-width="2"/>
    <text x="78" y="326" font-weight="700">Webhook Trigger</text>
    <text x="78" y="352">POST /patrol-datadog</text>
    <rect x="318" y="292" width="190" height="86" rx="8" fill="#ecfeff" stroke="#0891b2" stroke-width="2"/>
    <text x="348" y="326" font-weight="700">WebhookEvent</text>
    <text x="348" y="352">Processed</text>
    <rect x="588" y="292" width="190" height="86" rx="8" fill="#dcfce7" stroke="#16a34a" stroke-width="2"/>
    <text x="618" y="326" font-weight="700">Datadog Signal</text>
    <text x="618" y="352">Linked</text>
    <rect x="858" y="292" width="190" height="86" rx="8" fill="#fef3c7" stroke="#d97706" stroke-width="2"/>
    <text x="888" y="326" font-weight="700">FactoryCase</text>
    <text x="888" y="352">WorkCycle queued</text>
    <rect x="48" y="460" width="190" height="86" rx="8" fill="#dbeafe" stroke="#2563eb" stroke-width="2"/>
    <text x="78" y="494" font-weight="700">Webhook Trigger</text>
    <text x="78" y="520">POST /patrol-github</text>
    <rect x="318" y="460" width="190" height="86" rx="8" fill="#ecfeff" stroke="#0891b2" stroke-width="2"/>
    <text x="348" y="494" font-weight="700">WebhookEvent</text>
    <text x="348" y="520">Processed</text>
    <rect x="588" y="460" width="190" height="86" rx="8" fill="#dcfce7" stroke="#16a34a" stroke-width="2"/>
    <text x="618" y="494" font-weight="700">GitHub Signal</text>
    <text x="618" y="520">Linked</text>
    <rect x="858" y="460" width="190" height="86" rx="8" fill="#fef3c7" stroke="#d97706" stroke-width="2"/>
    <text x="888" y="494" font-weight="700">FactoryCase</text>
    <text x="888" y="520">WorkCycle queued</text>
    <rect x="48" y="628" width="190" height="86" rx="8" fill="#dbeafe" stroke="#2563eb" stroke-width="2"/>
    <text x="78" y="662" font-weight="700">Webhook Trigger</text>
    <text x="78" y="688">POST /patrol-discord</text>
    <rect x="318" y="628" width="190" height="86" rx="8" fill="#ecfeff" stroke="#0891b2" stroke-width="2"/>
    <text x="348" y="662" font-weight="700">WebhookEvent</text>
    <text x="348" y="688">Processed</text>
    <rect x="588" y="628" width="190" height="86" rx="8" fill="#dcfce7" stroke="#16a34a" stroke-width="2"/>
    <text x="618" y="662" font-weight="700">Discord Signal</text>
    <text x="618" y="688">Linked</text>
    <rect x="858" y="628" width="190" height="86" rx="8" fill="#fef3c7" stroke="#d97706" stroke-width="2"/>
    <text x="888" y="662" font-weight="700">FactoryCase</text>
    <text x="888" y="688">WorkCycle queued</text>
  </g>
  <g stroke="#64748b" stroke-width="3" fill="none" marker-end="url(#arrow)">
    <defs><marker id="arrow" markerWidth="10" markerHeight="10" refX="7" refY="3" orient="auto"><path d="M0,0 L0,6 L8,3 z" fill="#64748b"/></marker></defs>
    <path d="M238 167 H318"/>
    <path d="M508 167 H588"/>
    <path d="M778 167 H858"/>
    <path d="M238 335 H318"/>
    <path d="M508 335 H588"/>
    <path d="M778 335 H858"/>
    <path d="M238 503 H318"/>
    <path d="M508 503 H588"/>
    <path d="M778 503 H858"/>
    <path d="M238 671 H318"/>
    <path d="M508 671 H588"/>
    <path d="M778 671 H858"/>
  </g>
  <text x="48" y="774" font-family="Arial, sans-serif" font-size="14" fill="#475569">Proof bundle includes request, Datadog, GitHub, and Discord WebhookEvents, target Patrol entities, OData links, server log path, and machine summary JSON.</text>
</svg>
EOF

  cat >"$PROOF_DIR/proof.md" <<EOF
# Paw Patrol Webhook Intake Smoke Proof

## Summary

The local TemperPaw webhook trigger accepted external HTTP requests, created
WebhookEvent entities, routed them through paw-ingest WASM, and produced Patrol
entities without bypassing the trigger boundary.

## State Diagram

\`\`\`mermaid
flowchart LR
  A["POST /triggers/webhook/patrol-request"] --> B["WebhookEvent Processed"]
  B --> C["PatrolRequest Linked"]
  C --> D["FactoryCase + WorkCycle"]
  E["POST /triggers/webhook/patrol-datadog"] --> F["WebhookEvent Processed"]
  F --> G["Datadog Signal Linked"]
  G --> H["FactoryCase + WorkCycle"]
  I["POST /triggers/webhook/patrol-github"] --> J["WebhookEvent Processed"]
  J --> K["GitHub Signal Linked"]
  K --> L["FactoryCase + WorkCycle"]
  M["POST /triggers/webhook/patrol-discord"] --> N["WebhookEvent Processed"]
  N --> O["Discord Signal Linked"]
  O --> P["FactoryCase + WorkCycle"]
\`\`\`

## OData Links

- Request WebhookEvent: ${TEMPER_URL}/tdata/WebhookEvents('$(jq -r '.entities.request_event' <<<"$summary_json")')
- Request PatrolRequest: ${TEMPER_URL}/tdata/PatrolRequests('$(jq -r '.entities.patrol_request' <<<"$summary_json")')
- Request FactoryCase: ${TEMPER_URL}/tdata/FactoryCases('$(jq -r '.entities.request_factory_case' <<<"$summary_json")')
- Request WorkCycle: ${TEMPER_URL}/tdata/WorkCycles('$(jq -r '.entities.request_work_cycle' <<<"$summary_json")')
- Datadog WebhookEvent: ${TEMPER_URL}/tdata/WebhookEvents('$(jq -r '.entities.datadog_event' <<<"$summary_json")')
- Datadog Signal: ${TEMPER_URL}/tdata/Signals('$(jq -r '.entities.datadog_signal' <<<"$summary_json")')
- Datadog FactoryCase: ${TEMPER_URL}/tdata/FactoryCases('$(jq -r '.entities.datadog_factory_case' <<<"$summary_json")')
- Datadog WorkCycle: ${TEMPER_URL}/tdata/WorkCycles('$(jq -r '.entities.datadog_work_cycle' <<<"$summary_json")')
- GitHub WebhookEvent: ${TEMPER_URL}/tdata/WebhookEvents('$(jq -r '.entities.github_event' <<<"$summary_json")')
- GitHub Signal: ${TEMPER_URL}/tdata/Signals('$(jq -r '.entities.github_signal' <<<"$summary_json")')
- GitHub FactoryCase: ${TEMPER_URL}/tdata/FactoryCases('$(jq -r '.entities.github_factory_case' <<<"$summary_json")')
- GitHub WorkCycle: ${TEMPER_URL}/tdata/WorkCycles('$(jq -r '.entities.github_work_cycle' <<<"$summary_json")')
- Discord WebhookEvent: ${TEMPER_URL}/tdata/WebhookEvents('$(jq -r '.entities.discord_event' <<<"$summary_json")')
- Discord Signal: ${TEMPER_URL}/tdata/Signals('$(jq -r '.entities.discord_signal' <<<"$summary_json")')
- Discord FactoryCase: ${TEMPER_URL}/tdata/FactoryCases('$(jq -r '.entities.discord_factory_case' <<<"$summary_json")')
- Discord WorkCycle: ${TEMPER_URL}/tdata/WorkCycles('$(jq -r '.entities.discord_work_cycle' <<<"$summary_json")')

## Trace And Log Evidence

- Server log: /tmp/paw-patrol-webhook-smoke-server.log
- WASM build log: /tmp/paw-patrol-webhook-smoke-wasm-build.log

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
log "odata server: ${TEMPER_URL}"
log "webhook trigger: ${WEBHOOK_URL}"

log "building current paw-ingest and paw-patrol WASM modules"
{
  (cd "$ROOT/$(dirname "$INGEST_WASM_BUILD")" && bash "$(basename "$INGEST_WASM_BUILD")")
  (cd "$ROOT/$(dirname "$PATROL_WASM_BUILD")" && bash "$(basename "$PATROL_WASM_BUILD")")
} >/tmp/paw-patrol-webhook-smoke-wasm-build.log 2>&1

TEMPERPAW_WASM_STARTUP_POLICY=build \
PORT="$PORT" \
TEMPER_API_KEY="$API_KEY" \
PAW_TENANT="$TENANT" \
TURSO_URL="file:${DB_PATH}" \
cargo run -p temperpaw >/tmp/paw-patrol-webhook-smoke-server.log 2>&1 &
SERVER_PID="$!"

wait_for_metadata
log "control plane ready"

request_route_id="$(register_route \
  patrol-request \
  patrol-request \
  PatrolRequest \
  TemperPaw.Patrol.Submit)"
datadog_route_id="$(register_route \
  patrol-datadog \
  datadog \
  Signal \
  TemperPaw.Patrol.Ingest)"
github_route_id="$(register_route \
  patrol-github \
  github \
  Signal \
  TemperPaw.Patrol.Ingest)"
discord_route_id="$(register_route \
  patrol-discord \
  discord \
  Signal \
  TemperPaw.Patrol.Ingest)"
log "registered routes ${request_route_id}, ${datadog_route_id}, ${github_route_id}, and ${discord_route_id}"

request_event_response="$(post_webhook \
  patrol-request \
  "$(jq -n '{
    source: "webhook-smoke",
    request_text: "Webhook smoke request should enter Paw Patrol and create work.",
    requester_id: "codex-webhook-smoke"
  }')")"
request_event_id="$(jq -r '.event_id' <<<"$request_event_response")"
request_event_body="$(wait_for_status WebhookEvents "$request_event_id" Processed 120)"
request_target_type="$(field target_entity_type <<<"$request_event_body")"
request_target_id="$(field target_entity_id <<<"$request_event_body")"

if [[ "$request_target_type" != "PatrolRequest" || -z "$request_target_id" ]]; then
  log "request webhook routed to unexpected target '${request_target_type}' '${request_target_id}'"
  jq . <<<"$request_event_body"
  exit 1
fi

request_body="$(wait_for_status PatrolRequests "$request_target_id" Linked 120)"
request_case_id="$(field factory_case_id <<<"$request_body")"
request_pm_issue_id="$(field pm_issue_id <<<"$request_body")"
request_case_body="$(curl_json "$(entity_url FactoryCases "$request_case_id")")"
request_work_cycle_id="$(field work_cycle_id <<<"$request_case_body")"
request_worker_run_id="$(curl_json "$(entity_url WorkCycles "$request_work_cycle_id")" | field implementer_worker_run_id)"

datadog_event_response="$(post_webhook \
  patrol-datadog \
  "$(jq -n '{
    source: "datadog",
    severity: "warning",
    title: "Webhook smoke Datadog signal",
    message: "Discord DM surfaced a trace and needs Patrol triage.",
    source_url: "https://example.invalid/datadog/webhook-smoke"
  }')")"
datadog_event_id="$(jq -r '.event_id' <<<"$datadog_event_response")"
datadog_event_body="$(wait_for_status WebhookEvents "$datadog_event_id" Processed 120)"
datadog_target_type="$(field target_entity_type <<<"$datadog_event_body")"
datadog_target_id="$(field target_entity_id <<<"$datadog_event_body")"

if [[ "$datadog_target_type" != "Signal" || -z "$datadog_target_id" ]]; then
  log "Datadog webhook routed to unexpected target '${datadog_target_type}' '${datadog_target_id}'"
  jq . <<<"$datadog_event_body"
  exit 1
fi

datadog_body="$(wait_for_status Signals "$datadog_target_id" Linked 120)"
datadog_case_id="$(field factory_case_id <<<"$datadog_body")"
datadog_case_body="$(curl_json "$(entity_url FactoryCases "$datadog_case_id")")"
datadog_work_cycle_id="$(field work_cycle_id <<<"$datadog_case_body")"
datadog_worker_run_id="$(curl_json "$(entity_url WorkCycles "$datadog_work_cycle_id")" | field implementer_worker_run_id)"

github_event_response="$(post_webhook \
  patrol-github \
  "$(jq -n '{
    source: "github",
    severity: "warning",
    title: "Webhook smoke GitHub signal",
    message: "A failing pull request check should enter Patrol as a GitHub signal.",
    source_url: "https://github.com/nerdsane/temperpaw/actions/runs/webhook-smoke"
  }')")"
github_event_id="$(jq -r '.event_id' <<<"$github_event_response")"
github_event_body="$(wait_for_status WebhookEvents "$github_event_id" Processed 120)"
github_target_type="$(field target_entity_type <<<"$github_event_body")"
github_target_id="$(field target_entity_id <<<"$github_event_body")"

if [[ "$github_target_type" != "Signal" || -z "$github_target_id" ]]; then
  log "GitHub webhook routed to unexpected target '${github_target_type}' '${github_target_id}'"
  jq . <<<"$github_event_body"
  exit 1
fi

github_body="$(wait_for_status Signals "$github_target_id" Linked 120)"
github_case_id="$(field factory_case_id <<<"$github_body")"
github_case_body="$(curl_json "$(entity_url FactoryCases "$github_case_id")")"
github_work_cycle_id="$(field work_cycle_id <<<"$github_case_body")"
github_worker_run_id="$(curl_json "$(entity_url WorkCycles "$github_work_cycle_id")" | field implementer_worker_run_id)"

discord_event_response="$(post_webhook \
  patrol-discord \
  "$(jq -n '{
    source: "discord",
    severity: "error",
    title: "Webhook smoke Discord DM signal",
    message: "A Discord DM exposed a Rust trace to the user and needs Patrol triage.",
    source_url: "discord://dm/webhook-smoke"
  }')")"
discord_event_id="$(jq -r '.event_id' <<<"$discord_event_response")"
discord_event_body="$(wait_for_status WebhookEvents "$discord_event_id" Processed 120)"
discord_target_type="$(field target_entity_type <<<"$discord_event_body")"
discord_target_id="$(field target_entity_id <<<"$discord_event_body")"

if [[ "$discord_target_type" != "Signal" || -z "$discord_target_id" ]]; then
  log "Discord webhook routed to unexpected target '${discord_target_type}' '${discord_target_id}'"
  jq . <<<"$discord_event_body"
  exit 1
fi

discord_body="$(wait_for_status Signals "$discord_target_id" Linked 120)"
discord_case_id="$(field factory_case_id <<<"$discord_body")"
discord_case_body="$(curl_json "$(entity_url FactoryCases "$discord_case_id")")"
discord_work_cycle_id="$(field work_cycle_id <<<"$discord_case_body")"
discord_worker_run_id="$(curl_json "$(entity_url WorkCycles "$discord_work_cycle_id")" | field implementer_worker_run_id)"

summary_json="$(jq -n \
  --arg request_route "$request_route_id" \
  --arg datadog_route "$datadog_route_id" \
  --arg github_route "$github_route_id" \
  --arg discord_route "$discord_route_id" \
  --arg request_event "$request_event_id" \
  --arg patrol_request "$request_target_id" \
  --arg request_factory_case "$request_case_id" \
  --arg request_pm_issue "$request_pm_issue_id" \
  --arg request_work_cycle "$request_work_cycle_id" \
  --arg request_worker_run "$request_worker_run_id" \
  --arg datadog_event "$datadog_event_id" \
  --arg datadog_signal "$datadog_target_id" \
  --arg datadog_factory_case "$datadog_case_id" \
  --arg datadog_work_cycle "$datadog_work_cycle_id" \
  --arg datadog_worker_run "$datadog_worker_run_id" \
  --arg github_event "$github_event_id" \
  --arg github_signal "$github_target_id" \
  --arg github_factory_case "$github_case_id" \
  --arg github_work_cycle "$github_work_cycle_id" \
  --arg github_worker_run "$github_worker_run_id" \
  --arg discord_event "$discord_event_id" \
  --arg discord_signal "$discord_target_id" \
  --arg discord_factory_case "$discord_case_id" \
  --arg discord_work_cycle "$discord_work_cycle_id" \
  --arg discord_worker_run "$discord_worker_run_id" \
  --arg request_event_status "$(jq -r '.status' <<<"$request_event_body")" \
  --arg patrol_request_status "$(jq -r '.status' <<<"$request_body")" \
  --arg datadog_event_status "$(jq -r '.status' <<<"$datadog_event_body")" \
  --arg datadog_status "$(jq -r '.status' <<<"$datadog_body")" \
  --arg github_event_status "$(jq -r '.status' <<<"$github_event_body")" \
  --arg github_status "$(jq -r '.status' <<<"$github_body")" \
  --arg discord_event_status "$(jq -r '.status' <<<"$discord_event_body")" \
  --arg discord_status "$(jq -r '.status' <<<"$discord_body")" \
  '{
    statuses: {
      request_webhook_event: $request_event_status,
      patrol_request: $patrol_request_status,
      datadog_webhook_event: $datadog_event_status,
      datadog_signal: $datadog_status,
      github_webhook_event: $github_event_status,
      github_signal: $github_status,
      discord_webhook_event: $discord_event_status,
      discord_signal: $discord_status
    },
    routes: {
      patrol_request: $request_route,
      patrol_datadog: $datadog_route,
      patrol_github: $github_route,
      patrol_discord: $discord_route
    },
    entities: {
      request_event: $request_event,
      patrol_request: $patrol_request,
      request_factory_case: $request_factory_case,
      request_pm_issue: $request_pm_issue,
      request_work_cycle: $request_work_cycle,
      request_worker_run: $request_worker_run,
      datadog_event: $datadog_event,
      datadog_signal: $datadog_signal,
      datadog_factory_case: $datadog_factory_case,
      datadog_work_cycle: $datadog_work_cycle,
      datadog_worker_run: $datadog_worker_run,
      github_event: $github_event,
      github_signal: $github_signal,
      github_factory_case: $github_factory_case,
      github_work_cycle: $github_work_cycle,
      github_worker_run: $github_worker_run,
      discord_event: $discord_event,
      discord_signal: $discord_signal,
      discord_factory_case: $discord_factory_case,
      discord_work_cycle: $discord_work_cycle,
      discord_worker_run: $discord_worker_run
    },
    trigger_boundary: {
      http_route: "/triggers/webhook/{route_key}",
      first_entity: "WebhookEvent",
      first_action: "TemperPaw.Ingest.Received",
      downstream: "WASM integrations route and process into Patrol"
    }
  }')"

printf '%s\n' "$summary_json"
write_proof_bundle \
  "$summary_json" \
  "$request_event_body" \
  "$datadog_event_body" \
  "$github_event_body" \
  "$discord_event_body" \
  "$request_body" \
  "$datadog_body" \
  "$github_body" \
  "$discord_body"

log "webhook intake smoke passed"
