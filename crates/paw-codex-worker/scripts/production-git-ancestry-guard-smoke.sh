#!/usr/bin/env bash
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel)"
SCRIPT="${ROOT}/crates/paw-codex-worker/scripts/production-git-ancestry-guard.sh"
TMP_DIR="$(mktemp -d)"

cleanup() {
  rm -rf "$TMP_DIR"
}
trap cleanup EXIT

log() {
  printf '[paw-codex-git-ancestry-smoke] %s\n' "$*"
}

fail() {
  log "$*"
  exit 1
}

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || fail "missing required command: $1"
}

require_cmd git

REPO="${TMP_DIR}/repo"
mkdir -p "$REPO"
git -C "$REPO" init -q
git -C "$REPO" config user.email "patrol@example.invalid"
git -C "$REPO" config user.name "Paw Patrol Smoke"
git -C "$REPO" checkout -q -b main
printf 'base\n' >"${REPO}/state.txt"
git -C "$REPO" add state.txt
git -C "$REPO" commit -q -m "base"
BASE_SHA="$(git -C "$REPO" rev-parse HEAD)"
git -C "$REPO" checkout -q -b stale "$BASE_SHA"
git -C "$REPO" checkout -q main
printf 'main fix\n' >>"${REPO}/state.txt"
git -C "$REPO" commit -am "main fix" -q
MAIN_SHA="$(git -C "$REPO" rev-parse HEAD)"

ROOT="$REPO" MAIN_REF=main FETCH_MAIN=0 "$SCRIPT" >/dev/null

git -C "$REPO" checkout -q stale
if ROOT="$REPO" MAIN_REF=main FETCH_MAIN=0 "$SCRIPT" >/dev/null 2>&1; then
  fail "stale branch unexpectedly passed ancestry guard"
fi

PAW_ALLOW_STALE_MAIN_DEPLOY=1 ROOT="$REPO" MAIN_REF=main FETCH_MAIN=0 "$SCRIPT" >/dev/null

if ROOT="$REPO" MAIN_REF=main REQUIRED_MAIN_COMMIT="$MAIN_SHA" FETCH_MAIN=0 "$SCRIPT" >/dev/null 2>&1; then
  fail "stale branch unexpectedly passed required commit guard"
fi

git -C "$REPO" merge --no-edit main >/dev/null
ROOT="$REPO" MAIN_REF=main REQUIRED_MAIN_COMMIT="$MAIN_SHA" FETCH_MAIN=0 "$SCRIPT" >/dev/null

log "production git ancestry guard smoke passed"
