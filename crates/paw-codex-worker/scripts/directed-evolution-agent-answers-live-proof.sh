#!/usr/bin/env bash
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel)"
STAMP="$(date -u +%Y%m%dT%H%M%SZ)"
PROOF_DIR="${PROOF_DIR:-/tmp/directed-evolution-agent-answers-live-proof-${STAMP}-$$}"
SUMMARY_JSON="${PROOF_DIR}/summary.json"
PROOF_MD="${PROOF_DIR}/proof.md"
HUMAN_BLOCKERS_JSON="${PROOF_DIR}/human-blockers.json"
BLOCKERS_TSV="${PROOF_DIR}/human-blockers.tsv"
CONTRACT_JSON="${PROOF_DIR}/agent-answers-episode-contract.json"
START_JSON="${PROOF_DIR}/episode-start.json"
WORKER_LOG="${PROOF_DIR}/local-worker.log"

TEMPER_TENANT="${TEMPER_TENANT:-default}"
WORKER_ID="${WORKER_ID:-mac-mini-codex-prod}"
WORKER_BIN="${WORKER_BIN:-${ROOT}/target/release/paw-codex-worker}"
REPO_ROOT="${REPO_ROOT:-$ROOT}"
WORKSPACE_ROOT="${WORKSPACE_ROOT:-$(dirname "$ROOT")}"
CODEX_BIN="${CODEX_BIN:-codex}"
BUILD_WORKER="${BUILD_WORKER:-1}"
START_LOCAL_WORKER="${START_LOCAL_WORKER:-0}"
PAW_CODEX_ENABLE_EXECUTION="${PAW_CODEX_ENABLE_EXECUTION:-1}"
PAW_CODEX_POLL_ON_START="${PAW_CODEX_POLL_ON_START:-1}"
MAX_CONCURRENT_RUNS="${MAX_CONCURRENT_RUNS:-3}"
READY_ATTEMPTS="${READY_ATTEMPTS:-900}"
POLL_SECONDS="${POLL_SECONDS:-2}"
REQUIRE_DATADOG_EVIDENCE="${REQUIRE_DATADOG_EVIDENCE:-1}"
REQUIRE_EPISODE_SUCCESS="${REQUIRE_EPISODE_SUCCESS:-1}"
DIRECTED_EVOLUTION_PROOF_ACTOR="${DIRECTED_EVOLUTION_PROOF_ACTOR:-codex-agent-answers-live-proof}"

WORKER_PID=""

log() {
  printf '[directed-evolution-agent-answers-proof] %s\n' "$*"
}

fail() {
  log "$*"
  exit 1
}

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || fail "missing required command: $1"
}

add_blocker() {
  local gate="$1"
  local detail="$2"
  local evidence="$3"
  printf '%s\t%s\t%s\n' "$gate" "$detail" "$evidence" >>"$BLOCKERS_TSV"
}

cleanup() {
  if [[ -n "$WORKER_PID" ]] && kill -0 "$WORKER_PID" >/dev/null 2>&1; then
    kill "$WORKER_PID" >/dev/null 2>&1 || true
    wait "$WORKER_PID" >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT

mkdir -p "$PROOF_DIR"
: >"$BLOCKERS_TSV"

require_cmd curl
require_cmd git
require_cmd jq

record_precondition_blockers() {
  if [[ "${ALLOW_PRODUCTION_WRITE:-0}" != "1" ]]; then
    add_blocker "confirm:allow_production_write" \
      "ALLOW_PRODUCTION_WRITE=1 is required because this script creates a live Directed Evolution Episode." \
      "operator confirmation required"
  fi

  if [[ "${CONFIRM_AGENT_ANSWERS_LIVE_PROOF:-0}" != "1" ]]; then
    add_blocker "confirm:agent_answers_live_proof" \
      "CONFIRM_AGENT_ANSWERS_LIVE_PROOF=1 is required to run a live Agent Answers Directed Evolution proof cycle." \
      "operator confirmation required"
  fi

  if [[ -z "${TEMPER_URL:-}" ]]; then
    add_blocker "env:temper_url" \
      "TEMPER_URL is required for the production TemperPaw control plane." \
      "human input required"
  fi

  if [[ -z "${WORKER_TOKEN:-}" ]]; then
    add_blocker "env:worker_token" \
      "WORKER_TOKEN is required for the episode starter and optional local worker." \
      "mint or provide a Temper worker credential"
  fi

  if [[ -z "${DIRECTED_EVOLUTION_DIRECTION_ID:-}" ]]; then
    add_blocker "env:directed_evolution_direction_id" \
      "DIRECTED_EVOLUTION_DIRECTION_ID is required and must point at the fresh Agent Answers direction under review." \
      "create or select the production Direction first"
  fi

  if [[ -z "${DIRECTED_EVOLUTION_ORGANISM_ID:-}" ]]; then
    add_blocker "env:directed_evolution_organism_id" \
      "DIRECTED_EVOLUTION_ORGANISM_ID is required and must point at the Agent Answers organism." \
      "create or select the production Organism first"
  fi

  if [[ "$START_LOCAL_WORKER" == "1" && "$PAW_CODEX_ENABLE_EXECUTION" != "1" ]]; then
    add_blocker "env:paw_codex_enable_execution" \
      "START_LOCAL_WORKER=1 requires PAW_CODEX_ENABLE_EXECUTION=1 so Codex can execute variant/reviewer/evaluator work." \
      "set PAW_CODEX_ENABLE_EXECUTION=1"
  fi

  if [[ "$REQUIRE_DATADOG_EVIDENCE" != "1" ]]; then
    add_blocker "env:require_datadog_evidence" \
      "REQUIRE_DATADOG_EVIDENCE must remain 1; Datadog-measured evidence is mandatory for this proof." \
      "do not disable the Datadog observer/evaluator gate"
  fi

  if [[ "$REQUIRE_DATADOG_EVIDENCE" == "1" ]]; then
    if [[ "$START_LOCAL_WORKER" == "1" ]]; then
      if [[ -z "${DD_API_KEY:-${DATADOG_API_KEY:-}}" ]]; then
        add_blocker "env:dd_api_key" \
          "START_LOCAL_WORKER=1 requires DD_API_KEY or DATADOG_API_KEY so Datadog observer/evaluator WorkItems can query live telemetry." \
          "export the Datadog API key before starting the local proof worker"
      fi

      if [[ -z "${DD_APP_KEY:-}" ]]; then
        add_blocker "env:dd_app_key" \
          "START_LOCAL_WORKER=1 requires DD_APP_KEY so Datadog observer/evaluator WorkItems can query live telemetry." \
          "export the Datadog application key before starting the local proof worker"
      fi
    elif [[ "${CONFIRM_PRODUCTION_DATADOG_QUERY_SECRETS:-0}" != "1" ]]; then
      add_blocker "confirm:production_datadog_query_secrets" \
        "START_LOCAL_WORKER=0 expects an already-running production worker pool; confirm it has dd_api_key and dd_app_key secrets before starting the live proof." \
        "set CONFIRM_PRODUCTION_DATADOG_QUERY_SECRETS=1 after verifying the production worker/vault Datadog query credentials"
    fi
  fi

  if [[ "$REQUIRE_EPISODE_SUCCESS" != "1" ]]; then
    add_blocker "env:require_episode_success" \
      "REQUIRE_EPISODE_SUCCESS must remain 1; failed/cancelled/abandoned episodes cannot be reported as a passed proof." \
      "do not disable the terminal-success gate"
  fi
}

write_blocked_summary_and_exit() {
  if [[ ! -s "$BLOCKERS_TSV" ]]; then
    return 0
  fi

  jq -Rn '
    [inputs | split("\t") | {
      gate: .[0],
      detail: .[1],
      evidence: .[2]
    }]
  ' <"$BLOCKERS_TSV" >"$HUMAN_BLOCKERS_JSON"
  jq -n \
    --arg status "blocked" \
    --arg proof_dir "$PROOF_DIR" \
    --argjson blockers "$(cat "$HUMAN_BLOCKERS_JSON")" \
    '{
      status: $status,
      proof_dir: $proof_dir,
      proof_kind: "live Agent Answers Directed Evolution proof",
      human_blockers: $blockers
    }' >"$SUMMARY_JSON"
  cat >"$PROOF_MD" <<EOF
# Agent Answers Directed Evolution Live Proof Blocked

This file is generated by \`directed-evolution-agent-answers-live-proof.sh\`.
The script did not create or mutate production entities because required
operator confirmations or credentials were missing.

## Human Blockers

\`\`\`json
$(cat "$HUMAN_BLOCKERS_JSON")
\`\`\`

## Machine Summary

\`\`\`json
$(cat "$SUMMARY_JSON")
\`\`\`
EOF
  log "blocked; proof bundle: ${PROOF_DIR}"
  exit 1
}

curl_json() {
  curl -fsS \
    -H "Authorization: Bearer ${WORKER_TOKEN}" \
    -H "Content-Type: application/json" \
    -H "x-tenant-id: ${TEMPER_TENANT}" \
    -H "x-temper-principal-kind: agent" \
    -H "x-temper-principal-id: ${DIRECTED_EVOLUTION_PROOF_ACTOR}" \
    -H "x-temper-agent-type: codex" \
    "$@"
}

entity_url() {
  local set="$1"
  local id="$2"
  printf "%s/tdata/%s('%s')" "$TEMPER_URL" "$set" "$id"
}

wait_for_metadata() {
  for _ in $(seq 1 "$READY_ATTEMPTS"); do
    if curl_json "${TEMPER_URL}/tdata/\$metadata" >/dev/null 2>&1; then
      return 0
    fi
    sleep "$POLL_SECONDS"
  done
  fail "control plane did not become readable at ${TEMPER_URL}"
}

field() {
  local key="$1"
  jq -r --arg key "$key" '
    def norm: ascii_downcase | gsub("_"; "");
    . as $root
    | ($root[$key]
      // $root[($key | ascii_downcase)]
      // ($root | to_entries[]? | select(.key | norm == ($key | norm)) | .value)
      // .fields[$key]
      // .fields[($key | ascii_downcase)]
      // (.fields // {} | to_entries[]? | select(.key | norm == ($key | norm)) | .value)
      // "")
  '
}

list_set() {
  local set="$1"
  if [[ "${SCRIPT_SELF_TEST:-0}" == "1" ]]; then
    cat "${SCRIPT_SELF_TEST_FIXTURE_DIR}/${set}.json"
    return 0
  fi
  curl_json "${TEMPER_URL}/tdata/${set}"
}

count_for_episode() {
  local set="$1"
  local episode_id="$2"
  list_set "$set" | jq -r --arg episode_id "$episode_id" '
    def rows:
      if type == "array" then .
      elif has("value") then .value
      elif has("entities") then .entities
      elif has("items") then .items
      else []
      end;
    def f($name):
      .[$name]
      // .[($name | ascii_downcase)]
      // .fields[$name]
      // .fields[($name | ascii_downcase)]
      // "";
    def present($value):
      $value != null and ($value | tostring | length > 0);
    def corr:
      (f("CorrelationJson") | fromjson? // {});
    [rows[]? | select(
      f("EpisodeId") == $episode_id
      or f("episode_id") == $episode_id
      or corr.episode_id == $episode_id
      or corr.datadog.join_fields.episode_id == $episode_id
      or corr.output.episode_id == $episode_id
    )] | length
  '
}

datadog_evidence_count() {
  local episode_id="$1"
  list_set EvidenceArtifacts | jq -r --arg episode_id "$episode_id" '
    def rows:
      if type == "array" then .
      elif has("value") then .value
      elif has("entities") then .entities
      elif has("items") then .items
      else []
      end;
    def f($name):
      .[$name]
      // .[($name | ascii_downcase)]
      // .fields[$name]
      // .fields[($name | ascii_downcase)]
      // "";
    def present($value):
      $value != null and ($value | tostring | length > 0);
    def corr:
      (f("CorrelationJson") | fromjson? // {});
    def belongs_to_episode:
      f("EpisodeId") == $episode_id
      or f("episode_id") == $episode_id
      or corr.episode_id == $episode_id
      or corr.datadog.join_fields.episode_id == $episode_id
      or corr.output.episode_id == $episode_id;
    def datadog_url:
      if (f("Uri") | tostring | test("^https://app\\."))
      then (f("Uri") | tostring)
      else ((corr.output.evidence_scope // corr.output.evidenceScope // [])
        | map(.datadog_url // .datadogUrl // "")
        | map(select(test("^https://app\\.")))
        | .[0])
      end
      // "";
    [rows[]? | select(
      belongs_to_episode
      and
      f("EvidenceProvenance") == "datadog-measured"
      and present(f("Query"))
      and present(f("TimeWindow"))
      and present(f("ResultCount"))
      and present(f("Interpretation"))
      and present(f("ZeroResultMeaning"))
      and (datadog_url | test("^https://app\\."))
    )] | length
  '
}

wait_for_episode_terminal() {
  local episode_id="$1"
  local body status
  for _ in $(seq 1 "$READY_ATTEMPTS"); do
    body="$(curl_json "$(entity_url Episodes "$episode_id")")"
    status="$(field Status <<<"$body")"
    case "$status" in
      Completed|Complete|Succeeded|Promoted|NoPromotion|Failed|Cancelled|Abandoned)
        printf '%s' "$body"
        return 0
        ;;
    esac
    sleep "$POLL_SECONDS"
  done
  log "Episodes('${episode_id}') did not reach a terminal status; latest state follows"
  curl_json "$(entity_url Episodes "$episode_id")" | tee "${PROOF_DIR}/episode-timeout.json" | jq .
  exit 1
}

wait_for_min_count() {
  local label="$1"
  local set="$2"
  local episode_id="$3"
  local minimum="$4"
  local count
  for _ in $(seq 1 "$READY_ATTEMPTS"); do
    count="$(count_for_episode "$set" "$episode_id")"
    if [[ "$count" -ge "$minimum" ]]; then
      printf '%s\n' "$count"
      return 0
    fi
    sleep "$POLL_SECONDS"
  done
  fail "${label} did not reach count >= ${minimum} for episode ${episode_id}"
}

wait_for_datadog_evidence() {
  local episode_id="$1"
  local count
  for _ in $(seq 1 "$READY_ATTEMPTS"); do
    count="$(datadog_evidence_count "$episode_id")"
    if [[ "$count" -ge 1 ]]; then
      printf '%s\n' "$count"
      return 0
    fi
    sleep "$POLL_SECONDS"
  done
  fail "no datadog-measured EvidenceArtifacts with datadog_url/query/window/result_count/interpretation/zero_result_meaning were observed"
}

run_self_test() {
  SCRIPT_SELF_TEST_FIXTURE_DIR="${PROOF_DIR}/self-test-fixtures"
  export SCRIPT_SELF_TEST_FIXTURE_DIR
  mkdir -p "$SCRIPT_SELF_TEST_FIXTURE_DIR"

  cat >"${SCRIPT_SELF_TEST_FIXTURE_DIR}/EvidenceArtifacts.json" <<'JSON'
{
  "value": [
    {
      "fields": {
        "EpisodeId": "episode-self-test",
        "EvidenceProvenance": "datadog-measured",
        "Uri": "https://app.datadoghq.com/logs?query=directed-evolution",
        "Query": "service:temper-platform directed_evolution.episode_id:episode-self-test",
        "TimeWindow": "now-15m to now",
        "ResultCount": 0,
        "Interpretation": "No runtime request logs were observed for the fixture.",
        "ZeroResultMeaning": "failure"
      }
    },
    {
      "fields": {
        "EpisodeId": "other-episode",
        "EvidenceProvenance": "datadog-measured",
        "Uri": "https://app.datadoghq.com/logs?query=other",
        "Query": "other",
        "TimeWindow": "now-15m to now",
        "ResultCount": 7,
        "Interpretation": "Different episode.",
        "ZeroResultMeaning": "neutral"
      }
    }
  ]
}
JSON

  local count
  count="$(datadog_evidence_count "episode-self-test")"
  if [[ "$count" != "1" ]]; then
    fail "self-test expected one structured Datadog evidence artifact, got ${count}"
  fi

  local failed_status
  failed_status="$(field Status <<'JSON'
{"fields":{"Status":"Failed"}}
JSON
)"
  case "$failed_status" in
    Completed|Complete|Succeeded|Promoted|NoPromotion)
      fail "self-test misclassified Failed as a successful terminal episode"
      ;;
    Failed)
      ;;
    *)
      fail "self-test could not read failed terminal status"
      ;;
  esac

  log "self-test passed"
}

if [[ "${SCRIPT_SELF_TEST:-0}" == "1" ]]; then
  run_self_test
  exit 0
fi

record_precondition_blockers
write_blocked_summary_and_exit

if [[ "$BUILD_WORKER" == "1" ]]; then
  log "building paw-codex-worker release binary"
  cargo build -p paw-codex-worker --release
fi

if [[ ! -x "$WORKER_BIN" ]]; then
  fail "worker binary is not executable: ${WORKER_BIN}"
fi

wait_for_metadata

jq -n \
  --arg direction_id "$DIRECTED_EVOLUTION_DIRECTION_ID" \
  --arg organism_id "$DIRECTED_EVOLUTION_ORGANISM_ID" \
  --arg parent_version_id "${DIRECTED_EVOLUTION_PARENT_VERSION_ID:-}" \
  --arg actor "$DIRECTED_EVOLUTION_PROOF_ACTOR" \
  '{
    direction_id: $direction_id,
    organism_id: $organism_id,
    parent_version_id: $parent_version_id,
    adaptation_goal: "Improve Agent Answers while preserving question, answer, acceptance, citation, and reviewer workflows.",
    human_notes: "Fresh live Agent Answers Directed Evolution proof. Requires worker provenance, simulated-user evidence, state verification, and Datadog telemetry evidence before proof closure.",
    created_by_worker_run_id: ("manual:" + $actor),
    evaluator_ref: "genesis://nerdsane/agent-answers-evaluation@production",
    selected_by: $actor,
    selection_notes: "Live proof selection is bounded to the Agent Answers organism and pinned Genesis evaluator.",
    started_by: $actor,
    start_reason: "Operator-confirmed live Agent Answers Directed Evolution proof cycle."
  }' >"$CONTRACT_JSON"

log "starting semantic Agent Answers Directed Evolution episode"
TEMPER_URL="$TEMPER_URL" \
TEMPER_TENANT="$TEMPER_TENANT" \
WORKER_ID="$WORKER_ID" \
WORKER_TOKEN="$WORKER_TOKEN" \
REPO_ROOT="$REPO_ROOT" \
WORKSPACE_ROOT="$WORKSPACE_ROOT" \
CODEX_BIN="$CODEX_BIN" \
PAW_CODEX_ENABLE_EXECUTION="$PAW_CODEX_ENABLE_EXECUTION" \
PAW_CODEX_POLL_ON_START="$PAW_CODEX_POLL_ON_START" \
"$WORKER_BIN" directed-evolution-start-episode "$CONTRACT_JSON" >"$START_JSON"

episode_id="$(jq -r '.episode_id // ""' "$START_JSON")"
if [[ -z "$episode_id" ]]; then
  fail "directed-evolution-start-episode did not return episode_id"
fi

if [[ "$START_LOCAL_WORKER" == "1" ]]; then
  log "starting local worker for live proof"
  TEMPER_URL="$TEMPER_URL" \
  TEMPER_TENANT="$TEMPER_TENANT" \
  WORKER_ID="$WORKER_ID" \
  WORKER_TOKEN="$WORKER_TOKEN" \
  REPO_ROOT="$REPO_ROOT" \
  WORKSPACE_ROOT="$WORKSPACE_ROOT" \
  CODEX_BIN="$CODEX_BIN" \
  PAW_CODEX_ENABLE_EXECUTION="$PAW_CODEX_ENABLE_EXECUTION" \
  PAW_CODEX_POLL_ON_START="$PAW_CODEX_POLL_ON_START" \
  MAX_CONCURRENT_RUNS="$MAX_CONCURRENT_RUNS" \
  "$WORKER_BIN" run >"$WORKER_LOG" 2>&1 &
  WORKER_PID="$!"
else
  log "START_LOCAL_WORKER=0; expecting an already-running production worker pool"
fi

work_item_count="$(wait_for_min_count WorkItems WorkItems "$episode_id" 1)"
worker_run_count="$(wait_for_min_count WorkerRuns WorkerRuns "$episode_id" 1)"
evidence_count="$(wait_for_min_count EvidenceArtifacts EvidenceArtifacts "$episode_id" 1)"
datadog_count="$(wait_for_datadog_evidence "$episode_id")"

episode_body="$(wait_for_episode_terminal "$episode_id")"
episode_status="$(field Status <<<"$episode_body")"
jq . <<<"$episode_body" >"${PROOF_DIR}/episode-terminal.json"

case "$episode_status" in
  Completed|Complete|Succeeded|Promoted|NoPromotion)
    ;;
  *)
    jq -n \
      --arg status "failed" \
      --arg proof_dir "$PROOF_DIR" \
      --arg episode_id "$episode_id" \
      --arg episode_status "$episode_status" \
      --argjson work_item_count "$work_item_count" \
      --argjson worker_run_count "$worker_run_count" \
      --argjson evidence_count "$evidence_count" \
      --argjson datadog_evidence_count "$datadog_count" \
      '{
        status: $status,
        proof_kind: "live Agent Answers Directed Evolution proof",
        proof_dir: $proof_dir,
        agent_answers: {
          episode_id: $episode_id,
          episode_status: $episode_status
        },
        evidence: {
          work_items: $work_item_count,
          worker_runs: $worker_run_count,
          evidence_artifacts: $evidence_count,
          datadog_measured_evidence_artifacts: $datadog_evidence_count
        },
        failure_reason: "terminal episode status was not successful"
      }' >"$SUMMARY_JSON"
    fail "episode ${episode_id} reached ${episode_status}; live proof requires a successful terminal status"
    ;;
esac

summary_json="$(jq -n \
  --arg status "passed" \
  --arg proof_dir "$PROOF_DIR" \
  --arg temper_url "$TEMPER_URL" \
  --arg tenant "$TEMPER_TENANT" \
  --arg episode_id "$episode_id" \
  --arg episode_status "$episode_status" \
  --arg direction_id "$DIRECTED_EVOLUTION_DIRECTION_ID" \
  --arg organism_id "$DIRECTED_EVOLUTION_ORGANISM_ID" \
  --arg worker_id "$WORKER_ID" \
  --argjson work_item_count "$work_item_count" \
  --argjson worker_run_count "$worker_run_count" \
  --argjson evidence_count "$evidence_count" \
  --argjson datadog_evidence_count "$datadog_count" \
  '{
    status: $status,
    proof_kind: "live Agent Answers Directed Evolution proof",
    proof_dir: $proof_dir,
    control_plane: {
      temper_url: $temper_url,
      tenant: $tenant
    },
    agent_answers: {
      organism_id: $organism_id,
      direction_id: $direction_id,
      episode_id: $episode_id,
      episode_status: $episode_status
    },
    worker: {
      worker_id: $worker_id,
      start_local_worker: env.START_LOCAL_WORKER,
      execution_enabled: env.PAW_CODEX_ENABLE_EXECUTION
    },
    evidence: {
      work_items: $work_item_count,
      worker_runs: $worker_run_count,
      evidence_artifacts: $evidence_count,
      datadog_measured_evidence_artifacts: $datadog_evidence_count,
      mandatory_datadog_evidence: env.REQUIRE_DATADOG_EVIDENCE
    }
  }')"

printf '%s\n' "$summary_json" >"$SUMMARY_JSON"

cat >"$PROOF_MD" <<EOF
# Agent Answers Directed Evolution Live Proof

This proof was created by \`directed-evolution-agent-answers-live-proof.sh\`.
It starts a fresh semantic Directed Evolution Episode for the Agent Answers
organism, waits for paw-orchestration \`WorkItems\`, local/production
\`WorkerRuns\`, linked \`EvidenceArtifacts\`, and mandatory Datadog evidence
before declaring the live proof cycle passed.

## Episode

- Episode: ${TEMPER_URL}/tdata/Episodes('${episode_id}')
- Direction: ${TEMPER_URL}/tdata/Directions('${DIRECTED_EVOLUTION_DIRECTION_ID}')
- Organism: ${TEMPER_URL}/tdata/Organisms('${DIRECTED_EVOLUTION_ORGANISM_ID}')
- Terminal status: ${episode_status}

## Evidence Gates

- WorkItems for episode: ${work_item_count}
- WorkerRuns for episode: ${worker_run_count}
- EvidenceArtifacts for episode: ${evidence_count}
- datadog-measured EvidenceArtifacts with \`datadog_url\`: ${datadog_count}

## Files

- Contract: ${CONTRACT_JSON}
- Episode start response: ${START_JSON}
- Terminal episode state: ${PROOF_DIR}/episode-terminal.json
- Local worker log: ${WORKER_LOG}

## Machine Summary

\`\`\`json
${summary_json}
\`\`\`
EOF

printf '%s\n' "$summary_json"
log "live Agent Answers Directed Evolution proof bundle: ${PROOF_DIR}"
