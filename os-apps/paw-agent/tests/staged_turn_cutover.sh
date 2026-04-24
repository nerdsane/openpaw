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

if rg -n '^pub fn run_provider_caller|^pub fn run_provider_response_applier' \
  os-apps/paw-agent/wasm/context_preparer/src/lib.rs >/dev/null; then
  fail "context_preparer still defines foreign staged entrypoints"
fi

if rg -n '^pub fn run_context_preparer|^pub fn run_provider_response_applier' \
  os-apps/paw-agent/wasm/provider_caller/src/lib.rs >/dev/null; then
  fail "provider_caller still defines foreign staged entrypoints"
fi

if rg -n '^pub fn run_context_preparer|^pub fn run_provider_caller' \
  os-apps/paw-agent/wasm/provider_response_applier/src/lib.rs >/dev/null; then
  fail "provider_response_applier still defines foreign staged entrypoints"
fi

for src in \
  os-apps/paw-agent/wasm/context_preparer/src/lib.rs \
  os-apps/paw-agent/wasm/provider_caller/src/lib.rs \
  os-apps/paw-agent/wasm/provider_response_applier/src/lib.rs
do
  if rg -n '^pub struct PreparedContextArtifact|^pub struct ProviderResponseArtifact' "$src" >/dev/null; then
    fail "$src still defines shared session-turn artifact structs"
  fi

  if rg -n '^fn build_provider_response_ready_params|^fn build_provider_response_applier_base_params' "$src" >/dev/null; then
    fail "$src still defines shared provider response param builders"
  fi

  if rg -n '^fn build_gen_ai_system_instructions|^fn build_gen_ai_input_messages|^fn build_gen_ai_output_messages' "$src" >/dev/null; then
    fail "$src still defines shared gen_ai payload builders"
  fi

  if rg -n '^const DEFAULT_TOOLS_ENABLED|^struct ReplMethodSpec|^const REPL_METHOD_SPECS|^fn normalize_tool_token|^fn enabled_tool_set|^fn has_sandbox_surface|^fn build_method_listing' "$src" >/dev/null; then
    fail "$src still defines the shared tool catalog locally"
  fi
done

if rg -n '^enum ProviderProgressBoundary|^fn run_with_provider_progress|^struct LlmResponse|^fn call_anthropic|^fn call_openrouter|^fn call_openai|^fn resolve_provider_and_model|^fn read_provider_response_artifact|^fn append_assistant_response_to_session_tree' \
  os-apps/paw-agent/wasm/context_preparer/src/lib.rs >/dev/null; then
  fail "context_preparer still carries provider/applier-only helpers"
fi

if rg -n '^fn load_messages_for_prepare|^fn assemble_cached_system_prompt|^fn estimate_prepared_context_bytes|^fn emit_prepare_duration_metric|^fn model_context_window|^fn assemble_system_prompt|^fn build_sdk_reference|^fn load_soul_content|^fn resolve_soul_entity|^fn normalize_skill_key|^fn skill_name_from_path|^fn xml_escape|^fn parse_skill_frontmatter|^fn strip_skill_frontmatter|^fn load_skills_block|^fn load_agent_instructions|^const PLAN_MODE_FALLBACK|^fn load_mode_instructions|^fn load_active_plan|^fn load_memory_block|^fn resolve_context_refs|^fn read_content_file_raw|^fn append_assistant_response_to_session_tree|^fn create_content_file_for_entry|^fn should_store_entry_as_file|^fn read_provider_response_artifact' \
  os-apps/paw-agent/wasm/provider_caller/src/lib.rs >/dev/null; then
  fail "provider_caller still carries prep/applier-only helpers"
fi

if rg -n '^enum ProviderProgressBoundary|^fn run_with_provider_progress|^struct LlmResponse|^fn call_anthropic|^fn call_openrouter|^fn call_openai|^fn send_heartbeat|^fn send_progress|^fn mock_plan_requests_hang|^fn load_messages_for_prepare|^fn assemble_cached_system_prompt|^fn resolve_provider_and_model|^fn build_tool_definitions|^fn emit_prepare_duration_metric|^fn model_context_window|^fn assemble_system_prompt|^fn build_sdk_reference|^fn load_soul_content|^fn resolve_soul_entity|^fn normalize_skill_key|^fn skill_name_from_path|^fn xml_escape|^fn parse_skill_frontmatter|^fn strip_skill_frontmatter|^fn load_skills_block|^fn load_agent_instructions|^const PLAN_MODE_FALLBACK|^fn load_mode_instructions|^fn load_active_plan|^fn load_memory_block|^fn resolve_context_refs|^fn read_content_file_raw' \
  os-apps/paw-agent/wasm/provider_response_applier/src/lib.rs >/dev/null; then
  fail "provider_response_applier still carries prep/provider-only helpers"
fi

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
