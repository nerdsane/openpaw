#!/usr/bin/env bash
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel)"
PROOF_DIR="${PROOF_DIR:-/tmp/paw-patrol-preflight-github-smoke-$$}"
FAKE_BIN="${PROOF_DIR}/bin"
FAKE_LAUNCHD_PLIST="${PROOF_DIR}/com.temperpaw.paw-codex-worker.plist"
WITHOUT_CONFIRM="${PROOF_DIR}/without-confirm"
WITH_CONFIRM="${PROOF_DIR}/with-confirm"

log() {
  printf '[paw-codex-preflight-github-smoke] %s\n' "$*"
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

mkdir -p "$FAKE_BIN" "$WITHOUT_CONFIRM" "$WITH_CONFIRM"
touch "$FAKE_LAUNCHD_PLIST"

cat >"${FAKE_BIN}/launchctl" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

case "$1" in
  print)
    printf 'fake launchd worker loaded: %s\n' "${2:-}"
    ;;
  *)
    printf 'unexpected fake launchctl invocation: %s\n' "$*" >&2
    exit 64
    ;;
esac
EOF
chmod +x "${FAKE_BIN}/launchctl"

cat >"${FAKE_BIN}/gh" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

case "$*" in
  "pr view 216 --repo nerdsane/temper --json url,isDraft,state,mergeStateStatus,headRefOid")
    cat <<'JSON'
{
  "url": "https://github.com/nerdsane/temper/pull/216",
  "isDraft": false,
  "state": "MERGED",
  "mergeStateStatus": "UNKNOWN",
  "headRefOid": "557db7f30814801ad42d28e92725d007c6ce7732"
}
JSON
    ;;
  "pr view 218 --repo nerdsane/temperpaw --json url,isDraft,state,mergeStateStatus,headRefOid,statusCheckRollup")
    cat <<'JSON'
{
  "url": "https://github.com/nerdsane/temperpaw/pull/218",
  "isDraft": false,
  "state": "OPEN",
  "mergeStateStatus": "CLEAN",
  "headRefOid": "patrol-head",
  "statusCheckRollup": [
    {
      "__typename": "CheckRun",
      "name": "checks",
      "status": "COMPLETED",
      "conclusion": "SUCCESS"
    }
  ]
}
JSON
    ;;
  "pr view 220 --repo nerdsane/temperpaw --json url,isDraft,state,mergeStateStatus,headRefOid")
    cat <<'JSON'
{
  "url": "https://github.com/nerdsane/temperpaw/pull/220",
  "isDraft": false,
  "state": "MERGED",
  "mergeStateStatus": "UNKNOWN",
  "headRefOid": "patrol-wasm-image-head"
}
JSON
    ;;
  "pr view 221 --repo nerdsane/temperpaw --json url,isDraft,state,mergeStateStatus,headRefOid")
    cat <<'JSON'
{
  "url": "https://github.com/nerdsane/temperpaw/pull/221",
  "isDraft": false,
  "state": "MERGED",
  "mergeStateStatus": "UNKNOWN",
  "headRefOid": "patrol-mac-mini-bootstrap-head"
}
JSON
    ;;
  *)
    printf 'unexpected fake gh invocation: %s\n' "$*" >&2
    exit 64
    ;;
esac
EOF
chmod +x "${FAKE_BIN}/gh"

run_preflight() {
  local proof_dir="$1"
  shift

  PATH="${FAKE_BIN}:$PATH" \
  PROOF_DIR="$proof_dir" \
  CHECK_RAILWAY=0 \
  CHECK_GITHUB=1 \
  TEMPER_URL="https://temperpaw.example.test" \
  WORKER_TOKEN="fake-worker-token" \
  PATROL_OPERATOR_TOKEN="fake-operator-token" \
  CONFIRM_LOCAL_CODEX_WORKER_ID="mac-mini-codex-prod" \
  PATROL_DATADOG_WEBHOOK_SECRET="fake-datadog-secret" \
  PATROL_DISCORD_WEBHOOK_SECRET="fake-discord-secret" \
  PATROL_GITHUB_WEBHOOK_SECRET="fake-github-secret" \
  CODEX_BIN="${ROOT}/crates/paw-codex-worker/fixtures/fake-codex.sh" \
  LAUNCHD_PLIST="$FAKE_LAUNCHD_PLIST" \
  "$@" \
  "${ROOT}/crates/paw-codex-worker/scripts/production-preflight.sh" >/dev/null
}

log "proof dir: ${PROOF_DIR}"

run_preflight "$WITHOUT_CONFIRM" env
cp "${WITHOUT_CONFIRM}/summary.json" "${PROOF_DIR}/summary-without-confirm.json"

jq -e '
  .status == "blocked" and
  any(.human_blockers[];
    .gate == "github:temperpaw_pr_218" and
    (.detail | contains("clean and green but unmerged"))
  )
' "${PROOF_DIR}/summary-without-confirm.json" >/dev/null

run_preflight "$WITH_CONFIRM" env CONFIRM_TEMPERPAW_PR_OK=1
cp "${WITH_CONFIRM}/summary.json" "${PROOF_DIR}/summary-with-confirm.json"

jq -e '
  any(.gates[];
    .gate == "github:temperpaw_pr_218" and
    .status == "pass" and
    (.detail | contains("operator confirmed"))
  ) and
  any(.gates[];
    .gate == "github:temperpaw_pr_220" and
    .status == "pass" and
    (.detail | contains("merged"))
  ) and
  any(.gates[];
    .gate == "github:temperpaw_pr_221" and
    .status == "pass" and
    (.detail | contains("merged"))
  ) and
  ([.human_blockers[] | select(.gate == "github:temperpaw_pr_218")] | length == 0)
' "${PROOF_DIR}/summary-with-confirm.json" >/dev/null

cat >"${PROOF_DIR}/proof.md" <<EOF
# Paw Patrol GitHub Preflight Smoke

This smoke uses fake \`gh\` and \`launchctl\` binaries to prove production
preflight treats TemperPaw PR #218, #220, and #221 as cutover gates.

## Evidence

- Without \`CONFIRM_TEMPERPAW_PR_OK=1\`:
  \`${PROOF_DIR}/summary-without-confirm.json\`
- With \`CONFIRM_TEMPERPAW_PR_OK=1\`:
  \`${PROOF_DIR}/summary-with-confirm.json\`
- Full proof without confirmation:
  \`${WITHOUT_CONFIRM}\`
- Full proof with confirmation:
  \`${WITH_CONFIRM}\`

## Flow

\`\`\`mermaid
flowchart TD
    A["Fake GitHub: PR #218 clean and CI passed, but OPEN"] --> B["production-preflight.sh"]
    B --> C{"CONFIRM_TEMPERPAW_PR_OK=1?"}
    C -->|"no"| D["github:temperpaw_pr_218 blocked"]
    C -->|"yes"| E["github:temperpaw_pr_218 passed with operator decision"]
\`\`\`
EOF

log "production preflight GitHub smoke passed"
