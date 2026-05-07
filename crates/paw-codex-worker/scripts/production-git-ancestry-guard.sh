#!/usr/bin/env bash
set -euo pipefail

ROOT="${ROOT:-$(git rev-parse --show-toplevel)}"
MAIN_REF="${MAIN_REF:-origin/main}"
FETCH_MAIN="${FETCH_MAIN:-1}"
REQUIRED_MAIN_COMMIT="${REQUIRED_MAIN_COMMIT:-}"
PAW_ALLOW_STALE_MAIN_DEPLOY="${PAW_ALLOW_STALE_MAIN_DEPLOY:-0}"

log() {
  printf '[paw-codex-git-ancestry-guard] %s\n' "$*"
}

fail() {
  log "$*"
  exit 1
}

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || fail "missing required command: $1"
}

require_cmd git

git -C "$ROOT" rev-parse --is-inside-work-tree >/dev/null 2>&1 ||
  fail "ROOT is not a git worktree: ${ROOT}"

if [[ "$FETCH_MAIN" == "1" ]] && git -C "$ROOT" remote get-url origin >/dev/null 2>&1; then
  git -C "$ROOT" fetch --quiet origin main ||
    fail "could not fetch origin/main before production ancestry check"
fi

HEAD_SHA="$(git -C "$ROOT" rev-parse HEAD)"
BRANCH="$(git -C "$ROOT" branch --show-current 2>/dev/null || true)"

git -C "$ROOT" rev-parse --verify "${MAIN_REF}^{commit}" >/dev/null 2>&1 ||
  fail "main ref is not available: ${MAIN_REF}"

MAIN_SHA="$(git -C "$ROOT" rev-parse "${MAIN_REF}^{commit}")"

if [[ -n "$REQUIRED_MAIN_COMMIT" ]]; then
  git -C "$ROOT" rev-parse --verify "${REQUIRED_MAIN_COMMIT}^{commit}" >/dev/null 2>&1 ||
    fail "required main commit is not available in this checkout: ${REQUIRED_MAIN_COMMIT}"
  if ! git -C "$ROOT" merge-base --is-ancestor "$REQUIRED_MAIN_COMMIT" HEAD; then
    fail "HEAD ${HEAD_SHA} does not contain required main commit ${REQUIRED_MAIN_COMMIT}"
  fi
fi

if git -C "$ROOT" merge-base --is-ancestor "$MAIN_REF" HEAD; then
  log "pass: HEAD ${HEAD_SHA} contains ${MAIN_REF} ${MAIN_SHA}"
  exit 0
fi

if [[ "$PAW_ALLOW_STALE_MAIN_DEPLOY" == "1" ]]; then
  log "override: HEAD ${HEAD_SHA} on ${BRANCH:-detached} does not contain ${MAIN_REF} ${MAIN_SHA}"
  exit 0
fi

fail "blocked: HEAD ${HEAD_SHA} on ${BRANCH:-detached} does not contain ${MAIN_REF} ${MAIN_SHA}"
