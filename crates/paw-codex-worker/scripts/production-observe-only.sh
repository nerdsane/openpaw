#!/usr/bin/env bash
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel)"
STAMP="$(date -u +%Y%m%dT%H%M%SZ)"
PROOF_DIR="${PROOF_DIR:-/tmp/paw-patrol-production-observe-only-${STAMP}-$$}"
SUMMARY_JSON="${PROOF_DIR}/summary.json"
PROOF_MD="${PROOF_DIR}/proof.md"
OBSERVE_SVG="${PROOF_DIR}/observe-only.svg"
TEMPER_TENANT="${TEMPER_TENANT:-default}"
PATROL_OPERATOR_ID="${PATROL_OPERATOR_ID:-paw-patrol-production-observe}"
EXPECTED_WORKER_ID="${EXPECTED_WORKER_ID:-mac-mini-codex-prod}"
COMMIT_SHA="${COMMIT_SHA:-production-observe-$(git -C "$ROOT" rev-parse --short HEAD)}"
READY_ATTEMPTS="${READY_ATTEMPTS:-900}"
START_DAILY_BRIEF="${START_DAILY_BRIEF:-1}"

log() {
  printf '[paw-patrol-observe-only] %s\n' "$*"
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

if [[ "${ALLOW_PRODUCTION_WRITE:-0}" != "1" ]]; then
  fail "ALLOW_PRODUCTION_WRITE=1 is required because this script creates a RepoGraphSnapshot and optional DailyBrief"
fi

if [[ "${CONFIRM_PAW_CODEX_ENABLE_EXECUTION_0:-0}" != "1" ]]; then
  fail "CONFIRM_PAW_CODEX_ENABLE_EXECUTION_0=1 is required; the Mac mini worker must stay in PAW_CODEX_ENABLE_EXECUTION=0 observe-only mode for this gate"
fi

require_cmd curl
require_cmd git
require_cmd jq
require_env TEMPER_URL

PATROL_OPERATOR_TOKEN="${PATROL_OPERATOR_TOKEN:-${TEMPER_API_KEY:-}}"
if [[ -z "$PATROL_OPERATOR_TOKEN" ]]; then
  fail "PATROL_OPERATOR_TOKEN is required; use an operator/system token that can create RepoGraphSnapshots and DailyBriefs"
fi

mkdir -p "$PROOF_DIR"

curl_json() {
  curl -fsS \
    -H "Authorization: Bearer ${PATROL_OPERATOR_TOKEN}" \
    -H "Content-Type: application/json" \
    -H "x-tenant-id: ${TEMPER_TENANT}" \
    -H "x-temper-principal-kind: agent" \
    -H "x-temper-principal-id: ${PATROL_OPERATOR_ID}" \
    -H "x-temper-agent-type: system" \
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
    if curl_json "${TEMPER_URL}/tdata/\$metadata" >/dev/null 2>&1; then
      return 0
    fi
    sleep 1
  done
  fail "control plane did not become readable at ${TEMPER_URL}"
}

wait_for_status() {
  local set="$1"
  local id="$2"
  local wanted="$3"
  local attempts="${4:-$READY_ATTEMPTS}"
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
  log "${set}('${id}') did not reach ${wanted}; latest state follows"
  curl_json "$(entity_url "$set" "$id")" | tee "${PROOF_DIR}/${set}-${id}-timeout.json" | jq .
  exit 1
}

wait_for_field() {
  local set="$1"
  local id="$2"
  local field_name="$3"
  local attempts="${4:-$READY_ATTEMPTS}"
  local body value
  for _ in $(seq 1 "$attempts"); do
    body="$(curl_json "$(entity_url "$set" "$id")")"
    value="$(field "$field_name" <<<"$body")"
    if [[ -n "$value" ]]; then
      printf '%s' "$body"
      return 0
    fi
    sleep 1
  done
  log "${set}('${id}') did not populate ${field_name}; latest state follows"
  curl_json "$(entity_url "$set" "$id")" | tee "${PROOF_DIR}/${set}-${id}-missing-${field_name}.json" | jq .
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

write_observe_svg() {
  local summary_json="$1"
  local status worker_status review_status evaluation_status proof_status brief_status finding_count
  status="$(jq -r '.status' <<<"$summary_json")"
  worker_status="$(jq -r '.statuses.worker_run' <<<"$summary_json")"
  review_status="$(jq -r '.statuses.review_run' <<<"$summary_json")"
  evaluation_status="$(jq -r '.statuses.evaluation_run' <<<"$summary_json")"
  proof_status="$(jq -r '.statuses.proof_packet' <<<"$summary_json")"
  brief_status="$(jq -r '.statuses.daily_brief' <<<"$summary_json")"
  finding_count="$(jq -r '.repo_sweep.finding_count' <<<"$summary_json")"

  cat >"$OBSERVE_SVG" <<EOF
<svg xmlns="http://www.w3.org/2000/svg" width="960" height="540" viewBox="0 0 960 540" role="img" aria-labelledby="title desc">
  <title id="title">Paw Patrol production observe-only proof</title>
  <desc id="desc">Factual observe-only proof visual generated from summary.json.</desc>
  <rect width="960" height="540" fill="#f7f5ef"/>
  <rect x="40" y="36" width="880" height="468" rx="8" fill="#ffffff" stroke="#d5d2c6"/>
  <text x="70" y="86" font-family="ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, Segoe UI, sans-serif" font-size="30" font-weight="700" fill="#202124">Paw Patrol Observe-Only Proof</text>
  <text x="70" y="122" font-family="ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, Segoe UI, sans-serif" font-size="16" fill="#64615a">RepoGraphSnapshot.StartScan with PAW_CODEX_ENABLE_EXECUTION=0, independent review, evaluation, ProofPacket, and DailyBrief.</text>
  <rect x="70" y="158" width="220" height="112" rx="8" fill="#edf4ee" stroke="#b7d3bc"/>
  <text x="94" y="198" font-family="ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, Segoe UI, sans-serif" font-size="15" fill="#64615a">Overall Status</text>
  <text x="94" y="238" font-family="ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, Segoe UI, sans-serif" font-size="34" font-weight="700" fill="#137333">${status}</text>
  <rect x="320" y="158" width="160" height="112" rx="8" fill="#f4f7fb" stroke="#b8c7dc"/>
  <text x="344" y="198" font-family="ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, Segoe UI, sans-serif" font-size="15" fill="#64615a">Findings</text>
  <text x="344" y="238" font-family="ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, Segoe UI, sans-serif" font-size="34" font-weight="700" fill="#174ea6">${finding_count}</text>
  <rect x="510" y="158" width="350" height="112" rx="8" fill="#fffaf0" stroke="#e2cf9a"/>
  <text x="534" y="198" font-family="ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, Segoe UI, sans-serif" font-size="15" fill="#64615a">Allowed Worker</text>
  <text x="534" y="238" font-family="ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, Segoe UI, sans-serif" font-size="22" font-weight="700" fill="#202124">${EXPECTED_WORKER_ID}</text>
  <text x="70" y="328" font-family="ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, Segoe UI, sans-serif" font-size="18" font-weight="700" fill="#202124">Gate Chain</text>
  <text x="70" y="364" font-family="ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, Segoe UI, sans-serif" font-size="16" fill="#202124">WorkerRun ${worker_status} -> ReviewRun ${review_status} -> EvaluationRun ${evaluation_status} -> ProofPacket ${proof_status} -> DailyBrief ${brief_status}</text>
  <text x="70" y="420" font-family="ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, Segoe UI, sans-serif" font-size="18" font-weight="700" fill="#202124">Evidence</text>
  <text x="70" y="454" font-family="ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, Segoe UI, sans-serif" font-size="16" fill="#202124">See summary.json, proof.md, proof-packet.svg, daily-brief.svg, and OData links in this proof bundle.</text>
  <text x="70" y="484" font-family="ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, Segoe UI, sans-serif" font-size="13" fill="#64615a">Source: ${SUMMARY_JSON}</text>
</svg>
EOF
}

write_proof_bundle() {
  local summary_json="$1"
  local snapshot_body="$2"
  local proof_body="$3"
  local brief_body="$4"
  local graph_json proof_json proof_markdown proof_visual brief_visual brief_summary

  printf '%s\n' "$summary_json" >"$SUMMARY_JSON"
  jq . <<<"$snapshot_body" >"$PROOF_DIR/repo-graph-snapshot.json"
  jq . <<<"$proof_body" >"$PROOF_DIR/proof-packet.json"
  jq . <<<"$brief_body" >"$PROOF_DIR/daily-brief.json"

  graph_json="$(field graph_json <<<"$snapshot_body")"
  if jq -e . >/dev/null 2>&1 <<<"$graph_json"; then
    jq . <<<"$graph_json" >"$PROOF_DIR/repo-graph.json"
  else
    printf '%s\n' "$graph_json" >"$PROOF_DIR/repo-graph.json"
  fi

  proof_json="$(field proof_json <<<"$proof_body")"
  if jq -e . >/dev/null 2>&1 <<<"$proof_json"; then
    jq . <<<"$proof_json" >"$PROOF_DIR/proof.json"
  else
    printf '%s\n' "$proof_json" >"$PROOF_DIR/proof.json"
  fi

  proof_markdown="$(field summary_markdown <<<"$proof_body")"
  proof_visual="$(field visual_summary_url <<<"$proof_body")"
  brief_visual="$(field visual_summary_url <<<"$brief_body")"
  brief_summary="$(field summary_markdown <<<"$brief_body")"

  decode_svg_data_uri "$proof_visual" "$PROOF_DIR/proof-packet.svg"
  decode_svg_data_uri "$brief_visual" "$PROOF_DIR/daily-brief.svg"
  write_observe_svg "$summary_json"

  cat >"$PROOF_MD" <<EOF
# Paw Patrol Production Observe-Only Proof

This proof was created by \`production-observe-only.sh\` after the operator
confirmed \`PAW_CODEX_ENABLE_EXECUTION=0\` for the Mac mini worker. It creates a
low-risk \`RepoGraphSnapshot\`, dispatches \`TemperPaw.Patrol.StartScan\`, waits
for the registered local worker, independent reviewer, evaluation gate, final
\`ProofPacket\`, and optional \`DailyBrief\`.

## Repo Sweep Proof

${proof_markdown}

## Daily Brief

${brief_summary}

## OData Links

- RepoGraphSnapshot: ${TEMPER_URL}/tdata/RepoGraphSnapshots('$(jq -r '.entities.repo_graph_snapshot' <<<"$summary_json")')
- WorkCycle: ${TEMPER_URL}/tdata/WorkCycles('$(jq -r '.entities.work_cycle' <<<"$summary_json")')
- WorkerRun: ${TEMPER_URL}/tdata/WorkerRuns('$(jq -r '.entities.worker_run' <<<"$summary_json")')
- ReviewRun: ${TEMPER_URL}/tdata/ReviewRuns('$(jq -r '.entities.review_run' <<<"$summary_json")')
- EvaluationRun: ${TEMPER_URL}/tdata/EvaluationRuns('$(jq -r '.entities.evaluation_run' <<<"$summary_json")')
- ProofPacket: ${TEMPER_URL}/tdata/ProofPackets('$(jq -r '.entities.proof_packet' <<<"$summary_json")')
- DailyBrief: ${TEMPER_URL}/tdata/DailyBriefs('$(jq -r '.entities.daily_brief' <<<"$summary_json")')

## Visual Evidence

- Observe-only summary: ${OBSERVE_SVG}
- ProofPacket visual: ${PROOF_DIR}/proof-packet.svg
- DailyBrief visual: ${PROOF_DIR}/daily-brief.svg

## Machine Summary

\`\`\`json
${summary_json}
\`\`\`
EOF

  log "proof bundle: ${PROOF_DIR}"
}

require_cmd curl
require_cmd git
require_cmd jq

log "control plane: ${TEMPER_URL}"
log "tenant: ${TEMPER_TENANT}"
log "proof dir: ${PROOF_DIR}"
log "expected worker: ${EXPECTED_WORKER_ID}"
log "PAW_CODEX_ENABLE_EXECUTION=0 confirmed by operator"

wait_for_metadata

snapshot_id="$(post_json "${TEMPER_URL}/tdata/RepoGraphSnapshots" '{}' | jq -r '.entity_id')"
post_json \
  "$(entity_url RepoGraphSnapshots "$snapshot_id")/TemperPaw.Patrol.StartScan" \
  "$(jq -n --arg commit "$COMMIT_SHA" '{commit_sha:$commit}')" \
  >/dev/null

snapshot_scanning_body="$(wait_for_field RepoGraphSnapshots "$snapshot_id" worker_run_id 180)"
work_cycle_id="$(field work_cycle_id <<<"$snapshot_scanning_body")"
worker_run_id="$(field worker_run_id <<<"$snapshot_scanning_body")"
worker_body_initial="$(curl_json "$(entity_url WorkerRuns "$worker_run_id")")"
allowed_worker_id="$(field allowed_worker_id <<<"$worker_body_initial")"

if [[ "$allowed_worker_id" != "$EXPECTED_WORKER_ID" ]]; then
  fail "WorkerRun allowed_worker_id was '${allowed_worker_id}', expected '${EXPECTED_WORKER_ID}'"
fi

log "RepoGraphSnapshot ${snapshot_id}"
log "WorkCycle ${work_cycle_id}"
log "WorkerRun ${worker_run_id}"

worker_done_body="$(wait_for_status WorkerRuns "$worker_run_id" Done "$READY_ATTEMPTS")"
snapshot_ready_body="$(wait_for_status RepoGraphSnapshots "$snapshot_id" Ready 180)"
work_cycle_body="$(wait_for_status WorkCycles "$work_cycle_id" Complete 240)"

review_run_id="$(field reviewer_run_id <<<"$work_cycle_body")"
evaluation_run_id="$(field evaluation_run_id <<<"$work_cycle_body")"
proof_packet_id="$(field proof_packet_id <<<"$work_cycle_body")"

review_body="$(wait_for_status ReviewRuns "$review_run_id" Approved 60)"
evaluation_body="$(wait_for_status EvaluationRuns "$evaluation_run_id" Passed 60)"
proof_body="$(wait_for_status ProofPackets "$proof_packet_id" Ready 60)"

daily_brief_id=""
daily_brief_body='{"status":"Skipped","fields":{"summary_markdown":"DailyBrief was skipped by START_DAILY_BRIEF=0.","visual_summary_url":""}}'
if [[ "$START_DAILY_BRIEF" == "1" ]]; then
  daily_brief_id="$(post_json "${TEMPER_URL}/tdata/DailyBriefs" '{}' | jq -r '.entity_id')"
  post_json \
    "$(entity_url DailyBriefs "$daily_brief_id")/TemperPaw.Patrol.Start" \
    "$(jq -n --arg brief_date "$(date +%F)" '{brief_date:$brief_date}')" \
    >/dev/null
  daily_brief_body="$(wait_for_status DailyBriefs "$daily_brief_id" Ready 90)"
fi

summary_json="$(jq -n \
  --arg repo_graph_snapshot "$snapshot_id" \
  --arg work_cycle "$work_cycle_id" \
  --arg worker_run "$worker_run_id" \
  --arg review_run "$review_run_id" \
  --arg evaluation_run "$evaluation_run_id" \
  --arg proof_packet "$proof_packet_id" \
  --arg daily_brief "$daily_brief_id" \
  --arg snapshot_status "$(jq -r '.status' <<<"$snapshot_ready_body")" \
  --arg work_cycle_status "$(jq -r '.status' <<<"$work_cycle_body")" \
  --arg worker_status "$(jq -r '.status' <<<"$worker_done_body")" \
  --arg review_status "$(jq -r '.status' <<<"$review_body")" \
  --arg evaluation_status "$(jq -r '.status' <<<"$evaluation_body")" \
  --arg proof_status "$(jq -r '.status' <<<"$proof_body")" \
  --arg daily_brief_status "$(jq -r '.status' <<<"$daily_brief_body")" \
  --arg finding_count "$(field finding_count <<<"$snapshot_ready_body")" \
  --arg allowed_worker "$allowed_worker_id" \
  --arg expected_worker "$EXPECTED_WORKER_ID" \
  --arg commit_sha "$COMMIT_SHA" \
  --arg temper_url "$TEMPER_URL" \
  '{
    status: "passed",
    control_plane: {
      temper_url: $temper_url
    },
    statuses: {
      repo_graph_snapshot: $snapshot_status,
      work_cycle: $work_cycle_status,
      worker_run: $worker_status,
      review_run: $review_status,
      evaluation_run: $evaluation_status,
      proof_packet: $proof_status,
      daily_brief: $daily_brief_status
    },
    entities: {
      repo_graph_snapshot: $repo_graph_snapshot,
      work_cycle: $work_cycle,
      worker_run: $worker_run,
      review_run: $review_run,
      evaluation_run: $evaluation_run,
      proof_packet: $proof_packet,
      daily_brief: $daily_brief
    },
    repo_sweep: {
      commit_sha: $commit_sha,
      finding_count: $finding_count
    },
    worker: {
      expected_worker_id: $expected_worker,
      allowed_worker_id: $allowed_worker,
      execution_enabled: false
    },
    guardrails: {
      allow_production_write_required: true,
      confirmed_paw_codex_execution_zero: true
    }
  }')"

printf '%s\n' "$summary_json"
write_proof_bundle "$summary_json" "$snapshot_ready_body" "$proof_body" "$daily_brief_body"

log "production observe-only proof passed"
