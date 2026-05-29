#!/usr/bin/env bash
set -euo pipefail

BASE_URL="${TEMPER_BASE_URL:-${BASE_URL:-}}"
TENANT_ID="${TENANT_ID:-default}"
RUN_SUFFIX="${RUN_SUFFIX:-$(date -u +%Y%m%d%H%M%S)-$$}"
PACKAGED_WASM_PATH="${PACKAGED_WASM_PATH:-}"
ACTION_PRINCIPAL_KIND="${TEMPER_ACTION_PRINCIPAL_KIND:-agent}"
OBSERVE_PRINCIPAL_KIND="${TEMPER_OBSERVE_PRINCIPAL_KIND:-admin}"
PRINCIPAL_AGENT_TYPE="${TEMPER_AGENT_TYPE:-system}"

if [ -z "${BASE_URL}" ]; then
  echo "TEMPER_BASE_URL or BASE_URL is required" >&2
  exit 64
fi
if [ -z "${TEMPER_API_KEY:-}" ]; then
  echo "TEMPER_API_KEY is required" >&2
  exit 65
fi

BASE_URL="${BASE_URL%/}"
WORKSPACE_ID="ws-artifact-batch-e2e-${RUN_SUFFIX}"
BATCH_ID="ab-artifact-batch-e2e-${RUN_SUFFIX}"
PRINCIPAL_ID="production-artifact-batch-e2e"

action_headers=(
  -H "Authorization: Bearer ${TEMPER_API_KEY}"
  -H "X-Tenant-Id: ${TENANT_ID}"
  -H "x-temper-principal-kind: ${ACTION_PRINCIPAL_KIND}"
  -H "x-temper-principal-id: system"
  -H "x-temper-agent-type: ${PRINCIPAL_AGENT_TYPE}"
)

observe_headers=(
  -H "Authorization: Bearer ${TEMPER_API_KEY}"
  -H "X-Tenant-Id: ${TENANT_ID}"
  -H "x-temper-principal-kind: ${OBSERVE_PRINCIPAL_KIND}"
  -H "x-temper-principal-id: ${PRINCIPAL_ID}"
  -H "x-temper-agent-type: ${PRINCIPAL_AGENT_TYPE}"
)

api_get() {
  local path="$1"
  if [[ "${path}" == /observe/* ]]; then
    curl -fsS "${observe_headers[@]}" "${BASE_URL}${path}"
  else
    curl -fsS "${action_headers[@]}" "${BASE_URL}${path}"
  fi
}

api_post() {
  local path="$1"
  local body="$2"
  curl -fsS "${action_headers[@]}" \
    -H "Content-Type: application/json" \
    -X POST \
    --data "${body}" \
    "${BASE_URL}${path}"
}

json_field() {
  local expr="$1"
  jq -r "${expr} // empty"
}

entity_id_expr='.Id // .id // .entity_id // .fields.Id // .fields.id'
status_expr='.Status // .status // .current_state // .fields.Status // .fields.status'

module_json="$(api_get "/observe/wasm/modules")"
artifact_module="$(jq -c '[.. | objects | select((.module_name? // .name?) == "artifact_batch_apply")][0] // empty' <<<"${module_json}")"
if [ -z "${artifact_module}" ]; then
  echo "artifact_batch_apply module not found in /observe/wasm/modules" >&2
  exit 1
fi
observed_hash="$(jq -r '.sha256_hash // .hash // .content_hash // empty' <<<"${artifact_module}")"
if [ -z "${observed_hash}" ]; then
  echo "artifact_batch_apply module did not expose a hash" >&2
  exit 1
fi

packaged_hash=""
if [ -n "${PACKAGED_WASM_PATH}" ]; then
  if [ ! -f "${PACKAGED_WASM_PATH}" ]; then
    echo "PACKAGED_WASM_PATH does not exist: ${PACKAGED_WASM_PATH}" >&2
    exit 1
  fi
  if command -v sha256sum >/dev/null 2>&1; then
    packaged_hash="$(sha256sum "${PACKAGED_WASM_PATH}" | awk '{print $1}')"
  else
    packaged_hash="$(shasum -a 256 "${PACKAGED_WASM_PATH}" | awk '{print $1}')"
  fi
  if [ "${observed_hash}" != "${packaged_hash}" ]; then
    echo "artifact_batch_apply hash mismatch: observed=${observed_hash} packaged=${packaged_hash}" >&2
    exit 1
  fi
fi

path_one="/katagami/deploy-e2e/${RUN_SUFFIX}/language.md"
path_two="/katagami/deploy-e2e/${RUN_SUFFIX}/tokens.json"
path_three="/katagami/deploy-e2e/${RUN_SUFFIX}/review.txt"
content_one=$'# Deploy E2E\n\nArtifactBatch bounded-actor proof.'
content_two='{"palette":["ink","signal"],"bounded":true}'
content_three="review: deployed artifact batch passed"

manifest="$(jq -cn \
  --arg p1 "${path_one}" --arg c1 "${content_one}" \
  --arg p2 "${path_two}" --arg c2 "${content_two}" \
  --arg p3 "${path_three}" --arg c3 "${content_three}" \
  '[
    {path:$p1, content:$c1, mime_type:"text/markdown"},
    {path:$p2, content:$c2, mime_type:"application/json"},
    {path:$p3, content:$c3, mime_type:"text/plain"}
  ]')"

api_post "/tdata/Workspaces" "$(jq -cn \
  --arg id "${WORKSPACE_ID}" \
  --arg name "ArtifactBatch deploy E2E ${RUN_SUFFIX}" \
  '{Id:$id, Name:$name, QuotaLimit:1000000}')" >/dev/null

api_post "/tdata/ArtifactBatches" "$(jq -cn \
  --arg id "${BATCH_ID}" \
  --arg ws "${WORKSPACE_ID}" \
  --arg manifest "${manifest}" \
  '{Id:$id, WorkspaceId:$ws, FilesManifest:$manifest, FileCount:3}')" >/dev/null

api_post "/tdata/ArtifactBatches('${BATCH_ID}')/Temper.Submit" "$(jq -cn \
  --arg ws "${WORKSPACE_ID}" \
  --arg manifest "${manifest}" \
  --arg by "${PRINCIPAL_ID}" \
  '{workspace_id:$ws, files_manifest:$manifest, submitted_by:$by, file_count:3}')" >/dev/null

api_post "/tdata/ArtifactBatches('${BATCH_ID}')/Temper.Apply?await_integration=true" '{}' >/dev/null

batch_json=""
batch_status=""
for _ in $(seq 1 30); do
  batch_json="$(api_get "/tdata/ArtifactBatches('${BATCH_ID}')")"
  batch_status="$(json_field "${status_expr}" <<<"${batch_json}")"
  if [ "${batch_status}" = "Completed" ]; then
    break
  fi
  if [ "${batch_status}" = "Failed" ]; then
    jq . <<<"${batch_json}" >&2
    exit 1
  fi
  sleep 2
done

if [ "${batch_status}" != "Completed" ]; then
  echo "ArtifactBatch did not complete; status=${batch_status:-unknown}" >&2
  jq . <<<"${batch_json}" >&2
  exit 1
fi

files_json="$(api_get "/tdata/Files?\$filter=WorkspaceId%20eq%20'${WORKSPACE_ID}'&\$top=50")"

file_id_for_path() {
  local path="$1"
  jq -r --arg path "${path}" '
    (.value // .items // .entities // [])[]
    | select((.Path // .path // .fields.Path // .fields.path) == $path)
    | (.Id // .id // .entity_id // .fields.Id // .fields.id)
  ' <<<"${files_json}" | head -n 1
}

verify_file() {
  local path="$1"
  local expected="$2"
  local file_id
  file_id="$(file_id_for_path "${path}")"
  if [ -z "${file_id}" ]; then
    echo "No File entity found for ${path}" >&2
    jq . <<<"${files_json}" >&2
    exit 1
  fi
  local actual
  actual="$(api_get "/tdata/Files('${file_id}')/\$value")"
  if [ "${actual}" != "${expected}" ]; then
    echo "Readback mismatch for ${path}" >&2
    exit 1
  fi
  jq -cn --arg path "${path}" --arg file_id "${file_id}" --argjson bytes "${#actual}" \
    '{path:$path, file_id:$file_id, bytes:$bytes}'
}

file_one="$(verify_file "${path_one}" "${content_one}")"
file_two="$(verify_file "${path_two}" "${content_two}")"
file_three="$(verify_file "${path_three}" "${content_three}")"

usage_json="$(api_get "/tdata/WorkspaceUsageBuckets?\$filter=ArtifactBatchId%20eq%20'${BATCH_ID}'&\$top=10")"
usage_bucket="$(jq -c '(.value // .items // .entities // [])[0] // empty' <<<"${usage_json}")"
if [ -z "${usage_bucket}" ]; then
  echo "No WorkspaceUsageBucket found for ${BATCH_ID}" >&2
  jq . <<<"${usage_json}" >&2
  exit 1
fi
bytes_delta="$(jq -r '.BytesDelta // .bytes_delta // .fields.BytesDelta // .fields.bytes_delta // empty' <<<"${usage_bucket}")"
file_delta="$(jq -r '.FileDelta // .file_delta // .fields.FileDelta // .fields.file_delta // empty' <<<"${usage_bucket}")"
usage_bucket_id="$(jq -r "${entity_id_expr} // empty" <<<"${usage_bucket}")"
expected_bytes=$((${#content_one} + ${#content_two} + ${#content_three}))

if [ "${bytes_delta}" != "${expected_bytes}" ] || [ "${file_delta}" != "3" ]; then
  echo "Usage bucket mismatch: bytes_delta=${bytes_delta} file_delta=${file_delta} expected_bytes=${expected_bytes}" >&2
  jq . <<<"${usage_bucket}" >&2
  exit 1
fi

workspace_history="$(api_get "/observe/entities/Workspace/${WORKSPACE_ID}/history")"
hot_actions_regex='^(MkDir|CreateFile|ResolvePath|ListDir|IncrementUsage|IncrementFileCount)$'
if jq -e --arg re "${hot_actions_regex}" '.events[]? | select((.action // "") | test($re))' <<<"${workspace_history}" >/dev/null; then
  echo "Workspace history contains hot-path file IO actions" >&2
  jq . <<<"${workspace_history}" >&2
  exit 1
fi
workspace_event_count="$(jq -r '.events | length' <<<"${workspace_history}")"
workspace_actions="$(jq -c '[.events[]?.action]' <<<"${workspace_history}")"

jq -cn \
  --arg workspaceId "${WORKSPACE_ID}" \
  --arg batchId "${BATCH_ID}" \
  --arg batchStatus "${batch_status}" \
  --arg artifactModuleHash "${observed_hash}" \
  --arg packagedModuleHash "${packaged_hash}" \
  --arg usageBucketId "${usage_bucket_id}" \
  --argjson expectedBytes "${expected_bytes}" \
  --argjson bytesDelta "${bytes_delta}" \
  --argjson fileDelta "${file_delta}" \
  --argjson workspaceEventCount "${workspace_event_count}" \
  --argjson workspaceActions "${workspace_actions}" \
  --argjson files "[${file_one},${file_two},${file_three}]" \
  '{
    workspaceId:$workspaceId,
    batchId:$batchId,
    batchStatus:$batchStatus,
    artifactModuleHash:$artifactModuleHash,
    packagedModuleHash:($packagedModuleHash | if . == "" then null else . end),
    files:$files,
    usageBucket:{id:$usageBucketId, expectedBytes:$expectedBytes, bytesDelta:$bytesDelta, fileDelta:$fileDelta},
    workspaceHistory:{eventCount:$workspaceEventCount, actions:$workspaceActions}
  }'
