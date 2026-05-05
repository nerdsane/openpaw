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
if [ "${1:-}" = "--skip-git-repo-check" ]; then
  shift
fi
prompt="${1:-}"

case "$prompt" in
  "PAW_CODEX_DOCTOR_EXEC_SMOKE:"*)
    echo "PAW_CODEX_DOCTOR_EXEC_OK"
    ;;
  "You are the independent reviewer"*)
    echo "SUMMARY: Fake reviewer approved the deterministic worker E2E output."
    echo "LIVE_E2E: Confirmed the fake implementer marker exists in the assigned worktree."
    echo "VERDICT: approve"
    ;;
  *)
    printf '%s\n' "$prompt" > .paw-fake-codex-implementation
    echo "Fake implementation completed and wrote .paw-fake-codex-implementation."
    ;;
esac
