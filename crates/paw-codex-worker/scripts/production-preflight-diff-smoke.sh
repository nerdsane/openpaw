#!/usr/bin/env bash
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel)"
PROOF_DIR="${PROOF_DIR:-/tmp/paw-patrol-preflight-diff-smoke-$$}"
BASELINE_SUMMARY="${PROOF_DIR}/baseline-summary.json"
CURRENT_SUMMARY="${PROOF_DIR}/current-summary.json"

log() {
  printf '[paw-codex-preflight-diff-smoke] %s\n' "$*"
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

mkdir -p "$PROOF_DIR"

cat >"$BASELINE_SUMMARY" <<'JSON'
{
  "status": "blocked",
  "proof_dir": "/tmp/baseline-preflight",
  "worker_id": "mac-mini-codex-prod",
  "gates": [
    { "gate": "env:temper_url", "status": "blocked", "detail": "TEMPER_URL is missing", "evidence": "human input required" },
    { "gate": "env:worker_token", "status": "blocked", "detail": "WORKER_TOKEN is missing", "evidence": "human input required" },
    { "gate": "railway:linked_project", "status": "blocked", "detail": "Railway checkout is not linked", "evidence": "railway-status.txt" },
    { "gate": "command:jq", "status": "pass", "detail": "jq is available", "evidence": "/usr/bin/jq" }
  ],
  "human_blockers": [
    { "gate": "env:temper_url", "detail": "TEMPER_URL is missing", "evidence": "human input required" },
    { "gate": "env:worker_token", "detail": "WORKER_TOKEN is missing", "evidence": "human input required" },
    { "gate": "railway:linked_project", "detail": "Railway checkout is not linked", "evidence": "railway-status.txt" }
  ],
  "railway": {
    "candidates": [
      {
        "project_id": "project-openpaw",
        "project_name": "openpaw-seshendranalla",
        "likely_match": true,
        "environments": [{ "environment_name": "production" }],
        "services": [{ "service_name": "openpaw" }]
      }
    ]
  }
}
JSON

cat >"$CURRENT_SUMMARY" <<'JSON'
{
  "status": "blocked",
  "proof_dir": "/tmp/current-preflight",
  "worker_id": "mac-mini-codex-prod",
  "gates": [
    { "gate": "env:temper_url", "status": "pass", "detail": "TEMPER_URL is set", "evidence": "value intentionally not expanded in proof" },
    { "gate": "env:worker_token", "status": "blocked", "detail": "WORKER_TOKEN is missing", "evidence": "human input required" },
    { "gate": "railway:linked_project", "status": "pass", "detail": "Railway checkout is linked", "evidence": "railway-status.txt" },
    { "gate": "github:temper_pr_216", "status": "blocked", "detail": "Temper PR #216 still needs a decision", "evidence": "temper-pr-216.json" },
    { "gate": "command:jq", "status": "pass", "detail": "jq is available", "evidence": "/usr/bin/jq" }
  ],
  "human_blockers": [
    { "gate": "env:worker_token", "detail": "WORKER_TOKEN is missing", "evidence": "human input required" },
    { "gate": "github:temper_pr_216", "detail": "Temper PR #216 still needs a decision", "evidence": "temper-pr-216.json" }
  ],
  "railway": {
    "candidates": [
      {
        "project_id": "project-openpaw",
        "project_name": "openpaw-seshendranalla",
        "likely_match": true,
        "environments": [{ "environment_name": "production" }],
        "services": [{ "service_name": "openpaw" }]
      },
      {
        "project_id": "project-temper",
        "project_name": "temper",
        "likely_match": true,
        "environments": [{ "environment_name": "production" }],
        "services": [{ "service_name": "temper-server" }]
      }
    ]
  }
}
JSON

"${ROOT}/crates/paw-codex-worker/scripts/production-preflight-diff.sh" \
  "$BASELINE_SUMMARY" \
  "$CURRENT_SUMMARY" \
  "$PROOF_DIR" >/dev/null

test -s "${PROOF_DIR}/summary.json"
test -s "${PROOF_DIR}/proof.md"
test -s "${PROOF_DIR}/preflight-diff.svg"

jq -e '
  .status == "attention" and
  (.resolved_blockers | map(.gate) | index("env:temper_url")) and
  (.resolved_blockers | map(.gate) | index("railway:linked_project")) and
  (.new_blockers | map(.gate) | index("github:temper_pr_216")) and
  (.unchanged_blockers | map(.gate) | index("env:worker_token")) and
  (.railway_candidate_changes.added | map(.project_id) | index("project-temper")) and
  (.changed_gates | map(.gate) | index("env:temper_url"))
' "${PROOF_DIR}/summary.json" >/dev/null

grep -Fq 'Paw Patrol Preflight Diff Proof' "${PROOF_DIR}/proof.md"
grep -Fq 'Resolved Blockers' "${PROOF_DIR}/proof.md"
grep -Fq 'New Blockers' "${PROOF_DIR}/proof.md"
grep -Fq 'Railway Candidate Drift' "${PROOF_DIR}/proof.md"

log "production preflight diff smoke passed"
