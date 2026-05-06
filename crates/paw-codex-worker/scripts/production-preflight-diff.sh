#!/usr/bin/env bash
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel)"
STAMP="$(date -u +%Y%m%dT%H%M%SZ)"
BASELINE_SUMMARY="${1:-${BASELINE_SUMMARY:-}}"
CURRENT_SUMMARY="${2:-${CURRENT_SUMMARY:-}}"
PROOF_DIR="${3:-${PROOF_DIR:-/tmp/paw-patrol-production-preflight-diff-${STAMP}-$$}}"
SUMMARY_JSON="${PROOF_DIR}/summary.json"
PROOF_MD="${PROOF_DIR}/proof.md"
DIFF_SVG="${PROOF_DIR}/preflight-diff.svg"

log() {
  printf '[paw-codex-preflight-diff] %s\n' "$*"
}

fail() {
  log "$*" >&2
  exit 1
}

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || fail "missing required command: $1"
}

usage() {
  cat <<EOF
Usage: $(basename "$0") <baseline-summary.json> <current-summary.json> [proof-dir]

Compares two production-preflight summary.json files and writes:
- summary.json
- proof.md
- preflight-diff.svg

The script is read-only. It does not mutate Railway, launchd, Temper, or git.
EOF
}

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  usage
  exit 0
fi

require_cmd git
require_cmd jq

[[ -n "$BASELINE_SUMMARY" ]] || {
  usage >&2
  fail "missing baseline summary"
}

[[ -n "$CURRENT_SUMMARY" ]] || {
  usage >&2
  fail "missing current summary"
}

[[ -f "$BASELINE_SUMMARY" ]] || fail "baseline summary does not exist: ${BASELINE_SUMMARY}"
[[ -f "$CURRENT_SUMMARY" ]] || fail "current summary does not exist: ${CURRENT_SUMMARY}"

mkdir -p "$PROOF_DIR"

summary_json="$(jq -n \
  --arg baseline_path "$BASELINE_SUMMARY" \
  --arg current_path "$CURRENT_SUMMARY" \
  --arg proof_dir "$PROOF_DIR" \
  --slurpfile baseline "$BASELINE_SUMMARY" \
  --slurpfile current "$CURRENT_SUMMARY" \
  '
  def blocker_map($s):
    reduce (($s.human_blockers // [])[]) as $b ({}; .[$b.gate] = $b);

  def gate_map($s):
    reduce (($s.gates // [])[]) as $g ({}; .[$g.gate] = $g);

  def candidate_map($s):
    reduce (($s.railway.candidates // [])[]) as $c ({}; .[$c.project_id] = $c);

  def values_for_keys($map; $keys):
    [$keys[] as $key | $map[$key]];

  def sorted_keys($map):
    [$map | keys[]] | sort;

  ($baseline[0]) as $before |
  ($current[0]) as $after |
  (blocker_map($before)) as $before_blockers |
  (blocker_map($after)) as $after_blockers |
  (gate_map($before)) as $before_gates |
  (gate_map($after)) as $after_gates |
  (candidate_map($before)) as $before_candidates |
  (candidate_map($after)) as $after_candidates |
  (sorted_keys($before_blockers)) as $before_blocker_keys |
  (sorted_keys($after_blockers)) as $after_blocker_keys |
  (sorted_keys($before_gates + $after_gates)) as $gate_keys |
  (sorted_keys($before_candidates)) as $before_candidate_keys |
  (sorted_keys($after_candidates)) as $after_candidate_keys |
  (values_for_keys($before_blockers; ($before_blocker_keys - $after_blocker_keys))) as $resolved_blockers |
  (values_for_keys($after_blockers; ($after_blocker_keys - $before_blocker_keys))) as $new_blockers |
  (values_for_keys($after_blockers; ($after_blocker_keys - ($after_blocker_keys - $before_blocker_keys)))) as $unchanged_blockers |
  [
    $gate_keys[] as $gate |
    ($before_gates[$gate] // null) as $before_gate |
    ($after_gates[$gate] // null) as $after_gate |
    select(($before_gate.status // "missing") != ($after_gate.status // "missing") or ($before_gate.detail // "") != ($after_gate.detail // "")) |
    {
      gate: $gate,
      before_status: ($before_gate.status // "missing"),
      after_status: ($after_gate.status // "missing"),
      before_detail: ($before_gate.detail // ""),
      after_detail: ($after_gate.detail // "")
    }
  ] as $changed_gates |
  (values_for_keys($after_candidates; ($after_candidate_keys - $before_candidate_keys))) as $added_candidates |
  (values_for_keys($before_candidates; ($before_candidate_keys - $after_candidate_keys))) as $removed_candidates |
  {
    status: (
      if ($new_blockers | length) > 0 then "attention"
      elif ($resolved_blockers | length) > 0 then "improved"
      elif ($changed_gates | length) > 0 or ($added_candidates | length) > 0 or ($removed_candidates | length) > 0 then "changed"
      else "unchanged"
      end
    ),
    proof_dir: $proof_dir,
    baseline_summary: $baseline_path,
    current_summary: $current_path,
    baseline_status: ($before.status // "unknown"),
    current_status: ($after.status // "unknown"),
    counts: {
      baseline_blockers: ($before_blockers | length),
      current_blockers: ($after_blockers | length),
      resolved_blockers: ($resolved_blockers | length),
      new_blockers: ($new_blockers | length),
      unchanged_blockers: ($unchanged_blockers | length),
      changed_gates: ($changed_gates | length),
      railway_candidates_added: ($added_candidates | length),
      railway_candidates_removed: ($removed_candidates | length)
    },
    resolved_blockers: $resolved_blockers,
    new_blockers: $new_blockers,
    unchanged_blockers: $unchanged_blockers,
    changed_gates: $changed_gates,
    railway_candidate_changes: {
      added: $added_candidates,
      removed: $removed_candidates
    }
  }')"

printf '%s\n' "$summary_json" >"$SUMMARY_JSON"

status="$(jq -r '.status' "$SUMMARY_JSON")"
resolved_count="$(jq -r '.counts.resolved_blockers' "$SUMMARY_JSON")"
new_count="$(jq -r '.counts.new_blockers' "$SUMMARY_JSON")"
unchanged_count="$(jq -r '.counts.unchanged_blockers' "$SUMMARY_JSON")"
changed_count="$(jq -r '.counts.changed_gates' "$SUMMARY_JSON")"
added_candidates_count="$(jq -r '.counts.railway_candidates_added' "$SUMMARY_JSON")"
removed_candidates_count="$(jq -r '.counts.railway_candidates_removed' "$SUMMARY_JSON")"

case "$status" in
  improved) status_color="#137333" ;;
  unchanged) status_color="#4b5563" ;;
  changed) status_color="#b06000" ;;
  *) status_color="#a50e0e" ;;
esac

table_for() {
  local query="$1"
  local empty="$2"
  jq -r "$query" "$SUMMARY_JSON" | sed '/^$/d' >"${PROOF_DIR}/.table.tmp"
  if [[ -s "${PROOF_DIR}/.table.tmp" ]]; then
    cat "${PROOF_DIR}/.table.tmp"
  else
    printf '%s\n' "$empty"
  fi
  rm -f "${PROOF_DIR}/.table.tmp"
}

resolved_table="$(table_for '
  .resolved_blockers[]
  | "| `\(.gate)` | \(.detail) | \(.evidence) |"
' "| none | No blockers were resolved. | |")"

new_table="$(table_for '
  .new_blockers[]
  | "| `\(.gate)` | \(.detail) | \(.evidence) |"
' "| none | No new blockers appeared. | |")"

unchanged_table="$(table_for '
  .unchanged_blockers[]
  | "| `\(.gate)` | \(.detail) | \(.evidence) |"
' "| none | No blockers remain unchanged. | |")"

changed_gate_table="$(table_for '
  .changed_gates[]
  | "| `\(.gate)` | \(.before_status) | \(.after_status) | \(.after_detail) |"
' "| none | No gate status/detail drift. | | |")"

candidate_table="$(table_for '
  (.railway_candidate_changes.added[]? | "| added | `\(.project_id)` | \(.project_name) |") ,
  (.railway_candidate_changes.removed[]? | "| removed | `\(.project_id)` | \(.project_name) |")
' "| none | No Railway candidate drift. | |")"

cat >"$DIFF_SVG" <<EOF
<svg xmlns="http://www.w3.org/2000/svg" width="960" height="540" viewBox="0 0 960 540" role="img" aria-labelledby="title desc">
  <title id="title">Paw Patrol preflight diff summary</title>
  <desc id="desc">Factual preflight diff generated from two summary.json files.</desc>
  <rect width="960" height="540" fill="#f7f5ef"/>
  <rect x="40" y="36" width="880" height="468" rx="8" fill="#ffffff" stroke="#d5d2c6"/>
  <text x="70" y="86" font-family="ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, Segoe UI, sans-serif" font-size="30" font-weight="700" fill="#202124">Paw Patrol Preflight Diff</text>
  <text x="70" y="122" font-family="ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, Segoe UI, sans-serif" font-size="16" fill="#64615a">Read-only comparison of production preflight summaries.</text>
  <rect x="70" y="158" width="220" height="112" rx="8" fill="${status_color}" opacity="0.12" stroke="${status_color}"/>
  <text x="94" y="198" font-family="ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, Segoe UI, sans-serif" font-size="15" fill="#64615a">Diff Status</text>
  <text x="94" y="238" font-family="ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, Segoe UI, sans-serif" font-size="34" font-weight="700" fill="${status_color}">${status}</text>
  <rect x="320" y="158" width="160" height="112" rx="8" fill="#edf4ee" stroke="#b7d3bc"/>
  <text x="344" y="198" font-family="ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, Segoe UI, sans-serif" font-size="15" fill="#64615a">Resolved</text>
  <text x="344" y="238" font-family="ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, Segoe UI, sans-serif" font-size="34" font-weight="700" fill="#137333">${resolved_count}</text>
  <rect x="510" y="158" width="160" height="112" rx="8" fill="#fce8e6" stroke="#e6aaa4"/>
  <text x="534" y="198" font-family="ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, Segoe UI, sans-serif" font-size="15" fill="#64615a">New Blockers</text>
  <text x="534" y="238" font-family="ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, Segoe UI, sans-serif" font-size="34" font-weight="700" fill="#a50e0e">${new_count}</text>
  <rect x="700" y="158" width="160" height="112" rx="8" fill="#fff4e5" stroke="#e7c17c"/>
  <text x="724" y="198" font-family="ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, Segoe UI, sans-serif" font-size="15" fill="#64615a">Unchanged</text>
  <text x="724" y="238" font-family="ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, Segoe UI, sans-serif" font-size="34" font-weight="700" fill="#b06000">${unchanged_count}</text>
  <text x="70" y="326" font-family="ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, Segoe UI, sans-serif" font-size="18" font-weight="700" fill="#202124">Drift</text>
  <text x="70" y="360" font-family="ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, Segoe UI, sans-serif" font-size="16" fill="#202124">Changed gates: ${changed_count}</text>
  <text x="70" y="390" font-family="ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, Segoe UI, sans-serif" font-size="16" fill="#202124">Railway candidates added: ${added_candidates_count}; removed: ${removed_candidates_count}</text>
  <text x="70" y="444" font-family="ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, Segoe UI, sans-serif" font-size="13" fill="#64615a">Source: ${SUMMARY_JSON}</text>
</svg>
EOF

cat >"$PROOF_MD" <<EOF
# Paw Patrol Preflight Diff Proof

This proof compares two \`production-preflight.sh\` summary files. It is
read-only and does not mutate Railway, launchd, Temper, or git.

## Flow

\`\`\`mermaid
flowchart TD
    A["Baseline summary.json"] --> C["production-preflight-diff.sh"]
    B["Current summary.json"] --> C
    C --> D["Resolved blockers"]
    C --> E["New blockers"]
    C --> F["Unchanged blockers"]
    C --> G["Gate and Railway drift"]
\`\`\`

## Summary

- Status: \`${status}\`
- Baseline: \`${BASELINE_SUMMARY}\`
- Current: \`${CURRENT_SUMMARY}\`
- Machine summary: \`${SUMMARY_JSON}\`
- Visual summary: \`${DIFF_SVG}\`

## Resolved Blockers

| Gate | Previous Decision Needed | Previous Evidence |
| --- | --- | --- |
${resolved_table}

## New Blockers

| Gate | Current Decision Needed | Current Evidence |
| --- | --- | --- |
${new_table}

## Unchanged Blockers

| Gate | Current Decision Needed | Current Evidence |
| --- | --- | --- |
${unchanged_table}

## Changed Gates

| Gate | Before | After | Current Detail |
| --- | --- | --- | --- |
${changed_gate_table}

## Railway Candidate Drift

| Change | Project ID | Project Name |
| --- | --- | --- |
${candidate_table}

## Machine Summary

\`\`\`json
${summary_json}
\`\`\`
EOF

printf '%s\n' "$summary_json"
log "proof bundle: ${PROOF_DIR}"
