#!/usr/bin/env bash
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel)"
WORKFLOW_DIR="${ROOT}/.github/workflows"

if [[ ! -d "$WORKFLOW_DIR" ]]; then
  printf '[ci-actions-runtime-smoke] missing workflow directory: %s\n' "$WORKFLOW_DIR" >&2
  exit 1
fi

deprecated_uses="$(
  grep -RInE 'uses:[[:space:]]+actions/(checkout|setup-node)@v[1-4]([[:space:]]|$|\.)' "$WORKFLOW_DIR" || true
)"
if [[ -n "$deprecated_uses" ]]; then
  printf '[ci-actions-runtime-smoke] deprecated Node 20-era GitHub actions found:\n%s\n' "$deprecated_uses" >&2
  exit 1
fi

printf '[ci-actions-runtime-smoke] GitHub action runtime versions are current enough\n'
