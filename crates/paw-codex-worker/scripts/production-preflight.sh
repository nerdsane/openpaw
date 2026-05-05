#!/usr/bin/env bash
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel)"
STAMP="$(date -u +%Y%m%dT%H%M%SZ)"
PROOF_DIR="${PROOF_DIR:-/tmp/paw-patrol-production-preflight-${STAMP}-$$}"
GATES_TSV="${PROOF_DIR}/gates.tsv"
SUMMARY_JSON="${PROOF_DIR}/summary.json"
PROOF_MD="${PROOF_DIR}/proof.md"
PREFLIGHT_SVG="${PROOF_DIR}/preflight.svg"
WORKER_ID="${WORKER_ID:-mac-mini-codex-prod}"
REPO_ROOT="${REPO_ROOT:-$ROOT}"
WORKSPACE_ROOT="${WORKSPACE_ROOT:-$(dirname "$ROOT")}"
CODEX_BIN="${CODEX_BIN:-codex}"
LAUNCHD_LABEL="${LAUNCHD_LABEL:-com.temperpaw.paw-codex-worker}"
LAUNCHD_PLIST="${LAUNCHD_PLIST:-$HOME/Library/LaunchAgents/${LAUNCHD_LABEL}.plist}"
CHECK_RAILWAY="${CHECK_RAILWAY:-1}"
CHECK_GITHUB="${CHECK_GITHUB:-1}"
STRICT="${STRICT:-0}"

log() {
  printf '[paw-codex-preflight] %s\n' "$*"
}

sanitize_field() {
  printf '%s' "$*" | tr '\t\r\n' '   '
}

redact_output() {
  sed -E 's/[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}/<redacted-email>/g'
}

add_gate() {
  local gate="$1"
  local status="$2"
  local detail="$3"
  local evidence="$4"
  printf '%s\t%s\t%s\t%s\n' \
    "$(sanitize_field "$gate")" \
    "$(sanitize_field "$status")" \
    "$(sanitize_field "$detail")" \
    "$(sanitize_field "$evidence")" >>"$GATES_TSV"
}

capture_command() {
  local out="$1"
  shift
  local raw="${out}.raw"
  set +e
  "$@" >"$raw" 2>&1
  local status="$?"
  set -e
  redact_output <"$raw" >"$out"
  rm -f "$raw"
  return "$status"
}

command_gate() {
  local cmd="$1"
  if command -v "$cmd" >/dev/null 2>&1; then
    add_gate "command:${cmd}" "pass" "${cmd} is available" "$(command -v "$cmd")"
  else
    add_gate "command:${cmd}" "blocked" "missing required command: ${cmd}" "install ${cmd}"
  fi
}

optional_command_gate() {
  local cmd="$1"
  if command -v "$cmd" >/dev/null 2>&1; then
    add_gate "command:${cmd}" "pass" "${cmd} is available" "$(command -v "$cmd")"
  else
    add_gate "command:${cmd}" "warn" "optional command ${cmd} is not available" "install ${cmd} if this check matters"
  fi
}

mkdir -p "$PROOF_DIR"
: >"$GATES_TSV"

log "repo root: ${ROOT}"
log "proof dir: ${PROOF_DIR}"
log "this preflight does not mutate Railway, launchd, or Temper"

add_gate "non_mutating_guard" "pass" "production-preflight.sh does not mutate Railway, launchd, or Temper" "read-only checks only"

command_gate git
command_gate cargo
command_gate jq
command_gate curl
optional_command_gate gh
optional_command_gate railway
optional_command_gate launchctl

if git -C "$ROOT" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  add_gate "git:worktree" "pass" "current directory is a git worktree" "$ROOT"
else
  add_gate "git:worktree" "blocked" "current directory is not inside a git worktree" "$ROOT"
fi

git -C "$ROOT" branch --show-current >"${PROOF_DIR}/git-branch.txt" 2>/dev/null || true
git -C "$ROOT" rev-parse HEAD >"${PROOF_DIR}/git-head.txt" 2>/dev/null || true
git -C "$ROOT" status --short >"${PROOF_DIR}/git-status.txt" 2>/dev/null || true
if [[ -s "${PROOF_DIR}/git-status.txt" ]]; then
  add_gate "git:clean" "warn" "worktree has local changes; production activation should use a reviewed checkout" "${PROOF_DIR}/git-status.txt"
else
  add_gate "git:clean" "pass" "worktree is clean" "${PROOF_DIR}/git-status.txt"
fi

if [[ -d "$REPO_ROOT" ]]; then
  add_gate "paths:repo_root" "pass" "REPO_ROOT exists" "$REPO_ROOT"
else
  add_gate "paths:repo_root" "blocked" "REPO_ROOT does not exist" "$REPO_ROOT"
fi

if [[ -d "$WORKSPACE_ROOT" ]]; then
  add_gate "paths:workspace_root" "pass" "WORKSPACE_ROOT exists" "$WORKSPACE_ROOT"
else
  add_gate "paths:workspace_root" "blocked" "WORKSPACE_ROOT does not exist" "$WORKSPACE_ROOT"
fi

if [[ -n "${TEMPER_URL:-}" ]]; then
  add_gate "env:temper_url" "pass" "TEMPER_URL is set" "value intentionally not expanded in proof"
else
  add_gate "env:temper_url" "blocked" "TEMPER_URL is missing; set it to the Railway TemperPaw control-plane URL" "human input required"
fi

if [[ -n "${WORKER_TOKEN:-}" ]]; then
  add_gate "env:worker_token" "pass" "WORKER_TOKEN is set" "value intentionally not printed"
elif [[ -n "${TEMPER_WORKER_TOKEN:-}" ]]; then
  add_gate "env:worker_token" "warn" "TEMPER_WORKER_TOKEN is set but production-readiness.sh expects WORKER_TOKEN" "export WORKER_TOKEN=\"$TEMPER_WORKER_TOKEN\""
else
  add_gate "env:worker_token" "blocked" "WORKER_TOKEN is missing" "mint or provide a Temper worker credential"
fi

if [[ "${CONFIRM_LOCAL_CODEX_WORKER_ID:-}" == "$WORKER_ID" || "${LOCAL_CODEX_WORKER_ID_CONFIRMED:-0}" == "1" ]]; then
  add_gate "temper:local_codex_worker_id" "pass" "production local_codex_worker_id is confirmed for ${WORKER_ID}" "operator confirmation"
else
  add_gate "temper:local_codex_worker_id" "blocked" "confirm production local_codex_worker_id equals WORKER_ID=${WORKER_ID}" "human input required"
fi

for secret_name in PATROL_DATADOG_WEBHOOK_SECRET PATROL_DISCORD_WEBHOOK_SECRET PATROL_GITHUB_WEBHOOK_SECRET; do
  if [[ -n "${!secret_name:-}" ]]; then
    add_gate "webhook_secret:${secret_name}" "pass" "${secret_name} is set" "value intentionally not printed"
  else
    add_gate "webhook_secret:${secret_name}" "blocked" "${secret_name} is missing for signed production webhook intake" "human input required"
  fi
done

if command -v "$CODEX_BIN" >/dev/null 2>&1 || [[ -x "$CODEX_BIN" ]]; then
  if capture_command "${PROOF_DIR}/codex-version.txt" "$CODEX_BIN" --version; then
    add_gate "codex:binary" "pass" "CODEX_BIN can run --version" "${PROOF_DIR}/codex-version.txt"
  else
    add_gate "codex:binary" "blocked" "CODEX_BIN exists but --version failed" "${PROOF_DIR}/codex-version.txt"
  fi
else
  add_gate "codex:binary" "blocked" "CODEX_BIN is not executable or on PATH: ${CODEX_BIN}" "install or point CODEX_BIN at the Codex CLI"
fi

if [[ -x "${ROOT}/target/release/paw-codex-worker" ]]; then
  add_gate "worker:release_binary" "pass" "release paw-codex-worker binary already exists" "${ROOT}/target/release/paw-codex-worker"
else
  add_gate "worker:release_binary" "warn" "release paw-codex-worker binary is not built yet" "production-readiness.sh will run cargo build -p paw-codex-worker --release"
fi

if [[ -f "$LAUNCHD_PLIST" ]]; then
  add_gate "launchd:plist" "pass" "launchd plist exists" "$LAUNCHD_PLIST"
else
  add_gate "launchd:plist" "blocked" "launchd plist is not rendered yet" "run production-readiness.sh with WRITE_LAUNCHD_PLIST=1 after doctor passes"
fi

if command -v launchctl >/dev/null 2>&1; then
  if capture_command "${PROOF_DIR}/launchctl-print.txt" launchctl print "gui/$(id -u)/${LAUNCHD_LABEL}"; then
    add_gate "launchd:loaded" "pass" "launchd worker is loaded" "${PROOF_DIR}/launchctl-print.txt"
  else
    add_gate "launchd:loaded" "blocked" "launchd worker is not loaded" "${PROOF_DIR}/launchctl-print.txt"
  fi
else
  add_gate "launchd:loaded" "warn" "launchctl is unavailable on this machine" "skip on non-macOS development hosts"
fi

if [[ "$CHECK_RAILWAY" == "1" ]]; then
  if command -v railway >/dev/null 2>&1; then
    if capture_command "${PROOF_DIR}/railway-whoami.txt" railway whoami; then
      add_gate "railway:login" "pass" "railway CLI is logged in" "${PROOF_DIR}/railway-whoami.txt"
    else
      add_gate "railway:login" "blocked" "railway CLI login check failed" "${PROOF_DIR}/railway-whoami.txt"
    fi

    if capture_command "${PROOF_DIR}/railway-status.txt" railway status; then
      add_gate "railway:linked_project" "pass" "railway status succeeded for the current checkout" "${PROOF_DIR}/railway-status.txt"
    else
      add_gate "railway:linked_project" "blocked" "railway status failed; the checkout is probably not linked to a Railway project/service" "${PROOF_DIR}/railway-status.txt"
    fi
  else
    add_gate "railway:cli" "blocked" "railway CLI is unavailable and CHECK_RAILWAY=1" "install Railway CLI or set CHECK_RAILWAY=0 for local-only proof"
  fi
else
  add_gate "railway:skipped" "warn" "CHECK_RAILWAY=0; Railway login/link checks were skipped" "set CHECK_RAILWAY=1 for production cutover"
fi

if [[ "$CHECK_GITHUB" == "1" ]]; then
  if command -v gh >/dev/null 2>&1; then
    if capture_command "${PROOF_DIR}/temper-pr-216.json" gh pr view 216 --repo nerdsane/temper --json url,isDraft,state,mergeStateStatus,headRefOid; then
      if jq -e '.state == "MERGED"' "${PROOF_DIR}/temper-pr-216.json" >/dev/null 2>&1; then
        add_gate "github:temper_pr_216" "pass" "Temper Cedar dependency PR #216 is merged" "${PROOF_DIR}/temper-pr-216.json"
      elif [[ "${CONFIRM_TEMPER_PIN_OK:-0}" == "1" ]]; then
        add_gate "github:temper_pr_216" "pass" "operator confirmed the current TemperPaw git-revision pin is approved while Temper PR #216 remains unmerged" "${PROOF_DIR}/temper-pr-216.json"
      else
        add_gate "github:temper_pr_216" "blocked" "Temper Cedar dependency PR #216 is still draft or unmerged; set CONFIRM_TEMPER_PIN_OK=1 only if production may use the pinned Temper revision" "${PROOF_DIR}/temper-pr-216.json"
      fi
    else
      add_gate "github:temper_pr_216" "warn" "could not inspect Temper PR #216" "${PROOF_DIR}/temper-pr-216.json"
    fi

    if capture_command "${PROOF_DIR}/temperpaw-pr-218.json" gh pr view 218 --repo nerdsane/temperpaw --json url,isDraft,mergeStateStatus,headRefOid; then
      add_gate "github:temperpaw_pr_218" "pass" "TemperPaw PR #218 is inspectable" "${PROOF_DIR}/temperpaw-pr-218.json"
    else
      add_gate "github:temperpaw_pr_218" "warn" "could not inspect TemperPaw PR #218" "${PROOF_DIR}/temperpaw-pr-218.json"
    fi
  else
    add_gate "github:cli" "warn" "gh CLI is unavailable and CHECK_GITHUB=1" "install gh or set CHECK_GITHUB=0"
  fi
else
  add_gate "github:skipped" "warn" "CHECK_GITHUB=0; GitHub PR checks were skipped" "set CHECK_GITHUB=1 for dependency handoff evidence"
fi

gates_json="$(jq -R -s '
  split("\n")
  | map(select(length > 0))
  | map(split("\t") | {
      gate: .[0],
      status: .[1],
      detail: .[2],
      evidence: .[3]
    })
' <"$GATES_TSV")"

overall_status="$(jq -r '
  if any(.[]; .status == "blocked") then "blocked"
  elif any(.[]; .status == "warn") then "warn"
  else "passed"
  end
' <<<"$gates_json")"

summary_json="$(jq -n \
  --arg status "$overall_status" \
  --arg proof_dir "$PROOF_DIR" \
  --arg worker_id "$WORKER_ID" \
  --arg repo_root "$REPO_ROOT" \
  --arg workspace_root "$WORKSPACE_ROOT" \
  --arg launchd_label "$LAUNCHD_LABEL" \
  --arg strict "$STRICT" \
  --arg check_railway "$CHECK_RAILWAY" \
  --arg check_github "$CHECK_GITHUB" \
  --argjson gates "$gates_json" \
  '{
    status: $status,
    proof_dir: $proof_dir,
    worker_id: $worker_id,
    repo_root: $repo_root,
    workspace_root: $workspace_root,
    launchd_label: $launchd_label,
    strict: ($strict == "1"),
    checks: {
      railway: ($check_railway == "1"),
      github: ($check_github == "1")
    },
    gates: $gates,
    human_blockers: ($gates | map(select(.status == "blocked") | {
      gate: .gate,
      detail: .detail,
      evidence: .evidence
    }))
  }')"

printf '%s\n' "$summary_json" >"$SUMMARY_JSON"

pass_count="$(jq '[.gates[] | select(.status == "pass")] | length' "$SUMMARY_JSON")"
warn_count="$(jq '[.gates[] | select(.status == "warn")] | length' "$SUMMARY_JSON")"
blocked_count="$(jq '.human_blockers | length' "$SUMMARY_JSON")"
case "$overall_status" in
  passed) status_color="#137333" ;;
  warn) status_color="#b06000" ;;
  *) status_color="#a50e0e" ;;
esac

cat >"$PREFLIGHT_SVG" <<EOF
<svg xmlns="http://www.w3.org/2000/svg" width="960" height="540" viewBox="0 0 960 540" role="img" aria-labelledby="title desc">
  <title id="title">Paw Patrol production preflight summary</title>
  <desc id="desc">Factual preflight visual generated from summary.json.</desc>
  <rect width="960" height="540" fill="#f7f5ef"/>
  <rect x="40" y="36" width="880" height="468" rx="8" fill="#ffffff" stroke="#d5d2c6"/>
  <text x="70" y="86" font-family="ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, Segoe UI, sans-serif" font-size="30" font-weight="700" fill="#202124">Paw Patrol Production Preflight</text>
  <text x="70" y="122" font-family="ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, Segoe UI, sans-serif" font-size="16" fill="#64615a">Non-mutating readiness check for Railway, launchd, Temper, webhooks, and local Codex.</text>
  <rect x="70" y="158" width="220" height="112" rx="8" fill="${status_color}" opacity="0.12" stroke="${status_color}"/>
  <text x="94" y="198" font-family="ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, Segoe UI, sans-serif" font-size="15" fill="#64615a">Overall Status</text>
  <text x="94" y="238" font-family="ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, Segoe UI, sans-serif" font-size="34" font-weight="700" fill="${status_color}">${overall_status}</text>
  <rect x="320" y="158" width="160" height="112" rx="8" fill="#edf4ee" stroke="#b7d3bc"/>
  <text x="344" y="198" font-family="ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, Segoe UI, sans-serif" font-size="15" fill="#64615a">Passed</text>
  <text x="344" y="238" font-family="ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, Segoe UI, sans-serif" font-size="34" font-weight="700" fill="#137333">${pass_count}</text>
  <rect x="510" y="158" width="160" height="112" rx="8" fill="#fff4e5" stroke="#e7c17c"/>
  <text x="534" y="198" font-family="ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, Segoe UI, sans-serif" font-size="15" fill="#64615a">Warnings</text>
  <text x="534" y="238" font-family="ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, Segoe UI, sans-serif" font-size="34" font-weight="700" fill="#b06000">${warn_count}</text>
  <rect x="700" y="158" width="160" height="112" rx="8" fill="#fce8e6" stroke="#e6aaa4"/>
  <text x="724" y="198" font-family="ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, Segoe UI, sans-serif" font-size="15" fill="#64615a">Human Blockers</text>
  <text x="724" y="238" font-family="ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, Segoe UI, sans-serif" font-size="34" font-weight="700" fill="#a50e0e">${blocked_count}</text>
  <text x="70" y="326" font-family="ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, Segoe UI, sans-serif" font-size="18" font-weight="700" fill="#202124">Next Gate</text>
  <text x="70" y="360" font-family="ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, Segoe UI, sans-serif" font-size="16" fill="#202124">Resolve human_blockers in summary.json, then run production-readiness.sh with execution disabled.</text>
  <text x="70" y="410" font-family="ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, Segoe UI, sans-serif" font-size="18" font-weight="700" fill="#202124">Worker</text>
  <text x="70" y="444" font-family="ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, Segoe UI, sans-serif" font-size="16" fill="#202124">${WORKER_ID}</text>
  <text x="70" y="474" font-family="ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, Segoe UI, sans-serif" font-size="13" fill="#64615a">Source: ${SUMMARY_JSON}</text>
</svg>
EOF

cat >"$PROOF_MD" <<EOF
# Paw Patrol Production Preflight Proof

This preflight does not mutate Railway, launchd, or Temper. It records the
current Mac mini / checkout readiness and makes the remaining human blockers
visible before \`production-readiness.sh\` or \`launchctl bootstrap\` are used.

## Flow

\`\`\`mermaid
flowchart TD
    A["Run production-preflight.sh"] --> B["Collect local machine checks"]
    A --> C["Collect env and human-gate checks"]
    A --> D["Collect optional Railway/GitHub read-only checks"]
    B --> E{"Any blocked gates?"}
    C --> E
    D --> E
    E -->|"yes"| F["Write summary.json with human_blockers"]
    E -->|"no"| G["Ready for production-readiness.sh"]
    F --> H["Human supplies secrets/approval"]
    H --> G
\`\`\`

## Summary

- Status: \`${overall_status}\`
- Worker ID: \`${WORKER_ID}\`
- Proof directory: \`${PROOF_DIR}\`
- Visual summary: \`${PREFLIGHT_SVG}\`
- Machine summary: \`${SUMMARY_JSON}\`
- Gate table: \`${GATES_TSV}\`

Run with \`STRICT=1\` when a blocked gate should fail the command.

## Machine Summary

\`\`\`json
${summary_json}
\`\`\`
EOF

printf '%s\n' "$summary_json"
log "proof bundle: ${PROOF_DIR}"

if [[ "$STRICT" == "1" && "$overall_status" == "blocked" ]]; then
  log "STRICT=1 and blocked gates remain"
  exit 1
fi
