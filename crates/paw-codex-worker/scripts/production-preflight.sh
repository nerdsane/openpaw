#!/usr/bin/env bash
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel)"
STAMP="$(date -u +%Y%m%dT%H%M%SZ)"
PROOF_DIR="${PROOF_DIR:-/tmp/paw-patrol-production-preflight-${STAMP}-$$}"
GATES_TSV="${PROOF_DIR}/gates.tsv"
SUMMARY_JSON="${PROOF_DIR}/summary.json"
HUMAN_BLOCKERS_JSON="${PROOF_DIR}/human-blockers.json"
PROOF_MD="${PROOF_DIR}/proof.md"
PREFLIGHT_SVG="${PROOF_DIR}/preflight.svg"
OPERATOR_HANDOFF_MD="${PROOF_DIR}/operator-handoff.md"
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
railway_candidates_json='[]'

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
GIT_BRANCH="$(cat "${PROOF_DIR}/git-branch.txt" 2>/dev/null || true)"
GIT_HEAD="$(cat "${PROOF_DIR}/git-head.txt" 2>/dev/null || true)"
GIT_STATUS_SHORT="$(cat "${PROOF_DIR}/git-status.txt" 2>/dev/null || true)"
if [[ -s "${PROOF_DIR}/git-status.txt" ]]; then
  add_gate "git:clean" "warn" "worktree has local changes; production activation should use a reviewed checkout" "${PROOF_DIR}/git-status.txt"
  GIT_CLEAN="false"
else
  add_gate "git:clean" "pass" "worktree is clean" "${PROOF_DIR}/git-status.txt"
  GIT_CLEAN="true"
fi

if capture_command "${PROOF_DIR}/git-main-ancestry.txt" bash "${ROOT}/crates/paw-codex-worker/scripts/production-git-ancestry-guard.sh"; then
  add_gate "git:contains_main" "pass" "checkout contains the current main branch before production activation" "${PROOF_DIR}/git-main-ancestry.txt"
else
  add_gate "git:contains_main" "blocked" "checkout does not contain current main; production activation would risk dropping merged fixes" "${PROOF_DIR}/git-main-ancestry.txt"
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

if [[ -n "${PATROL_OPERATOR_TOKEN:-}" ]]; then
  add_gate "env:patrol_operator_token" "pass" "PATROL_OPERATOR_TOKEN is set" "value intentionally not printed"
else
  add_gate "env:patrol_operator_token" "blocked" "PATROL_OPERATOR_TOKEN is missing; production-observe-only.sh needs an operator/system token for the low-risk proof write" "human input required"
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

    if capture_command "${PROOF_DIR}/railway-projects.json" railway project list --json; then
      if railway_candidates_json="$(jq '
        map({
          project_id: (.id // ""),
          project_name: (.name // ""),
          likely_match: (
            ((.name // "") | test("temper|paw"; "i")) or
            ([.services.edges[]?.node.name // empty] | map(test("temper|paw"; "i")) | any)
          ),
          environments: [
            .environments.edges[]?.node
            | {
              environment_id: (.id // ""),
              environment_name: (.name // ""),
              can_access: (.canAccess // false),
              service_ids: [.serviceInstances.edges[]?.node.serviceId // empty]
            }
          ],
          services: [
            .services.edges[]?.node
            | {
              service_id: (.id // ""),
              service_name: (.name // "")
            }
          ]
        })
      ' "${PROOF_DIR}/railway-projects.json")"; then
        printf '%s\n' "$railway_candidates_json" >"${PROOF_DIR}/railway-candidates.json"
        railway_candidate_count="$(jq 'length' <<<"$railway_candidates_json")"
        if [[ "$railway_candidate_count" -gt 0 ]]; then
          add_gate "railway:candidate_projects" "pass" "captured ${railway_candidate_count} read-only Railway project/service candidate(s)" "${PROOF_DIR}/railway-candidates.json"
        else
          add_gate "railway:candidate_projects" "warn" "railway project list succeeded but returned no accessible project candidates" "${PROOF_DIR}/railway-projects.json"
        fi
      else
        railway_candidates_json='[]'
        add_gate "railway:candidate_projects" "warn" "railway project list output was not parseable as expected JSON" "${PROOF_DIR}/railway-projects.json"
      fi
    else
      add_gate "railway:candidate_projects" "warn" "could not list Railway projects for read-only candidate discovery" "${PROOF_DIR}/railway-projects.json"
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
        if jq -e '.isDraft == true' "${PROOF_DIR}/temper-pr-216.json" >/dev/null 2>&1; then
          temper_pr_state="still draft and unmerged"
        else
          temper_pr_state="ready for review but unmerged"
        fi
        add_gate "github:temper_pr_216" "blocked" "Temper Cedar dependency PR #216 is ${temper_pr_state}; set CONFIRM_TEMPER_PIN_OK=1 only if production may use the pinned Temper revision" "${PROOF_DIR}/temper-pr-216.json"
      fi
    else
      add_gate "github:temper_pr_216" "warn" "could not inspect Temper PR #216" "${PROOF_DIR}/temper-pr-216.json"
    fi

    if capture_command "${PROOF_DIR}/temperpaw-pr-218.json" gh pr view 218 --repo nerdsane/temperpaw --json url,isDraft,state,mergeStateStatus,headRefOid,statusCheckRollup; then
      if jq -e '.state == "MERGED"' "${PROOF_DIR}/temperpaw-pr-218.json" >/dev/null 2>&1; then
        add_gate "github:temperpaw_pr_218" "pass" "TemperPaw PR #218 is merged" "${PROOF_DIR}/temperpaw-pr-218.json"
      elif jq -e '
        .isDraft == false and
        .mergeStateStatus == "CLEAN" and
        ((.statusCheckRollup // []) | length > 0) and
        all(.statusCheckRollup[];
          ((.status // "") == "COMPLETED") and
          ((.conclusion // "") as $conclusion | ["SUCCESS", "SKIPPED", "NEUTRAL"] | index($conclusion) != null)
        )
      ' "${PROOF_DIR}/temperpaw-pr-218.json" >/dev/null 2>&1; then
        if [[ "${CONFIRM_TEMPERPAW_PR_OK:-0}" == "1" ]]; then
          add_gate "github:temperpaw_pr_218" "pass" "operator confirmed the clean and green TemperPaw PR #218 head is approved for production cutover while unmerged" "${PROOF_DIR}/temperpaw-pr-218.json"
        else
          add_gate "github:temperpaw_pr_218" "blocked" "TemperPaw PR #218 is clean and green but unmerged; set CONFIRM_TEMPERPAW_PR_OK=1 only if production may deploy this PR head" "${PROOF_DIR}/temperpaw-pr-218.json"
        fi
      else
        add_gate "github:temperpaw_pr_218" "blocked" "TemperPaw PR #218 is not merged and not a confirmed clean/green production candidate" "${PROOF_DIR}/temperpaw-pr-218.json"
      fi
    else
      add_gate "github:temperpaw_pr_218" "warn" "could not inspect TemperPaw PR #218" "${PROOF_DIR}/temperpaw-pr-218.json"
    fi

    if capture_command "${PROOF_DIR}/temperpaw-pr-220.json" gh pr view 220 --repo nerdsane/temperpaw --json url,isDraft,state,mergeStateStatus,headRefOid; then
      if jq -e '.state == "MERGED"' "${PROOF_DIR}/temperpaw-pr-220.json" >/dev/null 2>&1; then
        add_gate "github:temperpaw_pr_220" "pass" "TemperPaw PR #220 is merged" "${PROOF_DIR}/temperpaw-pr-220.json"
      else
        add_gate "github:temperpaw_pr_220" "blocked" "TemperPaw PR #220 is not merged; production image may be missing Patrol WASM modules" "${PROOF_DIR}/temperpaw-pr-220.json"
      fi
    else
      add_gate "github:temperpaw_pr_220" "warn" "could not inspect TemperPaw PR #220" "${PROOF_DIR}/temperpaw-pr-220.json"
    fi

    if capture_command "${PROOF_DIR}/temperpaw-pr-221.json" gh pr view 221 --repo nerdsane/temperpaw --json url,isDraft,state,mergeStateStatus,headRefOid; then
      if jq -e '.state == "MERGED"' "${PROOF_DIR}/temperpaw-pr-221.json" >/dev/null 2>&1; then
        add_gate "github:temperpaw_pr_221" "pass" "TemperPaw PR #221 is merged" "${PROOF_DIR}/temperpaw-pr-221.json"
      else
        add_gate "github:temperpaw_pr_221" "blocked" "TemperPaw PR #221 is not merged; Mac mini bootstrap handoff may be missing" "${PROOF_DIR}/temperpaw-pr-221.json"
      fi
    else
      add_gate "github:temperpaw_pr_221" "warn" "could not inspect TemperPaw PR #221" "${PROOF_DIR}/temperpaw-pr-221.json"
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
  --arg git_head "$GIT_HEAD" \
  --arg git_branch "$GIT_BRANCH" \
  --arg git_status_short "$GIT_STATUS_SHORT" \
  --arg git_clean "$GIT_CLEAN" \
  --arg strict "$STRICT" \
  --arg check_railway "$CHECK_RAILWAY" \
  --arg check_github "$CHECK_GITHUB" \
  --argjson gates "$gates_json" \
  --argjson railway_candidates "$railway_candidates_json" \
  '{
    status: $status,
    proof_dir: $proof_dir,
    worker_id: $worker_id,
    repo_root: $repo_root,
    workspace_root: $workspace_root,
    launchd_label: $launchd_label,
    git_head: $git_head,
    git_branch: $git_branch,
    git_status_short: $git_status_short,
    git_clean: ($git_clean == "true"),
    strict: ($strict == "1"),
    checks: {
      railway: ($check_railway == "1"),
      github: ($check_github == "1")
    },
    railway: {
      candidates: $railway_candidates
    },
    gates: $gates,
    human_blockers: ($gates | map(select(.status == "blocked") | {
      gate: .gate,
      detail: .detail,
      evidence: .evidence
    }))
  }')"

printf '%s\n' "$railway_candidates_json" >"${PROOF_DIR}/railway-candidates.json"
printf '%s\n' "$summary_json" >"$SUMMARY_JSON"
jq '.human_blockers' "$SUMMARY_JSON" >"$HUMAN_BLOCKERS_JSON"

pass_count="$(jq '[.gates[] | select(.status == "pass")] | length' "$SUMMARY_JSON")"
warn_count="$(jq '[.gates[] | select(.status == "warn")] | length' "$SUMMARY_JSON")"
blocked_count="$(jq '.human_blockers | length' "$SUMMARY_JSON")"
railway_candidate_count="$(jq '.railway.candidates | length' "$SUMMARY_JSON")"
case "$overall_status" in
  passed) status_color="#137333" ;;
  warn) status_color="#b06000" ;;
  *) status_color="#a50e0e" ;;
esac

railway_candidates_md="$(jq -r '
  if length == 0 then
    "- No Railway project candidates were captured in this run."
  else
    .[]
    | "- \(.project_name) (`\(.project_id)`) - services: \(([.services[].service_name] | join(", ")) // "none"); environments: \(([.environments[].environment_name] | join(", ")) // "none"); likely match: \(.likely_match)"
  end
' <<<"$railway_candidates_json")"

human_blockers_md="$(jq -r '
  if (.human_blockers | length) == 0 then
    "| Gate | Decision Needed | Evidence |\n| --- | --- | --- |\n| none | No blocked gates remain. | summary.json |"
  else
    "| Gate | Decision Needed | Evidence |\n| --- | --- | --- |\n" +
    (.human_blockers
      | map("| `\(.gate)` | \(.detail) | \(.evidence) |")
      | join("\n"))
  end
' "$SUMMARY_JSON")"

railway_choice_md="$(jq -r '
  if (.railway.candidates | length) == 0 then
    "- No candidates were captured. Re-run with `CHECK_RAILWAY=1` after Railway CLI login."
  else
    .railway.candidates[]
    | "- \(.project_name) (`\(.project_id)`) - likely match: \(.likely_match); environments: \(([.environments[].environment_name] | join(", ")) // "none"); services: \(([.services[].service_name] | join(", ")) // "none")"
  end
' "$SUMMARY_JSON")"

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
  <text x="70" y="390" font-family="ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, Segoe UI, sans-serif" font-size="16" fill="#202124">Railway candidates captured: ${railway_candidate_count}</text>
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
- Git head: \`${GIT_HEAD}\`
- Git branch: \`${GIT_BRANCH}\`
- Git clean: \`${GIT_CLEAN}\`
- Proof directory: \`${PROOF_DIR}\`
- Visual summary: \`${PREFLIGHT_SVG}\`
- Operator handoff: \`${OPERATOR_HANDOFF_MD}\`
- Machine summary: \`${SUMMARY_JSON}\`
- Standalone blocker list: \`${HUMAN_BLOCKERS_JSON}\`
- Gate table: \`${GATES_TSV}\`
- Railway candidates: \`${PROOF_DIR}/railway-candidates.json\`

Run with \`STRICT=1\` when a blocked gate should fail the command.

## Railway Candidate Projects

These are captured with \`railway project list --json\` only. The preflight does
not run \`railway link\`, select an environment, set variables, or deploy.

${railway_candidates_md}

## Machine Summary

\`\`\`json
${summary_json}
\`\`\`
EOF

cat >"$OPERATOR_HANDOFF_MD" <<EOF
# Paw Patrol Production Operator Handoff

This file is generated by \`production-preflight.sh\`. It is a human decision
sheet for the remaining production activation gates. It does not contain secret
values and the preflight did not mutate Railway, launchd, or Temper.

## Current Status

- Status: \`${overall_status}\`
- Worker ID: \`${WORKER_ID}\`
- Git head: \`${GIT_HEAD}\`
- Git branch: \`${GIT_BRANCH}\`
- Git clean: \`${GIT_CLEAN}\`
- Human blockers: \`${blocked_count}\`
- Railway candidates captured: \`${railway_candidate_count}\`
- Machine summary: \`${SUMMARY_JSON}\`
- Standalone blocker list: \`${HUMAN_BLOCKERS_JSON}\`
- Visual summary: \`${PREFLIGHT_SVG}\`

## Human Blocker Decisions

${human_blockers_md}

## Railway Project Choice

Choose the intended production project/service from the candidates below before
linking this checkout. The preflight only ran \`railway project list --json\`;
it did not run \`railway link\`, choose an environment, set variables, or deploy.

${railway_choice_md}

Command template after a human chooses the correct production target:

\`\`\`sh
railway link <project_id>
railway environment link production
railway service link <service_id>
\`\`\`

## Secret And Approval Inputs

Fill these in locally only. Do not paste the values into a PR, issue, or proof
file.

\`\`\`sh
export TEMPER_URL='https://<production-temperpaw-url>'
export WORKER_TOKEN='<temper-worker-token>'
export PATROL_OPERATOR_TOKEN='<temper-operator-token>'
export CONFIRM_LOCAL_CODEX_WORKER_ID='${WORKER_ID}'
export PATROL_DATADOG_WEBHOOK_SECRET='<datadog-webhook-secret>'
export PATROL_DISCORD_WEBHOOK_SECRET='<discord-webhook-secret>'
export PATROL_GITHUB_WEBHOOK_SECRET='<github-webhook-secret>'

# Use only after the Temper dependency decision is explicit.
export CONFIRM_TEMPER_PIN_OK='1'

# Use only after the TemperPaw PR #218 head is explicitly approved for
# production while it is still unmerged.
export CONFIRM_TEMPERPAW_PR_OK='1'
\`\`\`

## Next Safe Commands

Re-run preflight in strict mode after the decisions above are made:

\`\`\`sh
STRICT=1 CHECK_RAILWAY=1 CHECK_GITHUB=1 \\
crates/paw-codex-worker/scripts/production-preflight.sh
\`\`\`

After strict preflight passes, render launchd for review with execution disabled.
This is the launchd approval gate; review the generated plist before loading it.

\`\`\`sh
PAW_CODEX_ENABLE_EXECUTION=0 \\
PAW_CODEX_DOCTOR_EXEC_SMOKE=1 \\
WRITE_LAUNCHD_PLIST=1 \\
INSTALL_LAUNCHD=0 \\
crates/paw-codex-worker/scripts/production-readiness.sh
\`\`\`

## Hold Points

- Do not load launchd until strict preflight passes and the plist has human
  launchd approval.
- Do not enable worker execution until observe-only proof has passed.
- Do not merge or remove the Temper pin until the Temper PR #216 decision is
  explicit.
EOF

printf '%s\n' "$summary_json"
log "proof bundle: ${PROOF_DIR}"

if [[ "$STRICT" == "1" && "$overall_status" == "blocked" ]]; then
  log "STRICT=1 and blocked gates remain"
  exit 1
fi
