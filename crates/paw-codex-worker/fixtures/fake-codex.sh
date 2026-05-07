#!/bin/sh
set -eu

if [ "${1:-}" = "--version" ]; then
  echo "fake-codex 0.1.0"
  exit 0
fi

if [ "${1:-}" != "exec" ]; then
  echo "fake-codex only supports --version and exec" >&2
  exit 2
fi

shift
while [ "$#" -gt 0 ]; do
  case "${1:-}" in
    --dangerously-bypass-approvals-and-sandbox)
      shift
      ;;
    --sandbox)
      shift
      if [ "$#" -eq 0 ]; then
        echo "--sandbox requires a value" >&2
        exit 2
      fi
      shift
      ;;
    --cd)
      shift
      if [ "$#" -eq 0 ]; then
        echo "--cd requires a directory" >&2
        exit 2
      fi
      cd "$1"
      shift
      ;;
    --skip-git-repo-check)
      shift
      ;;
    --)
      shift
      break
      ;;
    -*)
      echo "fake-codex does not support option: $1" >&2
      exit 2
      ;;
    *)
      break
      ;;
  esac
done
prompt="${1:-}"

case "$prompt" in
  "PAW_FAKE_CODEX_HANG:"*)
    sleep "${PAW_FAKE_CODEX_SLEEP_SECS:-5}"
    echo "fake codex unexpectedly woke up"
    ;;
  "PAW_CODEX_DOCTOR_EXEC_SMOKE:"*)
    echo "PAW_CODEX_DOCTOR_EXEC_OK"
    ;;
  "You are the independent reviewer"*)
    echo "SUMMARY: Fake reviewer approved the deterministic worker E2E output."
    echo "LIVE_E2E: Confirmed the fake implementer marker exists in the assigned worktree."
    echo "VERDICT: approve"
    ;;
  "You are the Datadog MCP Risk Patrol agent"*)
    cat <<'JSON'
Fake Codex used its Datadog MCP fixture.
DATADOG_PATROL_RESULT_JSON_BEGIN
{
  "summary": "Fake Datadog MCP patrol found one actionable Discord-facing issue.",
  "evidence_scope": [
    {"surface":"monitors","query":"fixture monitor search","result_summary":"one monitor-like signal reviewed","datadog_url":""},
    {"surface":"logs","query":"fixture production error search","result_summary":"one Discord trace leak sample reviewed","datadog_url":""},
    {"surface":"traces","query":"fixture APM trace search","result_summary":"Discord request trace checked","datadog_url":""},
    {"surface":"metrics","query":"fixture error-rate metric search","result_summary":"error-rate metric checked","datadog_url":""},
    {"surface":"incidents","query":"fixture incident search","result_summary":"no open incident in fixture","datadog_url":""},
    {"surface":"dashboards","query":"fixture dashboard review","result_summary":"runtime dashboard reviewed","datadog_url":""}
  ],
  "findings": [
    {
      "title": "Fixture Discord trace leak",
      "severity": "error",
      "risk_lane": "L2",
      "source_url": "",
      "datadog_monitor_id": "",
      "fingerprint": "datadog:mcp:fixture-discord-trace-leak",
      "affected_services": ["openpaw-production"],
      "evidence_json": {"surface":"logs","sample_count":1},
      "work_summary": "Sanitize Discord trace output",
      "work_detail": "Add regression coverage and live Discord-facing proof for sanitized errors.",
      "requires_human_approval": true
    }
  ],
  "residual_risks": ["fixture only"],
  "recommended_next_queries": ["fixture follow-up query"]
}
DATADOG_PATROL_RESULT_JSON_END
JSON
    ;;
  *)
    printf '%s\n' "$prompt" > .paw-fake-codex-implementation
    echo "Fake implementation completed and wrote .paw-fake-codex-implementation."
    ;;
esac
