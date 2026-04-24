#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/../../.." && pwd)"
cd "$repo_root"

fail() {
  printf 'staged_turn_cutover: %s\n' "$1" >&2
  exit 1
}

spec="os-apps/paw-agent/specs/session.ioa.toml"

if rg -n 'name = "call_llm"|module = "llm_caller"|trigger = "call_llm"' "$spec" >/dev/null; then
  fail "legacy call_llm integration is still present in $spec"
fi

for crate in context_preparer provider_caller provider_response_applier; do
  cargo_toml="os-apps/paw-agent/wasm/$crate/Cargo.toml"
  src="os-apps/paw-agent/wasm/$crate/src/lib.rs"

  if rg -n 'llm-caller' "$cargo_toml" >/dev/null; then
    fail "$cargo_toml still depends on llm-caller"
  fi

  if rg -n 'llm_caller::' "$src" >/dev/null; then
    fail "$src still forwards to llm_caller"
  fi
done

if [ -d "os-apps/paw-agent/wasm/llm_caller" ]; then
  fail "os-apps/paw-agent/wasm/llm_caller still exists"
fi

if rg -n 'llm_caller' os-apps/paw-agent/wasm/build.sh >/dev/null; then
  fail "wasm/build.sh still references llm_caller"
fi

if rg -n '"llm_caller"' os-apps/paw-agent/policies/session.cedar >/dev/null; then
  fail "session.cedar still whitelists llm_caller"
fi

printf 'staged_turn_cutover: ok\n'
