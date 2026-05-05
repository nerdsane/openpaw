#!/usr/bin/env bash
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel)"
PROOF_DIR="${PROOF_DIR:-/tmp/paw-patrol-preflight-railway-discovery-$$}"
FAKE_BIN="${PROOF_DIR}/bin"
FAKE_LAUNCHD_PLIST="${PROOF_DIR}/com.temperpaw.paw-codex-worker.plist"

log() {
  printf '[paw-codex-preflight-railway-smoke] %s\n' "$*"
}

fail() {
  log "$*"
  exit 1
}

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || fail "missing required command: $1"
}

require_cmd git
require_cmd jq

mkdir -p "$FAKE_BIN"
cat >"${FAKE_BIN}/railway" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

case "$*" in
  "whoami")
    printf 'Logged in as patrol@example.com\n'
    ;;
  "status")
    printf 'No linked project found. Run railway link to connect to a project\n' >&2
    exit 1
    ;;
  "project list --json")
    cat <<'JSON'
[
  {
    "id": "project-openpaw",
    "name": "openpaw-seshendranalla",
    "environments": {
      "edges": [
        {
          "node": {
            "id": "env-openpaw-production",
            "name": "production",
            "canAccess": true,
            "serviceInstances": {
              "edges": [
                { "node": { "serviceId": "service-openpaw" } },
                { "node": { "serviceId": "service-postgres" } }
              ]
            }
          }
        }
      ]
    },
    "services": {
      "edges": [
        { "node": { "id": "service-openpaw", "name": "openpaw" } },
        { "node": { "id": "service-postgres", "name": "Postgres" } }
      ]
    }
  },
  {
    "id": "project-temper",
    "name": "temper",
    "environments": {
      "edges": [
        {
          "node": {
            "id": "env-temper-production",
            "name": "production",
            "canAccess": true,
            "serviceInstances": {
              "edges": [
                { "node": { "serviceId": "service-temper-server" } }
              ]
            }
          }
        }
      ]
    },
    "services": {
      "edges": [
        { "node": { "id": "service-temper-server", "name": "temper-server" } }
      ]
    }
  }
]
JSON
    ;;
  *)
    printf 'unexpected fake railway invocation: %s\n' "$*" >&2
    exit 64
    ;;
esac
EOF
chmod +x "${FAKE_BIN}/railway"

touch "$FAKE_LAUNCHD_PLIST"

log "proof dir: ${PROOF_DIR}"

PATH="${FAKE_BIN}:$PATH" \
PROOF_DIR="$PROOF_DIR" \
CHECK_RAILWAY=1 \
CHECK_GITHUB=0 \
TEMPER_URL="https://temperpaw.example.test" \
WORKER_TOKEN="fake-worker-token" \
PATROL_OPERATOR_TOKEN="fake-operator-token" \
CONFIRM_LOCAL_CODEX_WORKER_ID="mac-mini-codex-prod" \
PATROL_DATADOG_WEBHOOK_SECRET="fake-datadog-secret" \
PATROL_DISCORD_WEBHOOK_SECRET="fake-discord-secret" \
PATROL_GITHUB_WEBHOOK_SECRET="fake-github-secret" \
CODEX_BIN="${ROOT}/crates/paw-codex-worker/fixtures/fake-codex.sh" \
LAUNCHD_PLIST="$FAKE_LAUNCHD_PLIST" \
"${ROOT}/crates/paw-codex-worker/scripts/production-preflight.sh" >/dev/null

test -s "${PROOF_DIR}/summary.json"
test -s "${PROOF_DIR}/railway-projects.json"
test -s "${PROOF_DIR}/operator-handoff.md"

jq -e '
  .railway.candidates | length == 2
' "${PROOF_DIR}/summary.json" >/dev/null

jq -e '
  any(.railway.candidates[];
    .project_id == "project-openpaw" and
    .project_name == "openpaw-seshendranalla" and
    any(.services[]; .service_id == "service-openpaw" and .service_name == "openpaw") and
    any(.environments[]; .environment_name == "production" and .can_access == true)
  )
' "${PROOF_DIR}/summary.json" >/dev/null

jq -e '
  any(.gates[];
    .gate == "railway:candidate_projects" and
    .status == "pass"
  )
' "${PROOF_DIR}/summary.json" >/dev/null

jq -e '
  (.git_head | type == "string" and length >= 7) and
  (.git_branch | type == "string") and
  (.git_status_short | type == "string") and
  (.git_clean | type == "boolean")
' "${PROOF_DIR}/summary.json" >/dev/null

jq -e '
  any(.human_blockers[];
    .gate == "railway:linked_project"
  )
' "${PROOF_DIR}/summary.json" >/dev/null

grep -Fq 'Railway Candidate Projects' "${PROOF_DIR}/proof.md"
grep -Fq 'Git head:' "${PROOF_DIR}/proof.md"
grep -Fq 'Git clean:' "${PROOF_DIR}/proof.md"
grep -Fq 'Paw Patrol Production Operator Handoff' "${PROOF_DIR}/operator-handoff.md"
grep -Fq 'Human Blocker Decisions' "${PROOF_DIR}/operator-handoff.md"
grep -Fq 'Railway Project Choice' "${PROOF_DIR}/operator-handoff.md"
grep -Fq 'Git head:' "${PROOF_DIR}/operator-handoff.md"
grep -Fq 'Git clean:' "${PROOF_DIR}/operator-handoff.md"
grep -Fq 'railway link <project_id>' "${PROOF_DIR}/operator-handoff.md"
grep -Fq 'export TEMPER_URL=' "${PROOF_DIR}/operator-handoff.md"
grep -Fq 'CONFIRM_TEMPER_PIN_OK' "${PROOF_DIR}/operator-handoff.md"
grep -Fq 'launchd approval' "${PROOF_DIR}/operator-handoff.md"

if grep -Fq 'fake-worker-token' "${PROOF_DIR}/operator-handoff.md"; then
  fail "operator handoff leaked the fake worker token"
fi

if grep -Fq 'fake-operator-token' "${PROOF_DIR}/operator-handoff.md"; then
  fail "operator handoff leaked the fake operator token"
fi

log "Railway discovery preflight smoke passed"
