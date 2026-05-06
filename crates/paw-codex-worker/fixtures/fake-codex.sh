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
  *)
    printf '%s\n' "$prompt" > .paw-fake-codex-implementation
    echo "Fake implementation completed and wrote .paw-fake-codex-implementation."
    ;;
esac
