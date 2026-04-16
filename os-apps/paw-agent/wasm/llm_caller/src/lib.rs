//! LLM Caller — WASM module for calling LLM providers (Anthropic/OpenRouter/Mock).
//!
//! Reads conversation from TemperFS File entity (via $value endpoint) when
//! `conversation_file_id` is set, otherwise falls back to inline entity state.
//! Calls the LLM, appends the response, writes back to TemperFS, and returns
//! a dynamic callback action based on the LLM's response:
//! - `ProcessToolCalls` if the response contains tool_use blocks
//! - `RecordResult` if the response is an end_turn
//! - `Fail` if the turn budget is exceeded
//!
//! Supported modes:
//! - Anthropic API key (`x-api-key`)
//! - Anthropic OAuth token (`Authorization: Bearer sk-ant-oat...`)
//! - OpenRouter API key (`Authorization: Bearer`, OpenAI-compatible schema)
//!
//! Build: `cargo build --target wasm32-unknown-unknown --release`

use session_tree_lib::{EntryType, SessionTree};
use std::collections::BTreeSet;
use temper_wasm_sdk::prelude::*;
use wasm_helpers::{
    create_content_file, runtime_headers, runtime_headers_as, send_typing_indicator,
    write_temperfs_value_with_retry,
};

const SESSION_ENTRY_FILE_THRESHOLD_BYTES: usize = 4096;
const DEFAULT_TOOLS_ENABLED: &str = "temper_create,temper_get,temper_list,temper_action,temper_patch,temper_submit_specs,temper_show_spec,temper_specs,temper_upload_wasm,temper_get_trajectories,temper_get_insights,temper_get_decisions,temper_poll_decision,temper_approve_decision,temper_deny_decision,temper_submit_policy,temper_list_policies,temper_get_policy,temper_update_policy,temper_delete_policy,temper_install_app,temper_list_apps,temper_spawn_session,temper_list_sessions,temper_abort_session,temper_steer_session,temper_save_memory,temper_recall_memory,temper_write,temper_read,temper_ls,temper_grep,temper_glob,temper_edit,temper_rename,temper_search_history,temper_run_coding_agent,temper_get_secret,temper_datadog_query,temper_railway,temper_vercel,temper_web_search,temper_web_fetch,read,write,edit,bash";

struct ReplMethodSpec {
    object: &'static str,
    method: &'static str,
    signature: &'static str,
    description: &'static str,
    token: Option<&'static str>,
}

const REPL_METHOD_SPECS: &[ReplMethodSpec] = &[
    ReplMethodSpec {
        object: "sandbox",
        method: "bash",
        signature: "(command)",
        description: "run shell command, returns stdout",
        token: Some("bash"),
    },
    ReplMethodSpec {
        object: "sandbox",
        method: "read",
        signature: "(path)",
        description: "read file content",
        token: Some("read"),
    },
    ReplMethodSpec {
        object: "sandbox",
        method: "write",
        signature: "(path, content)",
        description: "write file",
        token: Some("write"),
    },
    ReplMethodSpec {
        object: "sandbox",
        method: "edit",
        signature: "(path, old, new)",
        description: "search-replace in file",
        token: Some("edit"),
    },
    ReplMethodSpec {
        object: "temper",
        method: "create",
        signature: "(entity_set, fields_dict)",
        description: "create entity, returns dict with entity_id",
        token: Some("temper_create"),
    },
    ReplMethodSpec {
        object: "temper",
        method: "get",
        signature: "(entity_set, entity_id)",
        description: "get entity by id",
        token: Some("temper_get"),
    },
    ReplMethodSpec {
        object: "temper",
        method: "list",
        signature: "(entity_set, filter_str)",
        description: "list entities with OData $filter",
        token: Some("temper_list"),
    },
    ReplMethodSpec {
        object: "temper",
        method: "action",
        signature: "(entity_set, entity_id, action_name, params_dict)",
        description: "dispatch action",
        token: Some("temper_action"),
    },
    ReplMethodSpec {
        object: "temper",
        method: "patch",
        signature: "(entity_set, entity_id, fields_dict)",
        description: "partial update",
        token: Some("temper_patch"),
    },
    ReplMethodSpec {
        object: "temper",
        method: "spawn_session",
        signature: "(task, soul_id=None, model=None, tools=None, workdir=None, sandbox_url=None, max_turns=None, background=False)",
        description: "spawn sub-session",
        token: Some("temper_spawn_session"),
    },
    ReplMethodSpec {
        object: "temper",
        method: "list_sessions",
        signature: "(filter=None, top=50)",
        description: "list recent sessions; add a filter to narrow the results",
        token: Some("temper_list_sessions"),
    },
    ReplMethodSpec {
        object: "temper",
        method: "abort_session",
        signature: "(session_id)",
        description: "cancel session",
        token: Some("temper_abort_session"),
    },
    ReplMethodSpec {
        object: "temper",
        method: "steer_session",
        signature: "(session_id, message)",
        description: "inject message",
        token: Some("temper_steer_session"),
    },
    ReplMethodSpec {
        object: "temper",
        method: "save_memory",
        signature: "(key, content, memory_type='project')",
        description: "persist long-lived memory",
        token: Some("temper_save_memory"),
    },
    ReplMethodSpec {
        object: "temper",
        method: "recall_memory",
        signature: "(query)",
        description: "search persisted memories, returns list",
        token: Some("temper_recall_memory"),
    },
    ReplMethodSpec {
        object: "temper",
        method: "write",
        signature: "(path, content, opts=None)",
        description: "write file by path (auto-creates workspace/dirs), returns {file_id, path, workspace_id}",
        token: Some("temper_write"),
    },
    ReplMethodSpec {
        object: "temper",
        method: "read",
        signature: "(path, opts=None)",
        description: "read file content by path. opts: {offset: int, limit: int} for partial reads (0-indexed line numbers)",
        token: Some("temper_read"),
    },
    ReplMethodSpec {
        object: "temper",
        method: "ls",
        signature: "(path, opts=None)",
        description: "list directory contents (files and subdirectories), returns JSON array",
        token: Some("temper_ls"),
    },
    ReplMethodSpec {
        object: "temper",
        method: "grep",
        signature: "(pattern, path, opts=None)",
        description: "search file contents for pattern, returns matching lines with file paths and line numbers. opts: {case_insensitive: bool, max_results: int}",
        token: Some("temper_grep"),
    },
    ReplMethodSpec {
        object: "temper",
        method: "glob",
        signature: "(pattern, path='/')",
        description: "find files matching name pattern (supports *, **, ?), returns list of matching paths",
        token: Some("temper_glob"),
    },
    ReplMethodSpec {
        object: "temper",
        method: "edit",
        signature: "(path, old_string, new_string, opts=None)",
        description: "replace first occurrence of old_string with new_string in file",
        token: Some("temper_edit"),
    },
    ReplMethodSpec {
        object: "temper",
        method: "rename",
        signature: "(old_path, new_path, opts=None)",
        description: "rename or move a file to a new path",
        token: Some("temper_rename"),
    },
    ReplMethodSpec {
        object: "temper",
        method: "search_history",
        signature: "(pattern)",
        description: "search recent conversation history in this session, including compacted entries",
        token: Some("temper_search_history"),
    },
    ReplMethodSpec {
        object: "temper",
        method: "run_coding_agent",
        signature: "(agent_type, task)",
        description: "spawn coding session",
        token: Some("temper_run_coding_agent"),
    },
    ReplMethodSpec {
        object: "temper",
        method: "submit_specs",
        signature: "(files_dict)",
        description: "load specs into Temper; files_dict must include model.csdl.xml plus one or more *.ioa.toml files (nested paths allowed)",
        token: Some("temper_submit_specs"),
    },
    ReplMethodSpec {
        object: "temper",
        method: "show_spec",
        signature: "(entity_name)",
        description: "inspect entity spec",
        token: Some("temper_show_spec"),
    },
    ReplMethodSpec {
        object: "temper",
        method: "specs",
        signature: "()",
        description: "list available entity specs",
        token: Some("temper_specs"),
    },
    ReplMethodSpec {
        object: "temper",
        method: "install_app",
        signature: "(app_name, reason, payload=None, capability_type='os_app')",
        description: "request capability install",
        token: Some("temper_install_app"),
    },
    ReplMethodSpec {
        object: "temper",
        method: "list_apps",
        signature: "()",
        description: "list available apps",
        token: Some("temper_list_apps"),
    },
    ReplMethodSpec {
        object: "temper",
        method: "upload_wasm",
        signature: "(module_name, wasm_base64)",
        description: "upload WASM module",
        token: Some("temper_upload_wasm"),
    },
    ReplMethodSpec {
        object: "temper",
        method: "get_secret",
        signature: "(key)",
        description: "read secret from vault (Cedar-gated)",
        token: Some("temper_get_secret"),
    },
    ReplMethodSpec {
        object: "temper",
        method: "switch_provider",
        signature: "(model=None, provider=None)",
        description: "change LLM provider/model mid-session (takes effect on next turn)",
        token: None,
    },
    ReplMethodSpec {
        object: "temper",
        method: "switch_mode",
        signature: "({\"mode\": \"plan\"})",
        description: "switch to plan mode (read-only + Plan entities)",
        token: None,
    },
    ReplMethodSpec {
        object: "temper",
        method: "switch_mode",
        signature: "({\"mode\": \"execute\"})",
        description: "switch to execute mode (full tools)",
        token: None,
    },
    ReplMethodSpec {
        object: "temper",
        method: "done",
        signature: "(result)",
        description: "signal session completion with result",
        token: None,
    },
    ReplMethodSpec {
        object: "temper",
        method: "submit_policy",
        signature: "(policy_id, cedar_text)",
        description: "create Cedar policy (Cedar-gated)",
        token: Some("temper_submit_policy"),
    },
    ReplMethodSpec {
        object: "temper",
        method: "list_policies",
        signature: "()",
        description: "list all Cedar policies",
        token: Some("temper_list_policies"),
    },
    ReplMethodSpec {
        object: "temper",
        method: "get_policy",
        signature: "(policy_id)",
        description: "read a specific Cedar policy",
        token: Some("temper_get_policy"),
    },
    ReplMethodSpec {
        object: "temper",
        method: "update_policy",
        signature: "(policy_id, cedar_text)",
        description: "update Cedar policy (Cedar-gated)",
        token: Some("temper_update_policy"),
    },
    ReplMethodSpec {
        object: "temper",
        method: "delete_policy",
        signature: "(policy_id)",
        description: "delete Cedar policy (Cedar-gated)",
        token: Some("temper_delete_policy"),
    },
    ReplMethodSpec {
        object: "temper",
        method: "get_trajectories",
        signature: "(entity_type, include_actions, limit=10)",
        description: "evolution data",
        token: Some("temper_get_trajectories"),
    },
    ReplMethodSpec {
        object: "temper",
        method: "get_insights",
        signature: "()",
        description: "evolution insights",
        token: Some("temper_get_insights"),
    },
    ReplMethodSpec {
        object: "temper",
        method: "get_decisions",
        signature: "()",
        description: "pending governance decisions",
        token: Some("temper_get_decisions"),
    },
    ReplMethodSpec {
        object: "temper",
        method: "poll_decision",
        signature: "(decision_id)",
        description: "wait for decision",
        token: Some("temper_poll_decision"),
    },
    ReplMethodSpec {
        object: "temper",
        method: "approve_decision",
        signature: "(decision_id, scope_dict)",
        description: "approve governance decision (Cedar-gated)",
        token: Some("temper_approve_decision"),
    },
    ReplMethodSpec {
        object: "temper",
        method: "deny_decision",
        signature: "(decision_id)",
        description: "deny governance decision (Cedar-gated)",
        token: Some("temper_deny_decision"),
    },
    ReplMethodSpec {
        object: "temper",
        method: "get_agent_id",
        signature: "()",
        description: "return the current agent/session identifier",
        token: None,
    },
    ReplMethodSpec {
        object: "temper",
        method: "get_session_id",
        signature: "()",
        description: "return the current session identifier",
        token: None,
    },
    ReplMethodSpec {
        object: "temper",
        method: "datadog_query",
        signature: "(query_kind, monitor_id=None, query=None, ...)",
        description: "Datadog API",
        token: Some("temper_datadog_query"),
    },
    ReplMethodSpec {
        object: "temper",
        method: "railway",
        signature: "(action, project_id=None, ...)",
        description: "Railway API",
        token: Some("temper_railway"),
    },
    ReplMethodSpec {
        object: "temper",
        method: "vercel",
        signature: "(action, deployment_id=None, ...)",
        description: "Vercel API",
        token: Some("temper_vercel"),
    },
    ReplMethodSpec {
        object: "temper",
        method: "web_search",
        signature: "(query)",
        description: "search the web via Exa, returns list of {title, url, text}",
        token: Some("temper_web_search"),
    },
    ReplMethodSpec {
        object: "temper",
        method: "web_fetch",
        signature: "(url)",
        description: "fetch a URL, returns text content (HTML tags stripped)",
        token: Some("temper_web_fetch"),
    },
];

fn normalize_tool_token(token: &str) -> &str {
    match token {
        "read_entity" => "temper_get",
        "save_memory" => "temper_save_memory",
        "recall_memory" => "temper_recall_memory",
        "spawn_agent" | "spawn_session" => "temper_spawn_session",
        "temper_file_upload" => "temper_write",
        other => other,
    }
}

fn enabled_tool_set(tools_enabled: &str) -> BTreeSet<String> {
    tools_enabled
        .split(',')
        .map(str::trim)
        .filter(|tool| !tool.is_empty())
        .map(normalize_tool_token)
        .map(ToOwned::to_owned)
        .collect()
}

fn method_is_enabled(spec: &ReplMethodSpec, enabled: &BTreeSet<String>) -> bool {
    match spec.token {
        Some(token) => enabled.contains(token),
        None => true,
    }
}

fn has_sandbox_surface(enabled: &BTreeSet<String>) -> bool {
    enabled.contains("bash")
        || enabled.contains("read")
        || enabled.contains("write")
        || enabled.contains("edit")
        || enabled.contains("temper_run_coding_agent")
}

fn build_method_listing(enabled: &BTreeSet<String>) -> String {
    REPL_METHOD_SPECS
        .iter()
        .filter(|spec| method_is_enabled(spec, enabled))
        .map(|spec| {
            format!(
                "- {}.{}{} -> {}",
                spec.object, spec.method, spec.signature, spec.description
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Entry point — NOT using `temper_module!` because we need dynamic callback actions.
#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        ctx.log("info", "llm_caller: starting");

        // Read entity state
        let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));

        // Check turn budget
        let _turn_count = fields
            .get("turn_count")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        // Read configuration
        let model = fields
            .get("model")
            .and_then(|v| v.as_str())
            .unwrap_or("claude-sonnet-4-6");
        let provider_raw = fields
            .get("provider")
            .and_then(|v| v.as_str())
            .unwrap_or("anthropic");
        let provider = normalize_provider(provider_raw);
        let temperature: f64 = fields
            .get("temperature")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(1.0);
        let tools_enabled = fields
            .get("tools_enabled")
            .and_then(|v| v.as_str())
            .unwrap_or(DEFAULT_TOOLS_ENABLED);
        // `system_prompt` is the Anthropic API system parameter (agent persona/behavior).
        // `user_message` is the actual user task from the Provision action.
        let system_prompt = fields
            .get("system_prompt")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let user_message = fields
            .get("user_message")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let sandbox_url = fields
            .get("sandbox_url")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let workdir = fields
            .get("workdir")
            .and_then(|v| v.as_str())
            .unwrap_or("/workspace");

        // Resolve provider credentials from integration config.
        // If the configured provider's key is unresolved (not set in vault),
        // auto-detect a provider that has a valid key available.
        let (provider, api_key) = if provider == "mock" {
            (provider, String::new())
        } else {
            let key = resolve_provider_api_key(&ctx, &provider)?;
            if is_unresolved_secret_template(&key) {
                // Try other providers
                let alternatives = ["anthropic", "openai_codex", "openai", "openrouter"];
                let mut found = None;
                for alt in &alternatives {
                    if *alt == provider {
                        continue;
                    }
                    if let Ok(alt_key) = resolve_provider_api_key(&ctx, alt) {
                        if !alt_key.is_empty() && !is_unresolved_secret_template(&alt_key) {
                            ctx.log(
                                "warn",
                                &format!(
                                    "llm_caller: provider={provider} has no key, falling back to {alt}"
                                ),
                            );
                            found = Some((alt.to_string(), alt_key));
                            break;
                        }
                    }
                }
                match found {
                    Some((p, k)) => (p, k),
                    None => {
                        return Err(format!(
                            "provider={provider} api key is unresolved secret template: '{key}'. \
set tenant secret and retry"
                        ));
                    }
                }
            } else {
                (provider, key)
            }
        };

        // If provider changed via fallback, remap model to the vault default
        // for the new provider (the original model won't work cross-provider).
        let model = if provider != normalize_provider(provider_raw) {
            // Prefer vault-configured default model, fall back to hardcoded defaults
            let vault_model = ctx.config.get("default_llm_model")
                .filter(|v| !v.is_empty() && !is_unresolved_secret_template(v));
            let new_model = if let Some(m) = vault_model {
                m.clone()
            } else {
                match provider.as_str() {
                    "openai" => "gpt-4.1".to_string(),
                    "openai_codex" => "gpt-5.4".to_string(),
                    "openrouter" => "anthropic/claude-sonnet-4".to_string(),
                    _ => "claude-sonnet-4-6".to_string(),
                }
            };
            ctx.log("warn", &format!(
                "llm_caller: remapping model from {model} to {new_model} for provider={provider}"
            ));
            new_model
        } else {
            model.to_string()
        };

        let anthropic_api_url = ctx
            .config
            .get("anthropic_api_url")
            .cloned()
            .unwrap_or_else(|| "https://api.anthropic.com/v1/messages".to_string());
        let openrouter_api_url = ctx
            .config
            .get("openrouter_api_url")
            .cloned()
            .unwrap_or_else(|| "https://openrouter.ai/api/v1/chat/completions".to_string());
        let openai_api_url = ctx
            .config
            .get("openai_api_url")
            .cloned()
            .unwrap_or_else(|| "https://chatgpt.com/backend-api/codex/responses".to_string());
        let anthropic_auth_mode = ctx
            .config
            .get("anthropic_auth_mode")
            .cloned()
            .unwrap_or_else(|| "auto".to_string());
        let openrouter_site_url = ctx
            .config
            .get("openrouter_site_url")
            .cloned()
            .unwrap_or_default();
        let openrouter_app_name = ctx
            .config
            .get("openrouter_app_name")
            .cloned()
            .unwrap_or_else(|| "temper-agent".to_string());

        if provider != "mock" && api_key.is_empty() {
            return Err(format!(
                "missing API key for provider={provider}. expected secrets: \
anthropic_api_token (or api_key) for anthropic, openrouter_api_key (or api_key) for openrouter"
            ));
        }

        // TemperFS conversation storage
        let conversation_file_id = fields
            .get("conversation_file_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let temper_api_url = resolve_temper_api_url(&ctx, &fields);
        let tenant = &ctx.tenant;

        // Session tree fields (Pi architecture)
        let session_file_id = fields
            .get("session_file_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let session_leaf_id = fields
            .get("session_leaf_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let workspace_id = fields
            .get("workspace_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // Soul and steering fields
        let soul_id = fields.get("soul_id").and_then(|v| v.as_str()).unwrap_or("");
        let max_follow_ups: i64 = fields
            .get("max_follow_ups")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse().ok())
            .unwrap_or(5);
        let reserve_tokens: usize = fields
            .get("reserve_tokens")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse().ok())
            .unwrap_or(20000);

        // Read conversation — from TemperFS if file_id set, else inline state.
        // First turn uses `user_message` (the actual user task from Provision).
        // `system_prompt` is always sent as the Anthropic system parameter, never as a message.
        if user_message.is_empty() {
            return Err("user_message is empty — nothing to send to the LLM".to_string());
        }
        let first_turn_content = user_message;

        // Determine which session storage to use
        let use_session_tree = !session_file_id.is_empty() && !session_leaf_id.is_empty();

        let (mut messages, mut session_tree) = if use_session_tree {
            let session_jsonl =
                read_session_from_temperfs(&ctx, &temper_api_url, tenant, session_file_id)?;
            if session_jsonl.is_empty() {
                // First turn — tree was just created by sandbox_provisioner but empty
                let tree = SessionTree::from_jsonl(&session_jsonl);
                let msgs = vec![json!({ "role": "user", "content": first_turn_content })];
                (msgs, Some(tree))
            } else {
                let tree = SessionTree::from_jsonl(&session_jsonl);
                let context_refs = tree.build_context_refs(session_leaf_id);
                let msgs = if context_refs.is_empty() {
                    vec![json!({ "role": "user", "content": first_turn_content })]
                } else {
                    resolve_context_refs(&ctx, &temper_api_url, tenant, &context_refs)?
                };
                if msgs.is_empty() {
                    (
                        vec![json!({ "role": "user", "content": first_turn_content })],
                        Some(tree),
                    )
                } else {
                    (msgs, Some(tree))
                }
            }
        } else if !conversation_file_id.is_empty() {
            // Legacy flat JSON mode
            let msgs = read_conversation_from_temperfs(
                &ctx,
                &temper_api_url,
                tenant,
                conversation_file_id,
                first_turn_content,
            )?;
            (msgs, None)
        } else {
            // Inline state
            let conversation_json = fields
                .get("conversation")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if conversation_json.is_empty() {
                (
                    vec![json!({ "role": "user", "content": first_turn_content })],
                    None,
                )
            } else {
                (
                    serde_json::from_str(conversation_json).unwrap_or_else(|_| {
                        vec![json!({ "role": "user", "content": first_turn_content })]
                    }),
                    None,
                )
            }
        };
        messages = repair_interrupted_tool_use_messages(&ctx, messages);
        let prune_after_turns: usize = fields
            .get("prune_tool_results_after_turns")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse().ok())
            .unwrap_or(4);
        prune_old_tool_results(&mut messages, prune_after_turns);

        // Build tool definitions based on tools_enabled
        let tools = build_tool_definitions(tools_enabled, sandbox_url, workdir);

        // Check compaction threshold (Pi architecture)
        if use_session_tree {
            if let Some(ref tree) = session_tree {
                let context_tokens = tree.estimate_tokens(session_leaf_id);
                // Model context windows (approximate)
                let context_window: usize = if model.contains("opus") {
                    200000
                } else if model.contains("haiku") {
                    200000
                } else {
                    200000
                }; // sonnet default
                if context_tokens > context_window.saturating_sub(reserve_tokens) {
                    ctx.log("info", &format!(
                        "llm_caller: context_tokens ({}) exceeds threshold ({}), triggering compaction",
                        context_tokens, context_window.saturating_sub(reserve_tokens)
                    ));
                    // Pass through existing hash/file_id from fields (before recomputation)
                    let existing_hash = fields
                        .get("system_prompt_hash")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let existing_file_id = fields
                        .get("system_prompt_file_id")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    set_success_result(
                        "NeedsCompaction",
                        &json!({
                            "context_tokens": context_tokens,
                            "session_leaf_id": session_leaf_id,
                            "system_prompt_hash": existing_hash,
                            "system_prompt_file_id": existing_file_id,
                        }),
                    );
                    return Ok(());
                }
            }
        }

        // System prompt assembly with hash-based caching (Pi architecture):
        // Components: Soul + agent instructions + override + harness + skills + memory
        // If hash matches previous run, re-use cached prompt from TemperFS.
        let agent_id = fields
            .get("agent_id")
            .or_else(|| fields.get("AgentId"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let project_harness_id = fields
            .get("project_harness_id")
            .or_else(|| fields.get("ProjectHarnessId"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let project_id = fields
            .get("project_id")
            .or_else(|| fields.get("ProjectId"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let session_mode = fields
            .get("session_mode")
            .and_then(|v| v.as_str())
            .unwrap_or("execute");
        let active_plan_id = fields
            .get("active_plan_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let new_prompt_hash = compute_system_prompt_hash(
            soul_id,
            agent_id,
            project_harness_id,
            project_id,
            session_mode,
            active_plan_id,
            system_prompt,
        );

        let prev_hash = fields
            .get("system_prompt_hash")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let prev_file_id = fields
            .get("system_prompt_file_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let (assembled_system_prompt, new_prompt_file_id) =
            if !prev_hash.is_empty() && prev_hash == new_prompt_hash && !prev_file_id.is_empty() {
                // Cache hit — read from TemperFS (1 HTTP call instead of many)
                match read_temperfs_file(&ctx, &temper_api_url, tenant, prev_file_id) {
                    Ok(cached) if !cached.is_empty() => {
                        ctx.log("info", "llm_caller: system prompt cache HIT");
                        (cached, prev_file_id.to_string())
                    }
                    _ => {
                        ctx.log(
                            "warn",
                            "llm_caller: system prompt cache file unreadable, rebuilding",
                        );
                        let prompt = assemble_system_prompt(
                            &ctx,
                            &temper_api_url,
                            tenant,
                            soul_id,
                            system_prompt,
                        )?;
                        let file_id = write_system_prompt_cache(
                            &ctx,
                            &temper_api_url,
                            tenant,
                            workspace_id,
                            &prompt,
                        )
                        .unwrap_or_default();
                        (prompt, file_id)
                    }
                }
            } else {
                // Cache miss — full assembly
                ctx.log("info", "llm_caller: system prompt cache MISS, assembling");
                let prompt =
                    assemble_system_prompt(&ctx, &temper_api_url, tenant, soul_id, system_prompt)?;
                let file_id =
                    write_system_prompt_cache(&ctx, &temper_api_url, tenant, workspace_id, &prompt)
                        .unwrap_or_default();
                (prompt, file_id)
            };
        let new_prompt_hash = new_prompt_hash; // rebind for clarity

        emit_progress_ignore(
            &ctx,
            json!({
                "kind": "prompt_assembled",
                "message": "system prompt assembled",
                "system_prompt": assembled_system_prompt,
            }),
        );
        let mock_hang = provider == "mock" && mock_plan_requests_hang(&messages);
        if !mock_hang {
            let _ = send_heartbeat(&ctx, &temper_api_url, tenant);
        }
        emit_progress_ignore(
            &ctx,
            json!({
                "kind": "llm_request_started",
                "message": format!("calling provider={provider} model={model}"),
            }),
        );

        // Send typing indicator to Discord before LLM call.
        // Use the persistent Agent entity ID (from session fields) for ChannelSession lookup.
        let typing_agent_id =
            entity_field_str(&fields, &["agent_id", "AgentId"]).unwrap_or(&ctx.entity_id);
        send_typing_indicator(&ctx, &temper_api_url, tenant, typing_agent_id);

        // Call LLM API
        let response = match provider.as_str() {
            "mock" => call_mock(&ctx, &messages, &assembled_system_prompt, &tools)?,
            "anthropic" => call_anthropic(
                &ctx,
                &api_key,
                &anthropic_api_url,
                &model,
                &assembled_system_prompt,
                &messages,
                &tools,
                &anthropic_auth_mode,
                temperature,
            )?,
            "openrouter" => call_openrouter(
                &ctx,
                &api_key,
                &openrouter_api_url,
                &model,
                &assembled_system_prompt,
                &messages,
                &tools,
                &openrouter_site_url,
                &openrouter_app_name,
                temperature,
            )?,
            "openai" | "openai_codex" => call_openai(
                &ctx,
                &api_key,
                &openai_api_url,
                &model,
                &assembled_system_prompt,
                &messages,
                &tools,
                temperature,
                &provider,
            )?,
            other => return Err(format!("unsupported LLM provider: {other}")),
        };

        ctx.log(
            "info",
            &format!(
                "llm_caller: got response, stop_reason={}",
                response.stop_reason
            ),
        );

        emit_progress_ignore(
            &ctx,
            json!({
                "kind": "llm_response",
                "message": format!("provider returned stop_reason={}", response.stop_reason),
                "stop_reason": response.stop_reason.clone(),
            }),
        );

        // Append assistant response to conversation
        messages.push(json!({
            "role": "assistant",
            "content": response.content,
        }));

        // Write updated conversation to TemperFS (if file_id set) or pass inline
        let updated_conversation = serde_json::to_string(&messages).unwrap_or_default();

        if !conversation_file_id.is_empty() && !use_session_tree {
            write_conversation_to_temperfs(
                &ctx,
                &temper_api_url,
                tenant,
                conversation_file_id,
                &updated_conversation,
            )?;
        }

        // For TemperFS mode, don't pass conversation inline (it's in the File)
        let conv_param = if conversation_file_id.is_empty() {
            Some(updated_conversation.clone())
        } else {
            None
        };

        // Route based on stop_reason
        match response.stop_reason.as_str() {
            "tool_use" => {
                // Extract tool_use blocks
                let tool_calls: Vec<Value> = response
                    .content
                    .as_array()
                    .unwrap_or(&vec![])
                    .iter()
                    .filter(|block| block.get("type").and_then(|v| v.as_str()) == Some("tool_use"))
                    .cloned()
                    .collect();

                // Update session tree if in tree mode
                let new_leaf = if use_session_tree {
                    if let Some(ref mut tree) = session_tree {
                        let parent = session_leaf_id;
                        let content_str =
                            serde_json::to_string(&response.content).unwrap_or_default();
                        let (leaf, _) = if !workspace_id.is_empty()
                            && should_store_entry_as_file(&content_str)
                        {
                            match create_content_file_for_entry(
                                &ctx,
                                &temper_api_url,
                                tenant,
                                workspace_id,
                                &format!("a-{}", tree.len()),
                                &content_str,
                            ) {
                                Ok(content_file_id) => tree.append_assistant_message_file(
                                    parent,
                                    &content_file_id,
                                    response.output_tokens as usize,
                                ),
                                Err(_) => tree.append_assistant_message(
                                    parent,
                                    &response.content,
                                    response.output_tokens as usize,
                                ),
                            }
                        } else {
                            tree.append_assistant_message(
                                parent,
                                &response.content,
                                response.output_tokens as usize,
                            )
                        };
                        let updated_jsonl = tree.to_jsonl();
                        write_session_to_temperfs(
                            &ctx,
                            &temper_api_url,
                            tenant,
                            session_file_id,
                            &updated_jsonl,
                        )?;
                        Some(leaf)
                    } else {
                        None
                    }
                } else {
                    None
                };

                let tool_calls_json = serde_json::to_string(&tool_calls).unwrap_or_default();
                let mut params = json!({
                    "pending_tool_calls": tool_calls_json,
                    "input_tokens": response.input_tokens,
                    "output_tokens": response.output_tokens,
                    // GenAI observability: input/output messages for Datadog LLM Obs
                    "_gen_ai_system_instructions": build_gen_ai_system_instructions(&assembled_system_prompt),
                    "_gen_ai_input_messages": build_gen_ai_input_messages(&messages),
                    "_gen_ai_output_messages": build_gen_ai_output_messages(&response.content, &response.stop_reason),
                    "_gen_ai_provider": provider.as_str(),
                    "_gen_ai_finish_reason": response.stop_reason.clone(),
                    "system_prompt_hash": new_prompt_hash,
                    "system_prompt_file_id": new_prompt_file_id,
                });
                if let Some(leaf) = new_leaf {
                    params["session_leaf_id"] = json!(leaf);
                }
                if let Some(ref conv) = conv_param {
                    params["conversation"] = json!(conv);
                }
                set_success_result("ProcessToolCalls", &params);
            }
            "end_turn" | "stop" => {
                let result_text = response
                    .content
                    .as_array()
                    .unwrap_or(&vec![])
                    .iter()
                    .filter_map(|block| {
                        if block.get("type").and_then(|v| v.as_str()) == Some("text") {
                            block.get("text").and_then(|v| v.as_str()).map(String::from)
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n");

                // Update session tree if in tree mode
                if use_session_tree {
                    if let Some(ref mut tree) = session_tree {
                        let parent = session_leaf_id;
                        let content_str =
                            serde_json::to_string(&response.content).unwrap_or_default();
                        let (new_leaf, _) = if !workspace_id.is_empty()
                            && should_store_entry_as_file(&content_str)
                        {
                            match create_content_file_for_entry(
                                &ctx,
                                &temper_api_url,
                                tenant,
                                workspace_id,
                                &format!("a-{}", tree.len()),
                                &content_str,
                            ) {
                                Ok(content_file_id) => tree.append_assistant_message_file(
                                    parent,
                                    &content_file_id,
                                    response.output_tokens as usize,
                                ),
                                Err(_) => tree.append_assistant_message(
                                    parent,
                                    &response.content,
                                    response.output_tokens as usize,
                                ),
                            }
                        } else {
                            tree.append_assistant_message(
                                parent,
                                &response.content,
                                response.output_tokens as usize,
                            )
                        };
                        let updated_jsonl = tree.to_jsonl();
                        write_session_to_temperfs(
                            &ctx,
                            &temper_api_url,
                            tenant,
                            session_file_id,
                            &updated_jsonl,
                        )?;

                        // GenAI observability messages for this turn
                        let gen_ai_system_instructions =
                            build_gen_ai_system_instructions(&assembled_system_prompt);
                        let gen_ai_input = build_gen_ai_input_messages(&messages);
                        let gen_ai_output =
                            build_gen_ai_output_messages(&response.content, &response.stop_reason);

                        // Route through steering check if follow-ups are enabled
                        if max_follow_ups > 0 {
                            set_success_result(
                                "CheckSteering",
                                &json!({
                                    "result": result_text,
                                    "session_leaf_id": new_leaf,
                                    "input_tokens": response.input_tokens,
                                    "output_tokens": response.output_tokens,
                                    "_gen_ai_system_instructions": gen_ai_system_instructions,
                                    "_gen_ai_input_messages": gen_ai_input,
                                    "_gen_ai_output_messages": gen_ai_output,
                                    "_gen_ai_provider": provider.as_str(),
                                    "_gen_ai_finish_reason": response.stop_reason.clone(),
                                    "system_prompt_hash": new_prompt_hash,
                                    "system_prompt_file_id": new_prompt_file_id,
                                }),
                            );
                        } else {
                            let params = json!({
                                "result": result_text,
                                "session_leaf_id": new_leaf,
                                "input_tokens": response.input_tokens,
                                "output_tokens": response.output_tokens,
                                "_gen_ai_system_instructions": gen_ai_system_instructions,
                                "_gen_ai_input_messages": gen_ai_input,
                                "_gen_ai_output_messages": gen_ai_output,
                                "_gen_ai_provider": provider.as_str(),
                                "_gen_ai_finish_reason": response.stop_reason.clone(),
                                "system_prompt_hash": new_prompt_hash,
                                "system_prompt_file_id": new_prompt_file_id,
                            });
                            set_success_result("RecordResult", &params);
                        }
                    }
                } else {
                    // Legacy mode — direct to RecordResult
                    let mut params = json!({
                        "result": result_text,
                        "input_tokens": response.input_tokens,
                        "output_tokens": response.output_tokens,
                        "_gen_ai_system_instructions": build_gen_ai_system_instructions(&assembled_system_prompt),
                        "_gen_ai_input_messages": build_gen_ai_input_messages(&messages),
                        "_gen_ai_output_messages": build_gen_ai_output_messages(&response.content, &response.stop_reason),
                        "_gen_ai_provider": provider.as_str(),
                        "_gen_ai_finish_reason": response.stop_reason.clone(),
                        "system_prompt_hash": new_prompt_hash,
                        "system_prompt_file_id": new_prompt_file_id,
                    });
                    if let Some(ref conv) = conv_param {
                        params["conversation"] = json!(conv);
                    }
                    set_success_result("RecordResult", &params);
                }
            }
            other => {
                set_success_result(
                    "Fail",
                    &json!({ "error_message": format!("unexpected stop_reason: {other}") }),
                );
            }
        }

        Ok(())
    })();

    if let Err(e) = result {
        set_error_result(&e);
    }
    0
}

/// Parsed LLM response.
struct LlmResponse {
    content: Value,
    stop_reason: String,
    input_tokens: i64,
    output_tokens: i64,
    cache_read_input_tokens: i64,
    cache_creation_input_tokens: i64,
}

/// Max bytes for gen_ai message attributes to avoid bloating spans.
const GEN_AI_MESSAGE_ATTR_LIMIT: usize = 16_384;

fn build_gen_ai_system_instructions(system_prompt: &str) -> String {
    let payload = if system_prompt.is_empty() {
        json!([])
    } else {
        json!([{
            "type": "text",
            "content": truncate_for_gen_ai_attr(system_prompt),
        }])
    };

    serialize_gen_ai_payload(payload, |size| {
        json!([{
            "type": "text",
            "content": format!("[truncated, {size} bytes]"),
        }])
    })
}

/// Build an OTEL GenAI JSON string of the ordered input messages.
fn build_gen_ai_input_messages(messages: &[Value]) -> String {
    let mut gen_ai_msgs = Vec::new();

    for message in messages {
        match message
            .get("role")
            .and_then(Value::as_str)
            .unwrap_or("user")
        {
            "user" => append_user_gen_ai_messages(&mut gen_ai_msgs, message.get("content")),
            "assistant" => {
                if let Some(parts) = build_assistant_parts(message.get("content")) {
                    gen_ai_msgs.push(json!({
                        "role": "assistant",
                        "parts": parts,
                    }));
                }
            }
            "tool" | "tool_result" => {
                if let Some(tool_message) =
                    build_tool_response_message(message.get("tool_use_id"), message.get("content"))
                {
                    gen_ai_msgs.push(tool_message);
                }
            }
            other => {
                if let Some(content) = message.get("content") {
                    let text = truncate_for_gen_ai_attr(&stringify_content(content));
                    if !text.is_empty() {
                        gen_ai_msgs.push(json!({
                            "role": other,
                            "parts": [{"type": "text", "content": text}],
                        }));
                    }
                }
            }
        }
    }

    serialize_gen_ai_payload(json!(gen_ai_msgs), |size| {
        json!([{
            "role": "user",
            "parts": [{"type": "text", "content": format!("[truncated, {size} bytes]")}],
        }])
    })
}

/// Build an OTEL GenAI JSON string of the assistant output messages.
fn build_gen_ai_output_messages(response_content: &Value, finish_reason: &str) -> String {
    let mut parts = Vec::new();
    if let Some(blocks) = response_content.as_array() {
        for block in blocks {
            match block.get("type").and_then(Value::as_str).unwrap_or("") {
                "text" => {
                    if let Some(text) = block.get("text").and_then(Value::as_str) {
                        parts.push(json!({
                            "type": "text",
                            "content": truncate_for_gen_ai_attr(text),
                        }));
                    }
                }
                "tool_use" => {
                    parts.push(json!({
                        "type": "tool_call",
                        "id": block.get("id").and_then(Value::as_str).unwrap_or_default(),
                        "name": block.get("name").and_then(Value::as_str).unwrap_or("unknown"),
                        "arguments": block.get("input").cloned().unwrap_or_else(|| json!({})),
                    }));
                }
                _ => {}
            }
        }
    }

    let payload = if parts.is_empty() {
        json!([])
    } else {
        json!([{
            "role": "assistant",
            "finish_reason": finish_reason,
            "parts": parts,
        }])
    };

    serialize_gen_ai_payload(payload, |size| {
        json!([{
            "role": "assistant",
            "finish_reason": finish_reason,
            "parts": [{"type": "text", "content": format!("[truncated, {size} bytes]")}],
        }])
    })
}

fn append_user_gen_ai_messages(target: &mut Vec<Value>, content: Option<&Value>) {
    let Some(content) = content else {
        return;
    };

    match content {
        Value::String(text) => {
            let text = truncate_for_gen_ai_attr(text);
            if !text.is_empty() {
                target.push(json!({
                    "role": "user",
                    "parts": [{"type": "text", "content": text}],
                }));
            }
        }
        Value::Array(blocks) => {
            let mut user_parts = Vec::new();
            for block in blocks {
                match block.get("type").and_then(Value::as_str).unwrap_or("") {
                    "text" => {
                        if let Some(text) = block.get("text").and_then(Value::as_str) {
                            user_parts.push(json!({
                                "type": "text",
                                "content": truncate_for_gen_ai_attr(text),
                            }));
                        }
                    }
                    "tool_result" => {
                        if let Some(message) = build_tool_response_message(
                            block.get("tool_use_id"),
                            block.get("content"),
                        ) {
                            target.push(message);
                        }
                    }
                    _ => {}
                }
            }

            if !user_parts.is_empty() {
                target.push(json!({
                    "role": "user",
                    "parts": user_parts,
                }));
            }
        }
        other => {
            let text = truncate_for_gen_ai_attr(&stringify_content(other));
            if !text.is_empty() {
                target.push(json!({
                    "role": "user",
                    "parts": [{"type": "text", "content": text}],
                }));
            }
        }
    }
}

fn build_assistant_parts(content: Option<&Value>) -> Option<Vec<Value>> {
    let content = content?;
    let mut parts = Vec::new();

    match content {
        Value::String(text) => {
            let text = truncate_for_gen_ai_attr(text);
            if !text.is_empty() {
                parts.push(json!({"type": "text", "content": text}));
            }
        }
        Value::Array(blocks) => {
            for block in blocks {
                match block.get("type").and_then(Value::as_str).unwrap_or("") {
                    "text" => {
                        if let Some(text) = block.get("text").and_then(Value::as_str) {
                            parts.push(json!({
                                "type": "text",
                                "content": truncate_for_gen_ai_attr(text),
                            }));
                        }
                    }
                    "tool_use" => {
                        parts.push(json!({
                            "type": "tool_call",
                            "id": block.get("id").and_then(Value::as_str).unwrap_or_default(),
                            "name": block.get("name").and_then(Value::as_str).unwrap_or("unknown"),
                            "arguments": block.get("input").cloned().unwrap_or_else(|| json!({})),
                        }));
                    }
                    _ => {}
                }
            }
        }
        other => {
            let text = truncate_for_gen_ai_attr(&stringify_content(other));
            if !text.is_empty() {
                parts.push(json!({"type": "text", "content": text}));
            }
        }
    }

    (!parts.is_empty()).then_some(parts)
}

fn build_tool_response_message(
    tool_use_id: Option<&Value>,
    content: Option<&Value>,
) -> Option<Value> {
    let tool_use_id = tool_use_id.and_then(Value::as_str)?;
    let result = normalize_tool_result_content(content?);

    Some(json!({
        "role": "tool",
        "id": tool_use_id,
        "parts": [{
            "type": "tool_call_response",
            "id": tool_use_id,
            "result": result,
        }],
    }))
}

fn normalize_tool_result_content(content: &Value) -> Value {
    match content {
        Value::String(text) => serde_json::from_str::<Value>(text)
            .unwrap_or_else(|_| json!(truncate_for_gen_ai_attr(text))),
        Value::Array(blocks) => {
            let text = blocks
                .iter()
                .filter_map(|block| block.get("text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join("\n");
            if text.is_empty() {
                json!(truncate_for_gen_ai_attr(&content.to_string()))
            } else {
                json!(truncate_for_gen_ai_attr(&text))
            }
        }
        other => other.clone(),
    }
}

fn serialize_gen_ai_payload(payload: Value, fallback: impl Fn(usize) -> Value) -> String {
    let serialized = serde_json::to_string(&payload).unwrap_or_default();
    if serialized.len() <= GEN_AI_MESSAGE_ATTR_LIMIT {
        serialized
    } else {
        serde_json::to_string(&fallback(serialized.len())).unwrap_or_else(|_| "[]".to_string())
    }
}

fn truncate_for_gen_ai_attr(value: &str) -> String {
    if value.len() <= GEN_AI_MESSAGE_ATTR_LIMIT / 2 {
        return value.to_string();
    }

    let prefix = prefix_at_char_boundary(value, GEN_AI_MESSAGE_ATTR_LIMIT / 4);
    format!("{prefix}... [truncated, {} bytes total]", value.len())
}

fn prefix_at_char_boundary(input: &str, max_bytes: usize) -> &str {
    if input.len() <= max_bytes {
        return input;
    }
    if max_bytes == 0 {
        return "";
    }

    let mut end = 0;
    for (idx, ch) in input.char_indices() {
        let next = idx + ch.len_utf8();
        if next > max_bytes {
            break;
        }
        end = next;
    }
    &input[..end]
}

fn normalize_provider(provider: &str) -> String {
    let norm = provider.trim().to_ascii_lowercase();
    match norm.as_str() {
        "open_router" => "openrouter".to_string(),
        "codex" | "openai-codex" => "openai_codex".to_string(),
        _ => norm,
    }
}

fn is_unresolved_secret_template(value: &str) -> bool {
    value.contains("{secret:")
}

fn first_non_empty(values: &[Option<String>]) -> String {
    for v in values.iter().flatten() {
        if !v.trim().is_empty() {
            return v.trim().to_string();
        }
    }
    String::new()
}

fn resolve_provider_api_key(ctx: &Context, provider: &str) -> Result<String, String> {
    let key = match provider {
        "anthropic" => first_non_empty(&[
            ctx.config.get("anthropic_api_key").cloned(),
            ctx.config.get("anthropic_api_token").cloned(),
            ctx.config.get("api_key").cloned(),
        ]),
        "openai" => first_non_empty(&[
            ctx.config.get("openai_api_key").cloned(),
            ctx.config.get("api_key").cloned(),
        ]),
        "openai_codex" => first_non_empty(&[
            ctx.config.get("openai_codex_token").cloned(),
        ]),
        "openrouter" => first_non_empty(&[
            ctx.config.get("openrouter_api_key").cloned(),
            ctx.config.get("api_key").cloned(),
        ]),
        other => return Err(format!("unsupported LLM provider: {other}")),
    };
    Ok(key)
}

fn call_mock(
    ctx: &Context,
    messages: &[Value],
    assembled_system_prompt: &str,
    _tools: &[Value],
) -> Result<LlmResponse, String> {
    ctx.log("info", "llm_caller: using deterministic mock provider");
    if mock_plan_requests_hang(messages) {
        simulate_mock_hang(ctx)?;
        return Err("mock hang scenario finished without heartbeat".to_string());
    }

    let assistant_turns = messages
        .iter()
        .filter(|message| message.get("role").and_then(Value::as_str) == Some("assistant"))
        .count();

    if let Some(step) =
        extract_mock_plan(messages).and_then(|steps| steps.get(assistant_turns).cloned())
    {
        return build_mock_step_response(messages, assembled_system_prompt, assistant_turns, &step);
    }

    let latest_user = latest_user_text(messages);
    let text = resolve_mock_template(
        latest_user
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("mock provider completed"),
        assembled_system_prompt,
        latest_user.as_deref().unwrap_or(""),
    );
    Ok(mock_text_response(messages, text))
}

// Dead mock-analysis helpers removed (extract_mock_signal_summary, build_mock_analysis,
// collect_existing_dedupe_keys, lookup_string, lookup_u64, lookup_f64, normalize_key,
// humanize_issue_focus). Recoverable from git history if needed.

fn detect_anthropic_oauth_mode(api_key: &str, auth_mode: &str) -> bool {
    match auth_mode.trim().to_ascii_lowercase().as_str() {
        "oauth" | "token" | "bearer" => true,
        "api_key" => false,
        // Auto-detect: Anthropic accepts all token formats via x-api-key header.
        // Only use Bearer if explicitly configured. Default to x-api-key.
        _ => false,
    }
}

/// Call Anthropic Messages API.
fn call_anthropic(
    ctx: &Context,
    api_key: &str,
    api_url: &str,
    model: &str,
    system_prompt: &str,
    messages: &[Value],
    tools: &[Value],
    anthropic_auth_mode: &str,
    temperature: f64,
) -> Result<LlmResponse, String> {
    // Detect OAuth token (sk-ant-oat-*) vs standard API key
    let is_oauth = detect_anthropic_oauth_mode(api_key, anthropic_auth_mode);

    // OAuth tokens enforce a fixed system prompt when tools are present.
    // Custom system instructions are prepended to the first user message instead.
    let (effective_system, effective_messages) = if is_oauth {
        let oauth_system = "You are Claude Code, Anthropic's official CLI for Claude.".to_string();
        let mut msgs = messages.to_vec();
        if !system_prompt.is_empty() {
            if let Some(first) = msgs.first_mut() {
                if let Some(content) = first.get("content").and_then(|v| v.as_str()) {
                    let combined = format!("[System instructions: {system_prompt}]\n\n{content}");
                    first["content"] = json!(combined);
                }
            }
        }
        (oauth_system, msgs)
    } else {
        (system_prompt.to_string(), messages.to_vec())
    };

    let mut body = json!({
        "model": model,
        "max_tokens": 16384,
        "messages": effective_messages,
        "temperature": temperature,
    });

    if !effective_system.is_empty() {
        body["system"] = json!([{
            "type": "text",
            "text": effective_system,
            "cache_control": {"type": "ephemeral"}
        }]);
    }

    // Add cache_control breakpoints to up to 2 recent user messages
    if let Some(msgs_arr) = body.get_mut("messages").and_then(|v| v.as_array_mut()) {
        let mut user_indices: Vec<usize> = Vec::new();
        for (i, m) in msgs_arr.iter().enumerate() {
            if m.get("role").and_then(|v| v.as_str()) == Some("user") {
                user_indices.push(i);
            }
        }
        // Take last 2 user message indices
        let breakpoints: Vec<usize> = user_indices.into_iter().rev().take(2).collect();
        for idx in breakpoints {
            let msg = &mut msgs_arr[idx];
            if let Some(content) = msg.get_mut("content") {
                if let Some(arr) = content.as_array_mut() {
                    // Array content — add cache_control to last block
                    if let Some(last) = arr.last_mut() {
                        last["cache_control"] = json!({"type": "ephemeral"});
                    }
                } else if let Some(text) = content.as_str().map(|s| s.to_string()) {
                    // String content — convert to array format with cache_control
                    msg["content"] = json!([{
                        "type": "text",
                        "text": text,
                        "cache_control": {"type": "ephemeral"}
                    }]);
                }
            }
        }
    }

    if !tools.is_empty() {
        body["tools"] = json!(tools);
    }

    let body_str =
        serde_json::to_string(&body).map_err(|e| format!("JSON serialize error: {e}"))?;

    ctx.log(
        "info",
        &format!(
            "llm_caller: calling Anthropic API, model={model}, oauth={is_oauth}, messages={}, url={api_url}",
            messages.len(),
        ),
    );

    // Build auth headers — OAuth tokens use Bearer + beta header
    let headers = if is_oauth {
        vec![
            ("authorization".to_string(), format!("Bearer {api_key}")),
            ("anthropic-version".to_string(), "2023-06-01".to_string()),
            (
                "anthropic-beta".to_string(),
                "oauth-2025-04-20,computer-use-2025-01-24,prompt-caching-2024-07-31".to_string(),
            ),
            ("content-type".to_string(), "application/json".to_string()),
            ("user-agent".to_string(), "claude-cli/2.1.75".to_string()),
            ("x-app".to_string(), "cli".to_string()),
        ]
    } else {
        vec![
            ("x-api-key".to_string(), api_key.to_string()),
            ("anthropic-version".to_string(), "2023-06-01".to_string()),
            (
                "anthropic-beta".to_string(),
                "prompt-caching-2024-07-31".to_string(),
            ),
            ("content-type".to_string(), "application/json".to_string()),
        ]
    };

    // Retry on transient API errors (500, 529, and 400 with vague "Error" message)
    let mut last_err = String::new();
    let mut resp = None;
    for attempt in 0..5 {
        if attempt > 0 {
            ctx.log(
                "warn",
                &format!(
                    "llm_caller: retrying (attempt {}/5), last error: {last_err}",
                    attempt + 1
                ),
            );
        }
        match ctx.http_call("POST", api_url, &headers, &body_str) {
            Ok(r) if r.status == 200 => {
                resp = Some(r);
                break;
            }
            Ok(r) if r.status == 500 || r.status == 529 => {
                last_err = format!("HTTP {}: {}", r.status, &r.body[..r.body.len().min(200)]);
                continue;
            }
            Ok(r) if r.status == 400 && r.body.contains("\"message\":\"Error\"") => {
                // Transient 400 with vague error message — retry
                last_err = format!("HTTP 400 (transient): {}", &r.body[..r.body.len().min(200)]);
                continue;
            }
            Ok(r) => {
                return Err(format!(
                    "Anthropic API returned {}: {}",
                    r.status,
                    &r.body[..r.body.len().min(500)]
                ));
            }
            Err(e) => {
                last_err = e;
                continue;
            }
        }
    }
    let resp = resp.ok_or_else(|| format!("Anthropic API failed after 5 attempts: {last_err}"))?;

    let parsed: Value = serde_json::from_str(&resp.body)
        .map_err(|e| format!("failed to parse LLM response: {e}"))?;

    let stop_reason = parsed
        .get("stop_reason")
        .and_then(|v| v.as_str())
        .unwrap_or("end_turn")
        .to_string();

    let content = parsed.get("content").cloned().unwrap_or(json!([]));

    // Extract token usage from Anthropic response
    let usage = parsed.get("usage").cloned().unwrap_or(json!({}));
    let input_tokens = usage
        .get("input_tokens")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let output_tokens = usage
        .get("output_tokens")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let cache_read_input_tokens = usage
        .get("cache_read_input_tokens")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let cache_creation_input_tokens = usage
        .get("cache_creation_input_tokens")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);

    ctx.log(
        "info",
        &format!("llm_caller: usage: input={input_tokens}, output={output_tokens}, cache_read={cache_read_input_tokens}, cache_create={cache_creation_input_tokens}"),
    );

    Ok(LlmResponse {
        content,
        stop_reason,
        input_tokens,
        output_tokens,
        cache_read_input_tokens,
        cache_creation_input_tokens,
    })
}

/// Call OpenRouter Chat Completions API (OpenAI-compatible schema).
fn call_openrouter(
    ctx: &Context,
    api_key: &str,
    api_url: &str,
    model: &str,
    system_prompt: &str,
    messages: &[Value],
    tools: &[Value],
    site_url: &str,
    app_name: &str,
    temperature: f64,
) -> Result<LlmResponse, String> {
    let mut or_messages = Vec::<Value>::new();
    if !system_prompt.is_empty() {
        or_messages.push(json!({
            "role": "system",
            "content": system_prompt,
        }));
    }
    or_messages.extend(convert_messages_to_openrouter(messages));

    let openai_tools = convert_tools_to_openrouter(tools);
    let mut body = json!({
        "model": model,
        "messages": or_messages,
        "max_tokens": 16384,
        "temperature": temperature,
    });
    if !openai_tools.is_empty() {
        body["tools"] = json!(openai_tools);
        body["tool_choice"] = json!("auto");
    }

    let body_str =
        serde_json::to_string(&body).map_err(|e| format!("JSON serialize error: {e}"))?;

    let mut headers = vec![
        ("authorization".to_string(), format!("Bearer {api_key}")),
        ("content-type".to_string(), "application/json".to_string()),
    ];
    if !site_url.trim().is_empty() {
        headers.push(("HTTP-Referer".to_string(), site_url.trim().to_string()));
    }
    if !app_name.trim().is_empty() {
        headers.push(("X-Title".to_string(), app_name.trim().to_string()));
    }

    ctx.log(
        "info",
        &format!(
            "llm_caller: calling OpenRouter API, model={model}, messages={}, url={api_url}",
            messages.len(),
        ),
    );

    let mut last_err = String::new();
    let mut resp = None;
    for attempt in 0..5 {
        if attempt > 0 {
            ctx.log(
                "warn",
                &format!(
                    "llm_caller: openrouter retry (attempt {}/5), last error: {last_err}",
                    attempt + 1
                ),
            );
        }
        match ctx.http_call("POST", api_url, &headers, &body_str) {
            Ok(r) if r.status == 200 => {
                resp = Some(r);
                break;
            }
            Ok(r) if matches!(r.status, 429 | 500 | 502 | 503 | 504) => {
                last_err = format!("HTTP {}: {}", r.status, &r.body[..r.body.len().min(200)]);
                continue;
            }
            Ok(r) => {
                return Err(format!(
                    "OpenRouter API returned {}: {}",
                    r.status,
                    &r.body[..r.body.len().min(500)]
                ));
            }
            Err(e) => {
                last_err = e;
                continue;
            }
        }
    }
    let resp = resp.ok_or_else(|| format!("OpenRouter API failed after 5 attempts: {last_err}"))?;

    let parsed: Value = serde_json::from_str(&resp.body)
        .map_err(|e| format!("failed to parse OpenRouter response: {e}"))?;
    let choice = parsed
        .get("choices")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .cloned()
        .unwrap_or(json!({}));
    let message = choice.get("message").cloned().unwrap_or(json!({}));

    let mut content_blocks = Vec::<Value>::new();
    let text = extract_openrouter_text(&message);
    if !text.is_empty() {
        content_blocks.push(json!({
            "type": "text",
            "text": text,
        }));
    }

    let mut has_tool_calls = false;
    if let Some(tool_calls) = message.get("tool_calls").and_then(Value::as_array) {
        for (idx, tc) in tool_calls.iter().enumerate() {
            let fn_name = tc
                .get("function")
                .and_then(|f| f.get("name"))
                .and_then(Value::as_str)
                .unwrap_or("unknown_tool");
            let call_id = tc
                .get("id")
                .and_then(Value::as_str)
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("or_tool_{}", idx + 1));
            let args_str = tc
                .get("function")
                .and_then(|f| f.get("arguments"))
                .and_then(Value::as_str)
                .unwrap_or("{}");
            let input = serde_json::from_str::<Value>(args_str).unwrap_or(json!({}));

            content_blocks.push(json!({
                "type": "tool_use",
                "id": call_id,
                "name": fn_name,
                "input": input,
            }));
            has_tool_calls = true;
        }
    }

    let usage = parsed.get("usage").cloned().unwrap_or(json!({}));
    let input_tokens = usage
        .get("prompt_tokens")
        .and_then(|v| v.as_i64())
        .or_else(|| usage.get("input_tokens").and_then(|v| v.as_i64()))
        .unwrap_or(0);
    let output_tokens = usage
        .get("completion_tokens")
        .and_then(|v| v.as_i64())
        .or_else(|| usage.get("output_tokens").and_then(|v| v.as_i64()))
        .unwrap_or(0);

    let stop_reason = if has_tool_calls {
        "tool_use".to_string()
    } else {
        "end_turn".to_string()
    };

    Ok(LlmResponse {
        content: Value::Array(content_blocks),
        stop_reason,
        input_tokens,
        output_tokens,
        cache_read_input_tokens: 0,
        cache_creation_input_tokens: 0,
    })
}

/// Call OpenAI Codex Responses API (chatgpt.com/backend-api/codex/responses).
///
/// Uses the Responses API format (not Chat Completions): instructions, input, stream=true.
/// The WASM http_call buffers the full SSE stream — we parse the response.completed event.
fn call_openai(
    ctx: &Context,
    api_key: &str,
    api_url: &str,
    model: &str,
    system_prompt: &str,
    messages: &[Value],
    tools: &[Value],
    temperature: f64,
    provider: &str,
) -> Result<LlmResponse, String> {
    // Convert Anthropic-format messages to Responses API input format
    let pre_convert_types: Vec<String> = messages
        .iter()
        .map(|m| {
            let role = m.get("role").and_then(Value::as_str).unwrap_or("?");
            let ct = if m.get("content").and_then(Value::as_str).is_some() {
                "str".to_string()
            } else if let Some(arr) = m.get("content").and_then(Value::as_array) {
                let block_types: Vec<&str> = arr
                    .iter()
                    .filter_map(|b| b.get("type").and_then(Value::as_str))
                    .collect();
                format!("arr[{}]", block_types.join(","))
            } else {
                "?".to_string()
            };
            format!("{}:{}", role, ct)
        })
        .collect();
    ctx.log(
        "info",
        &format!(
            "llm_caller: openai pre-convert messages={} types={:?}",
            messages.len(),
            pre_convert_types
        ),
    );

    let mut input = Vec::<Value>::new();
    for msg in messages {
        let role = msg.get("role").and_then(Value::as_str).unwrap_or("user");
        match role {
            "user" => {
                if let Some(content) = msg.get("content").and_then(Value::as_str) {
                    input.push(json!({"role": "user", "content": content}));
                } else if let Some(blocks) = msg.get("content").and_then(Value::as_array) {
                    // Handle array content blocks — may contain text AND tool_result blocks
                    let mut has_tool_results = false;
                    for block in blocks {
                        let block_type = block.get("type").and_then(Value::as_str).unwrap_or("");
                        if block_type == "tool_result" {
                            // Anthropic tool_result → Responses API function_call_output
                            let call_id = block
                                .get("tool_use_id")
                                .and_then(Value::as_str)
                                .unwrap_or("");
                            let output = if let Some(inner) =
                                block.get("content").and_then(Value::as_array)
                            {
                                inner
                                    .iter()
                                    .filter_map(|b| b.get("text").and_then(Value::as_str))
                                    .collect::<Vec<_>>()
                                    .join("\n")
                            } else if let Some(text) = block.get("content").and_then(Value::as_str)
                            {
                                text.to_string()
                            } else {
                                String::new()
                            };
                            input.push(json!({
                                "type": "function_call_output",
                                "call_id": call_id,
                                "output": output
                            }));
                            has_tool_results = true;
                        }
                    }
                    // Also extract any text blocks (non-tool-result content)
                    if !has_tool_results {
                        let text: String = blocks
                            .iter()
                            .filter_map(|b| b.get("text").and_then(Value::as_str))
                            .collect::<Vec<_>>()
                            .join("\n");
                        if !text.is_empty() {
                            input.push(json!({"role": "user", "content": text}));
                        }
                    }
                }
            }
            "assistant" => {
                if let Some(blocks) = msg.get("content").and_then(Value::as_array) {
                    for block in blocks {
                        let block_type = block.get("type").and_then(Value::as_str).unwrap_or("");
                        match block_type {
                            "text" => {
                                if let Some(text) = block.get("text").and_then(Value::as_str) {
                                    input.push(json!({
                                        "type": "message",
                                        "role": "assistant",
                                        "content": [{"type": "output_text", "text": text}]
                                    }));
                                }
                            }
                            "tool_use" => {
                                let call_id = block
                                    .get("id")
                                    .and_then(Value::as_str)
                                    .unwrap_or("")
                                    .to_string();
                                let name = block
                                    .get("name")
                                    .and_then(Value::as_str)
                                    .unwrap_or("")
                                    .to_string();
                                let arguments =
                                    serde_json::to_string(block.get("input").unwrap_or(&json!({})))
                                        .unwrap_or_else(|_| "{}".to_string());
                                input.push(json!({
                                    "type": "function_call",
                                    "call_id": call_id,
                                    "name": name,
                                    "arguments": arguments
                                }));
                            }
                            _ => {}
                        }
                    }
                }
            }
            "tool_result" => {
                // Anthropic tool_result → Responses API function_call_output
                let tool_use_id = msg.get("tool_use_id").and_then(Value::as_str).unwrap_or("");
                let content = if let Some(blocks) = msg.get("content").and_then(Value::as_array) {
                    blocks
                        .iter()
                        .filter_map(|b| b.get("text").and_then(Value::as_str))
                        .collect::<Vec<_>>()
                        .join("\n")
                } else if let Some(text) = msg.get("content").and_then(Value::as_str) {
                    text.to_string()
                } else {
                    String::new()
                };
                input.push(json!({
                    "type": "function_call_output",
                    "call_id": tool_use_id,
                    "output": content
                }));
            }
            _ => {}
        }
    }

    // Convert tools to Responses API format
    let codex_tools: Vec<Value> = tools
        .iter()
        .map(|t| {
            let name = t.get("name").and_then(Value::as_str).unwrap_or("");
            let desc = t.get("description").and_then(Value::as_str).unwrap_or("");
            let schema = t.get("input_schema").cloned().unwrap_or(json!({}));
            json!({
                "type": "function",
                "name": name,
                "description": desc,
                "parameters": schema,
                "strict": false
            })
        })
        .collect();

    let mut body = json!({
        "model": model,
        "instructions": system_prompt,
        "input": input,
        "stream": true,
        "store": false,
        "reasoning": {
            "effort": "medium",
            "summary": "auto",
        },
    });
    // Codex API does not support the temperature parameter
    if provider != "openai_codex" {
        body["temperature"] = json!(temperature);
    }
    if !codex_tools.is_empty() {
        body["tools"] = json!(codex_tools);
        // "auto" lets the model choose text or tool calls. "required" forces
        // a tool call every turn, which creates an infinite loop when the model
        // wants to respond with text (e.g., "hello").
        body["tool_choice"] = json!("auto");
    }

    let body_str =
        serde_json::to_string(&body).map_err(|e| format!("JSON serialize error: {e}"))?;

    let headers = vec![
        ("authorization".to_string(), format!("Bearer {api_key}")),
        ("content-type".to_string(), "application/json".to_string()),
    ];

    // Log input types for debugging conversation format issues
    let input_types: Vec<String> = input
        .iter()
        .map(|i| {
            let t = i
                .get("type")
                .and_then(Value::as_str)
                .or_else(|| i.get("role").and_then(Value::as_str))
                .unwrap_or("?");
            t.to_string()
        })
        .collect();
    // Log top-level body keys (not full body — system prompt can be huge)
    let body_keys: Vec<&str> = body
        .as_object()
        .map(|m| m.keys().map(|k| k.as_str()).collect())
        .unwrap_or_default();
    ctx.log(
        "info",
        &format!(
            "llm_caller: calling OpenAI API, model={model}, input={}, types={:?}, url={api_url}, body_keys={body_keys:?}",
            input.len(),
            input_types,
        ),
    );

    let mut last_err = String::new();
    let mut output_items = Vec::<Value>::new();
    let mut usage = json!({});

    for attempt in 0..5 {
        if attempt > 0 {
            ctx.log(
                "warn",
                &format!("llm_caller: OpenAI Codex retry {}/{}", attempt + 1, 5),
            );
        }
        let resp = match ctx.http_call("POST", api_url, &headers, &body_str) {
            Ok(r) if r.status >= 200 && r.status < 300 => r,
            Ok(r) if r.status == 429 => {
                last_err = format!("OpenAI Codex API rate limited (429)");
                continue;
            }
            Ok(r) => {
                let snippet = &r.body[..r.body.len().min(300)];
                ctx.log(
                    "error",
                    &format!(
                        "llm_caller: OpenAI Codex API error status={} body={snippet}",
                        r.status
                    ),
                );
                return Err(format!("OpenAI Codex API returned {}: {snippet}", r.status));
            }
            Err(e) => {
                last_err = e;
                continue;
            }
        };

        // Parse SSE data payloads (newline-separated JSON lines from host).
        // The Codex endpoint streams individual events — output_item.done events
        // contain the actual tool calls and messages. response.completed may have
        // empty output (Codex strips it for bandwidth). So we accumulate output
        // items from output_item.done events and usage from response.completed.
        let body = &resp.body;
        output_items.clear();
        usage = json!({});
        let mut streamed_text = String::new();
        let mut saw_completed = false;

        for line in body.lines() {
            let line = line.trim();
            if line.is_empty() || line == "[DONE]" {
                continue;
            }
            let json_str = line.strip_prefix("data: ").unwrap_or(line);
            if let Ok(event) = serde_json::from_str::<Value>(json_str) {
                let event_type = event.get("type").and_then(Value::as_str).unwrap_or("");
                match event_type {
                    "response.output_item.done" => {
                        if let Some(item) = event.get("item") {
                            output_items.push(item.clone());
                        }
                    }
                    "response.output_text.delta" => {
                        if let Some(delta) = event.get("delta").and_then(Value::as_str) {
                            streamed_text.push_str(delta);
                        } else if let Some(text) = event.get("text").and_then(Value::as_str) {
                            streamed_text.push_str(text);
                        }
                    }
                    "response.output_text.done" => {
                        if let Some(text) = event.get("text").and_then(Value::as_str) {
                            if streamed_text.is_empty() {
                                streamed_text.push_str(text);
                            }
                        }
                    }
                    "response.completed" => {
                        saw_completed = true;
                        if let Some(resp) = event.get("response") {
                            if let Some(u) = resp.get("usage") {
                                usage = u.clone();
                            }
                            if let Some(out) = resp.get("output").and_then(Value::as_array) {
                                if !out.is_empty() {
                                    output_items = out.clone();
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        if output_items.is_empty() {
            let trimmed = streamed_text.trim();
            if !trimmed.is_empty() {
                output_items.push(json!({
                    "type": "message",
                    "content": [{
                        "type": "output_text",
                        "text": trimmed,
                    }],
                }));
            }
        }

        if !output_items.is_empty() {
            break;
        }

        // No output items — stream was likely truncated (SSE decode error
        // during reasoning phase). Retry if we never saw response.completed.
        if !saw_completed {
            last_err = format!(
                "SSE stream truncated: {} lines ({}B) but no response.completed event",
                body.lines().count(),
                body.len()
            );
            ctx.log(
                "warn",
                &format!("llm_caller: {last_err}, will retry"),
            );
            continue;
        }

        // Saw response.completed but still no output — genuine empty response
        return Err(format!(
            "OpenAI: no output items found in {} lines ({}B) despite response.completed",
            body.lines().count(),
            body.len()
        ));
    }

    if output_items.is_empty() {
        return Err(format!(
            "OpenAI Codex API failed after 5 attempts: {last_err}"
        ));
    }

    // Build a synthetic response object for the existing parsing code
    let response = json!({
        "output": output_items,
        "usage": usage,
    });

    // Extract content and tool calls from response.output
    let mut content_blocks = Vec::<Value>::new();
    let mut has_tool_calls = false;

    if let Some(output) = response.get("output").and_then(Value::as_array) {
        for item in output {
            let item_type = item.get("type").and_then(Value::as_str).unwrap_or("");
            match item_type {
                "message" => {
                    if let Some(content) = item.get("content").and_then(Value::as_array) {
                        for part in content {
                            let part_type = part.get("type").and_then(Value::as_str).unwrap_or("");
                            if part_type == "output_text" {
                                if let Some(text) = part.get("text").and_then(Value::as_str) {
                                    if !text.is_empty() {
                                        content_blocks.push(json!({
                                            "type": "text",
                                            "text": text,
                                        }));
                                    }
                                }
                            }
                        }
                    }
                }
                "function_call" => {
                    let call_id = item
                        .get("call_id")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string();
                    let name = item
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string();
                    let arguments = item
                        .get("arguments")
                        .and_then(Value::as_str)
                        .unwrap_or("{}");
                    let input = serde_json::from_str::<Value>(arguments).unwrap_or(json!({}));
                    content_blocks.push(json!({
                        "type": "tool_use",
                        "id": call_id,
                        "name": name,
                        "input": input,
                    }));
                    has_tool_calls = true;
                }
                _ => {} // reasoning, etc. — skip
            }
        }
    }

    let usage = response.get("usage").cloned().unwrap_or(json!({}));
    let input_tokens = usage
        .get("input_tokens")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    let output_tokens = usage
        .get("output_tokens")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);

    let stop_reason = if has_tool_calls {
        "tool_use".to_string()
    } else {
        "end_turn".to_string()
    };

    ctx.log(
        "info",
        &format!("llm_caller: OpenAI Codex response: blocks={}, stop={stop_reason}, in={input_tokens}, out={output_tokens}",
            content_blocks.len()),
    );

    Ok(LlmResponse {
        content: Value::Array(content_blocks),
        stop_reason,
        input_tokens,
        output_tokens,
        cache_read_input_tokens: 0,
        cache_creation_input_tokens: 0,
    })
}

fn extract_openrouter_text(message: &Value) -> String {
    if let Some(text) = message.get("content").and_then(Value::as_str) {
        return text.to_string();
    }
    if let Some(arr) = message.get("content").and_then(Value::as_array) {
        let mut chunks = Vec::<String>::new();
        for item in arr {
            if let Some(text) = item.get("text").and_then(Value::as_str) {
                chunks.push(text.to_string());
            } else if let Some(text) = item.get("content").and_then(Value::as_str) {
                chunks.push(text.to_string());
            }
        }
        return chunks.join("\n");
    }
    String::new()
}

fn stringify_content(value: &Value) -> String {
    if let Some(s) = value.as_str() {
        s.to_string()
    } else {
        value.to_string()
    }
}

fn emit_progress_ignore(ctx: &Context, payload: Value) {
    let _ = (ctx, payload);
}

fn agent_headers(
    ctx: &Context,
    tenant: &str,
    content_type: Option<&str>,
    accept: Option<&str>,
) -> Vec<(String, String)> {
    let fields = ctx
        .entity_state
        .get("fields")
        .cloned()
        .unwrap_or_else(|| json!({}));
    runtime_headers(ctx, tenant, &fields, content_type, accept)
}

fn send_heartbeat(ctx: &Context, temper_api_url: &str, tenant: &str) -> Result<(), String> {
    let url = format!(
        "{temper_api_url}/tdata/Sessions('{}')/OpenPaw.Heartbeat",
        ctx.entity_id
    );
    let body = json!({ "last_heartbeat_at": "alive" });
    let fields = ctx
        .entity_state
        .get("fields")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let headers = runtime_headers_as(
        ctx,
        tenant,
        &fields,
        "system",
        Some("application/json"),
        None,
    );
    let _ = ctx.http_call("POST", &url, &headers, &body.to_string())?;
    Ok(())
}

fn mock_plan_requests_hang(messages: &[Value]) -> bool {
    if let Some(steps) = extract_mock_plan(messages)
        && steps
            .iter()
            .any(|step| step.get("mode").and_then(Value::as_str) == Some("hang"))
    {
        return true;
    }
    latest_user_text(messages)
        .map(|text| text.contains("[mock-hang]"))
        .unwrap_or(false)
}

fn simulate_mock_hang(ctx: &Context) -> Result<(), String> {
    let fields = ctx
        .entity_state
        .get("fields")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let base_url = resolve_temper_api_url(ctx, &fields);
    let url = format!(
        "{base_url}/observe/entities/{}/{}/wait?statuses=__never__&timeout_ms=10000&poll_ms=250",
        ctx.entity_type, ctx.entity_id
    );
    let headers = agent_headers(ctx, &ctx.tenant, None, Some("application/json"));
    let _ = ctx.http_call("GET", &url, &headers, "")?;
    Ok(())
}

fn extract_mock_plan(messages: &[Value]) -> Option<Vec<Value>> {
    for message in messages {
        if message.get("role").and_then(Value::as_str) != Some("user") {
            continue;
        }
        let raw = stringify_content(message.get("content").unwrap_or(&Value::Null));
        let Ok(parsed) = serde_json::from_str::<Value>(&raw) else {
            continue;
        };
        if let Some(steps) = parsed.get("steps").and_then(Value::as_array) {
            return Some(steps.clone());
        }
        if let Some(steps) = parsed
            .get("mock_plan")
            .and_then(|value| value.get("steps"))
            .and_then(Value::as_array)
        {
            return Some(steps.clone());
        }
    }
    None
}

fn build_mock_step_response(
    messages: &[Value],
    assembled_system_prompt: &str,
    assistant_turns: usize,
    step: &Value,
) -> Result<LlmResponse, String> {
    if step.get("mode").and_then(Value::as_str) == Some("hang") {
        return Ok(mock_text_response(
            messages,
            "mock hang placeholder".to_string(),
        ));
    }

    let mut content = Vec::<Value>::new();
    if let Some(text) = step.get("text").and_then(Value::as_str) {
        let resolved = resolve_mock_template(
            text,
            assembled_system_prompt,
            latest_user_text(messages).as_deref().unwrap_or(""),
        );
        if !resolved.is_empty() {
            content.push(json!({ "type": "text", "text": resolved }));
        }
    }

    if let Some(tool_calls) = step.get("tool_calls").and_then(Value::as_array) {
        for (index, tool_call) in tool_calls.iter().enumerate() {
            let name = tool_call
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("unknown_tool");
            let input = tool_call.get("input").cloned().unwrap_or_else(|| json!({}));
            let id = tool_call
                .get("id")
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| format!("mock-tool-{assistant_turns}-{index}"));
            content.push(json!({
                "type": "tool_use",
                "id": id,
                "name": name,
                "input": input,
            }));
        }
    }

    if content
        .iter()
        .any(|block| block.get("type").and_then(Value::as_str) == Some("tool_use"))
    {
        let output_len = serde_json::to_string(&content).unwrap_or_default().len() as i64;
        return Ok(LlmResponse {
            content: Value::Array(content),
            stop_reason: "tool_use".to_string(),
            input_tokens: estimate_message_tokens(messages),
            output_tokens: output_len,
            cache_read_input_tokens: 0,
            cache_creation_input_tokens: 0,
        });
    }

    let final_text = step
        .get("final_text")
        .or_else(|| step.get("text"))
        .and_then(Value::as_str)
        .unwrap_or("mock provider completed");
    Ok(mock_text_response(
        messages,
        resolve_mock_template(
            final_text,
            assembled_system_prompt,
            latest_user_text(messages).as_deref().unwrap_or(""),
        ),
    ))
}

fn repair_interrupted_tool_use_messages(ctx: &Context, messages: Vec<Value>) -> Vec<Value> {
    let mut repaired = Vec::new();
    let mut repair_count = 0u32;

    for (idx, message) in messages.iter().enumerate() {
        repaired.push(message.clone());

        if message.get("role").and_then(Value::as_str) != Some("assistant") {
            continue;
        }

        let pending_ids = extract_tool_use_ids(message);
        if pending_ids.is_empty() {
            continue;
        }

        let next_tool_results = messages
            .get(idx + 1)
            .filter(|next| next.get("role").and_then(Value::as_str) == Some("user"))
            .map(extract_tool_result_ids)
            .unwrap_or_default();

        let missing_ids = pending_ids
            .into_iter()
            .filter(|tool_use_id| !next_tool_results.contains(tool_use_id))
            .collect::<Vec<_>>();
        if missing_ids.is_empty() {
            continue;
        }

        repair_count += missing_ids.len() as u32;
        repaired.push(json!({
            "role": "user",
            "content": missing_ids
                .into_iter()
                .map(|tool_use_id| json!({
                    "type": "tool_result",
                    "tool_use_id": tool_use_id,
                    "content": "Tool execution was interrupted because a prior agent run ended before returning results. Continue from the existing thread context.",
                    "is_error": true,
                }))
                .collect::<Vec<_>>(),
        }));
    }

    if repair_count > 0 {
        ctx.log(
            "warn",
            &format!(
                "llm_caller: repair_interrupted_tool_use_messages injected {repair_count} synthetic tool_result(s) — session_recoverer should have handled this (ADR-0025)"
            ),
        );
    }

    repaired
}

fn extract_tool_use_ids(message: &Value) -> BTreeSet<String> {
    message
        .get("content")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|block| block.get("type").and_then(Value::as_str) == Some("tool_use"))
        .filter_map(|block| block.get("id").and_then(Value::as_str))
        .map(ToString::to_string)
        .collect()
}

fn extract_tool_result_ids(message: &Value) -> BTreeSet<String> {
    message
        .get("content")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|block| block.get("type").and_then(Value::as_str) == Some("tool_result"))
        .filter_map(|block| block.get("tool_use_id").and_then(Value::as_str))
        .map(ToString::to_string)
        .collect()
}

fn prune_old_tool_results(messages: &mut [Value], keep_recent_turns: usize) {
    let total_assistant_turns = messages
        .iter()
        .filter(|m| m.get("role").and_then(|v| v.as_str()) == Some("assistant"))
        .count();
    if total_assistant_turns <= keep_recent_turns {
        return;
    }
    let cutoff = total_assistant_turns - keep_recent_turns;
    let mut assistant_turn = 0;
    for msg in messages.iter_mut() {
        let role = msg.get("role").and_then(|v| v.as_str()).unwrap_or("");
        if role == "assistant" {
            assistant_turn += 1;
        }
        if role == "user" && assistant_turn < cutoff {
            if let Some(content) = msg.get_mut("content") {
                if let Some(arr) = content.as_array_mut() {
                    for block in arr.iter_mut() {
                        if block.get("type").and_then(|v| v.as_str()) == Some("tool_result") {
                            if let Some(c) = block.get("content") {
                                let content_str = match c.as_str() {
                                    Some(s) => s.to_string(),
                                    None => serde_json::to_string(c).unwrap_or_default(),
                                };
                                if content_str.len() > 200 {
                                    block["content"] = json!(format!(
                                        "[tool result pruned — {} chars]",
                                        content_str.len()
                                    ));
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn mock_text_response(messages: &[Value], text: String) -> LlmResponse {
    LlmResponse {
        content: json!([{ "type": "text", "text": text.clone() }]),
        stop_reason: "end_turn".to_string(),
        input_tokens: estimate_message_tokens(messages),
        output_tokens: text.len() as i64,
        cache_read_input_tokens: 0,
        cache_creation_input_tokens: 0,
    }
}

fn estimate_message_tokens(messages: &[Value]) -> i64 {
    messages
        .iter()
        .map(|message| {
            message
                .get("content")
                .map(stringify_content)
                .unwrap_or_default()
                .len() as i64
        })
        .sum::<i64>()
}

fn latest_user_text(messages: &[Value]) -> Option<String> {
    messages
        .iter()
        .rev()
        .find(|message| message.get("role").and_then(Value::as_str) == Some("user"))
        .map(|message| stringify_content(message.get("content").unwrap_or(&Value::Null)))
}

fn resolve_mock_template(
    template: &str,
    assembled_system_prompt: &str,
    latest_user: &str,
) -> String {
    let mut text = template.to_string();
    text = text.replace("{{latest_user}}", latest_user);
    text = text.replace(
        "{{memory_block}}",
        &extract_tag_block(assembled_system_prompt, "agent_memory"),
    );
    text = text.replace(
        "{{memory_keys}}",
        &extract_memory_keys(assembled_system_prompt).join(", "),
    );
    text = text.replace(
        "{{memory_count}}",
        &extract_memory_keys(assembled_system_prompt)
            .len()
            .to_string(),
    );
    text = text.replace(
        "{{skills_block}}",
        &extract_tag_block(assembled_system_prompt, "available_skills"),
    );
    text
}

fn extract_tag_block(text: &str, tag: &str) -> String {
    let start_tag = format!("<{tag}>");
    let end_tag = format!("</{tag}>");
    let Some(start) = text.find(&start_tag) else {
        return String::new();
    };
    let Some(end) = text[start..].find(&end_tag) else {
        return String::new();
    };
    text[start..start + end + end_tag.len()].to_string()
}

fn extract_memory_keys(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|line| {
            let marker = "key=\"";
            let start = line.find(marker)? + marker.len();
            let rest = &line[start..];
            let end = rest.find('"')?;
            Some(rest[..end].to_string())
        })
        .collect()
}

fn convert_messages_to_openrouter(messages: &[Value]) -> Vec<Value> {
    let mut out = Vec::<Value>::new();
    for msg in messages {
        let role = msg.get("role").and_then(Value::as_str).unwrap_or("user");
        let content = msg.get("content").cloned().unwrap_or(json!(""));

        match content {
            Value::String(text) => {
                out.push(json!({
                    "role": role,
                    "content": text,
                }));
            }
            Value::Array(blocks) => {
                if role == "assistant" {
                    let mut text_chunks = Vec::<String>::new();
                    let mut tool_calls = Vec::<Value>::new();
                    for (idx, block) in blocks.iter().enumerate() {
                        match block.get("type").and_then(Value::as_str).unwrap_or("") {
                            "text" => {
                                if let Some(t) = block.get("text").and_then(Value::as_str) {
                                    text_chunks.push(t.to_string());
                                }
                            }
                            "tool_use" => {
                                let id = block
                                    .get("id")
                                    .and_then(Value::as_str)
                                    .map(|s| s.to_string())
                                    .unwrap_or_else(|| format!("tool_{}", idx + 1));
                                let name = block
                                    .get("name")
                                    .and_then(Value::as_str)
                                    .unwrap_or("unknown_tool");
                                let input = block.get("input").cloned().unwrap_or(json!({}));
                                tool_calls.push(json!({
                                    "id": id,
                                    "type": "function",
                                    "function": {
                                        "name": name,
                                        "arguments": input.to_string(),
                                    }
                                }));
                            }
                            _ => {}
                        }
                    }

                    let mut assistant = json!({
                        "role": "assistant",
                        "content": text_chunks.join("\n"),
                    });
                    if !tool_calls.is_empty() {
                        assistant["tool_calls"] = json!(tool_calls);
                    }
                    out.push(assistant);
                } else if role == "user" {
                    let mut user_text = Vec::<String>::new();
                    for block in &blocks {
                        match block.get("type").and_then(Value::as_str).unwrap_or("") {
                            "tool_result" => {
                                let tool_call_id = block
                                    .get("tool_use_id")
                                    .and_then(Value::as_str)
                                    .unwrap_or("unknown_tool_call");
                                let content = stringify_content(
                                    block
                                        .get("content")
                                        .unwrap_or(&Value::String(String::new())),
                                );
                                out.push(json!({
                                    "role": "tool",
                                    "tool_call_id": tool_call_id,
                                    "content": content,
                                }));
                            }
                            "text" => {
                                if let Some(t) = block.get("text").and_then(Value::as_str) {
                                    user_text.push(t.to_string());
                                }
                            }
                            _ => {}
                        }
                    }
                    if !user_text.is_empty() {
                        out.push(json!({
                            "role": "user",
                            "content": user_text.join("\n"),
                        }));
                    }
                } else {
                    out.push(json!({
                        "role": role,
                        "content": Value::Array(blocks),
                    }));
                }
            }
            other => {
                out.push(json!({
                    "role": role,
                    "content": other,
                }));
            }
        }
    }
    out
}

fn convert_tools_to_openrouter(tools: &[Value]) -> Vec<Value> {
    let mut out = Vec::<Value>::new();
    for tool in tools {
        let Some(name) = tool.get("name").and_then(Value::as_str) else {
            continue;
        };
        let description = tool
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("");
        let parameters = tool
            .get("input_schema")
            .cloned()
            .unwrap_or(json!({"type": "object", "properties": {}}));
        out.push(json!({
            "type": "function",
            "function": {
                "name": name,
                "description": description,
                "parameters": parameters,
            }
        }));
    }
    out
}

/// Build tool definitions for the LLM.
///
/// Returns a single `execute` tool for the Monty REPL. Agents write Python
/// code using `temper.*` and `sandbox.*` objects. The method listing is
/// inline in the tool description so agents see it immediately.
fn build_tool_definitions(tools_enabled: &str, _sandbox_url: &str, _workdir: &str) -> Vec<Value> {
    let enabled = enabled_tool_set(tools_enabled);
    let method_listing = build_method_listing(&enabled);
    let description = format!(
        "Execute Python code in the Temper REPL. Variables persist across calls.\n\n\
         Available methods:\n\
         {method_listing}\n\n\
         IMPORTANT PYTHON RULES (this is Monty, a restricted Python — NOT standard CPython):\n\
         - No 'import' statements at all (no import json, os, re, typing, sys — NOTHING)\n\
         - No enumerate(x, start=N) — use range(len(x)) instead\n\
         - No f-strings with nested quotes — use string concatenation\n\
         - No tuple comparison operators (<, >, etc.)\n\
         - All temper.* calls return pre-parsed dicts/lists — no json.loads() needed\n\
         - No pip packages (no requests, httpx, subprocess, os)\n\
         - Use sandbox.bash() for ALL shell commands when sandbox tools are enabled\n\
         Write complete multi-step scripts. Use simple Python: for/if/while, string concat, list indexing."
    );

    vec![json!({
        "name": "execute",
        "description": description,
        "input_schema": {
            "type": "object",
            "properties": {
                "code": { "type": "string", "description": "Python code to execute" }
            },
            "required": ["code"]
        }
    })]
}

/// Read conversation messages from TemperFS File entity via $value endpoint.
fn read_conversation_from_temperfs(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    file_id: &str,
    user_message: &str,
) -> Result<Vec<Value>, String> {
    let url = format!("{temper_api_url}/tdata/Files('{file_id}')/$value");
    let headers = agent_headers(ctx, tenant, None, Some("application/json"));

    const READ_ATTEMPTS: usize = 10;
    let mut last_status = 0;
    let mut last_body = String::new();

    for attempt in 0..READ_ATTEMPTS {
        match ctx.http_call("GET", &url, &headers, "") {
            Ok(resp) if resp.status == 200 => {
                let parsed: Value =
                    serde_json::from_str(&resp.body).unwrap_or(json!({"messages": []}));
                let messages = parsed
                    .get("messages")
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default();
                if messages.is_empty() {
                    return Ok(vec![json!({ "role": "user", "content": user_message })]);
                }
                return Ok(messages);
            }
            Ok(resp) if resp.status == 404 => {
                ctx.log(
                    "info",
                    "llm_caller: TemperFS file has no content, initializing",
                );
                return Ok(vec![json!({ "role": "user", "content": user_message })]);
            }
            Ok(resp) => {
                last_status = resp.status;
                last_body = resp.body;
                if (500..600).contains(&resp.status) && attempt + 1 < READ_ATTEMPTS {
                    ctx.log(
                        "warn",
                        &format!(
                            "llm_caller: TemperFS conversation read transient HTTP {}, retry {}/{}",
                            resp.status,
                            attempt + 2,
                            READ_ATTEMPTS
                        ),
                    );
                    continue;
                }
                break;
            }
            Err(e) => {
                ctx.log(
                    "warn",
                    &format!("llm_caller: TemperFS read error: {e}, falling back to inline"),
                );
                return Ok(vec![json!({ "role": "user", "content": user_message })]);
            }
        }
    }

    ctx.log(
        "warn",
        &format!(
            "llm_caller: TemperFS read failed (HTTP {}): {}, falling back to inline",
            last_status,
            &last_body[..last_body.len().min(200)]
        ),
    );
    Ok(vec![json!({ "role": "user", "content": user_message })])
}

/// Write conversation messages to TemperFS File entity via $value endpoint.
fn write_conversation_to_temperfs(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    file_id: &str,
    conversation_json: &str,
) -> Result<(), String> {
    let url = format!("{temper_api_url}/tdata/Files('{file_id}')/$value");
    let headers = agent_headers(ctx, tenant, Some("application/json"), None);

    // Wrap messages array in the TemperFS conversation format
    let body = format!("{{\"messages\":{conversation_json}}}");

    write_temperfs_value_with_retry(ctx, &url, &headers, &body, "TemperFS $value write failed")?;
    ctx.log(
        "info",
        &format!(
            "llm_caller: wrote conversation to TemperFS ({} bytes)",
            body.len()
        ),
    );
    Ok(())
}

fn read_temperfs_file_value(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    file_id: &str,
    content_type: Option<&str>,
    label: &str,
) -> Result<String, String> {
    let url = format!("{temper_api_url}/tdata/Files('{file_id}')/$value");
    let headers = agent_headers(ctx, tenant, None, content_type);

    const READ_ATTEMPTS: usize = 10;
    let mut last_status = 0;
    let mut last_body = String::new();

    for attempt in 0..READ_ATTEMPTS {
        let resp = ctx.http_call("GET", &url, &headers, "")?;
        if resp.status == 200 {
            return Ok(resp.body);
        }
        if resp.status == 404 {
            return Ok(String::new());
        }

        last_status = resp.status;
        last_body = resp.body;

        if (500..600).contains(&last_status) && attempt + 1 < READ_ATTEMPTS {
            ctx.log(
                "warn",
                &format!(
                    "llm_caller: {label} transient HTTP {}, retry {}/{}",
                    last_status,
                    attempt + 2,
                    READ_ATTEMPTS
                ),
            );
            continue;
        }
        break;
    }

    Err(format!(
        "{label} (HTTP {}): {}",
        last_status,
        &last_body[..last_body.len().min(200)]
    ))
}

/// Read session JSONL from TemperFS.
fn read_session_from_temperfs(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    file_id: &str,
) -> Result<String, String> {
    read_temperfs_file_value(
        ctx,
        temper_api_url,
        tenant,
        file_id,
        None,
        "TemperFS session read failed",
    )
}

/// Write session JSONL to TemperFS.
fn write_session_to_temperfs(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    file_id: &str,
    jsonl: &str,
) -> Result<(), String> {
    let url = format!("{temper_api_url}/tdata/Files('{file_id}')/$value");
    let headers = agent_headers(ctx, tenant, Some("text/plain"), None);
    write_temperfs_value_with_retry(ctx, &url, &headers, jsonl, "TemperFS session write failed")
}

/// Load project harness conventions as a context block for the system prompt.
/// Acts like CLAUDE.md for Claude Code — auto-injected tech stack and conventions.
fn load_harness_block(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    project_harness_id: &str,
) -> Result<String, String> {
    if project_harness_id.is_empty() {
        return Ok(String::new());
    }
    let headers = agent_headers(ctx, tenant, None, Some("application/json"));
    let url = format!("{temper_api_url}/tdata/Harnesses('{project_harness_id}')");
    let resp = ctx.http_call("GET", &url, &headers, "")?;
    if resp.status != 200 {
        ctx.log(
            "warn",
            &format!(
                "load_harness_block: failed to fetch harness {project_harness_id} (HTTP {})",
                resp.status
            ),
        );
        return Ok(String::new());
    }
    let parsed: Value = serde_json::from_str(&resp.body).unwrap_or(json!({}));
    let tech_stack = entity_field_str(&parsed, &["TechStack", "tech_stack"]).unwrap_or("");
    let conventions = entity_field_str(&parsed, &["Conventions", "conventions"]).unwrap_or("");
    if tech_stack.is_empty() && conventions.is_empty() {
        return Ok(String::new());
    }
    let id_attr = entity_field_str(&parsed, &["Id", "id"]).unwrap_or(project_harness_id);
    let mut block = format!("<project_harness id=\"{id_attr}\">\n");
    if !tech_stack.is_empty() {
        block.push_str(&format!("<tech_stack>\n{tech_stack}\n</tech_stack>\n"));
    }
    if !conventions.is_empty() {
        block.push_str(&format!("<conventions>\n{conventions}\n</conventions>\n"));
    }
    block.push_str("</project_harness>");
    Ok(block)
}

/// Read a TemperFS file's content as a string (convenience wrapper).
fn read_temperfs_file(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    file_id: &str,
) -> Result<String, String> {
    read_temperfs_file_value(
        ctx,
        temper_api_url,
        tenant,
        file_id,
        None,
        "read_temperfs_file",
    )
}

/// Write system prompt content to a new TemperFS File entity, returning the file_id.
fn write_system_prompt_cache(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    workspace_id: &str,
    content: &str,
) -> Result<String, String> {
    // Create File entity
    let headers = agent_headers(
        ctx,
        tenant,
        Some("application/json"),
        Some("application/json"),
    );
    let ws = if workspace_id.is_empty() {
        "default"
    } else {
        workspace_id
    };
    let body = json!({
        "name": "system-prompt-cache.txt",
        "path": format!("/system/cache/system-prompt-{}.txt", &ctx.entity_id),
        "workspace_id": ws,
    });
    let url = format!("{temper_api_url}/tdata/Files");
    let resp = ctx.http_call("POST", &url, &headers, &body.to_string())?;
    if resp.status < 200 || resp.status >= 300 {
        return Err(format!(
            "create system prompt cache file failed (HTTP {})",
            resp.status
        ));
    }
    let parsed: Value = serde_json::from_str(&resp.body).unwrap_or(json!({}));
    let file_id = entity_field_str(&parsed, &["Id", "entity_id"])
        .unwrap_or("")
        .to_string();
    if file_id.is_empty() {
        return Err("created system prompt cache file but got no id".to_string());
    }
    // Write content
    let value_url = format!("{temper_api_url}/tdata/Files('{file_id}')/$value");
    let value_headers = agent_headers(ctx, tenant, Some("text/plain"), None);
    write_temperfs_value_with_retry(
        ctx,
        &value_url,
        &value_headers,
        content,
        "system prompt cache write",
    )?;
    Ok(file_id)
}

/// Compute a simple hash of the system prompt component inputs for caching.
fn compute_system_prompt_hash(
    soul_id: &str,
    agent_id: &str,
    project_harness_id: &str,
    project_id: &str,
    session_mode: &str,
    active_plan_id: &str,
    system_prompt_override: &str,
) -> String {
    // Simple additive hash — we just need change detection, not cryptographic security.
    let mut hash: u64 = 0xcbf29ce484222325; // FNV offset basis
    for b in soul_id
        .bytes()
        .chain(b"|".iter().copied())
        .chain(agent_id.bytes())
        .chain(b"|".iter().copied())
        .chain(project_harness_id.bytes())
        .chain(b"|".iter().copied())
        .chain(project_id.bytes())
        .chain(b"|".iter().copied())
        .chain(session_mode.bytes())
        .chain(b"|".iter().copied())
        .chain(active_plan_id.bytes())
        .chain(b"|".iter().copied())
        .chain(system_prompt_override.bytes())
    {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3); // FNV prime
    }
    format!("{:016x}", hash)
}

/// Assemble the full system prompt from soul + override + harness + skills + memory.
fn assemble_system_prompt(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    soul_id: &str,
    system_prompt_override: &str,
) -> Result<String, String> {
    let mut parts: Vec<String> = Vec::new();

    // 1. Soul content
    if !soul_id.is_empty() {
        match load_soul_content(ctx, temper_api_url, tenant, soul_id) {
            Ok(content) if !content.is_empty() => parts.push(content),
            Ok(_) => ctx.log("warn", "assemble_system_prompt: soul content is empty"),
            Err(e) => ctx.log(
                "warn",
                &format!("assemble_system_prompt: failed to load soul: {e}"),
            ),
        }
    }

    // 1b. Agent instructions (from Agent entity's instructions_file_id)
    {
        let agent_id = ctx
            .entity_state
            .get("fields")
            .and_then(|f| f.get("agent_id").or_else(|| f.get("AgentId")))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if !agent_id.is_empty() {
            match load_agent_instructions(ctx, temper_api_url, tenant, agent_id) {
                Ok(content) if !content.is_empty() => parts.push(content),
                Ok(_) => {}
                Err(e) => ctx.log(
                    "warn",
                    &format!("assemble_system_prompt: failed to load agent instructions: {e}"),
                ),
            }
        }
    }

    // 2. System prompt override
    if !system_prompt_override.is_empty() {
        parts.push(system_prompt_override.to_string());
    }

    // 2b. Project harness conventions (auto-injected like CLAUDE.md)
    {
        let fields_val = ctx.entity_state.get("fields");
        let project_harness_id = fields_val
            .and_then(|f| {
                f.get("project_harness_id")
                    .or_else(|| f.get("ProjectHarnessId"))
            })
            .and_then(|v| v.as_str())
            .unwrap_or("");
        match load_harness_block(ctx, temper_api_url, tenant, project_harness_id) {
            Ok(block) if !block.is_empty() => parts.push(block),
            Ok(_) => {}
            Err(e) => ctx.log(
                "warn",
                &format!("assemble_system_prompt: failed to load harness: {e}"),
            ),
        }
    }

    // 3. Available skills — discovered from TemperFS SKILL.md files (ADR-002)
    //    Path = scope: /system/skills/, /agents/{id}/skills/, /projects/{id}/skills/
    {
        let fields_val = ctx.entity_state.get("fields");
        let agent_id = fields_val
            .and_then(|f| f.get("agent_id").or_else(|| f.get("AgentId")))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let project_id = fields_val
            .and_then(|f| f.get("project_id").or_else(|| f.get("ProjectId")))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        match load_skills_block(ctx, temper_api_url, tenant, project_id, agent_id) {
            Ok(block) if !block.is_empty() => parts.push(block),
            Ok(_) => {}
            Err(e) => ctx.log(
                "warn",
                &format!("assemble_system_prompt: failed to load skills: {e}"),
            ),
        }
    }

    // 3b. Plan-mode instructions — conditional on session_mode, NOT a system skill (ADR-004)
    {
        let session_mode = ctx
            .entity_state
            .get("fields")
            .and_then(|f| f.get("session_mode"))
            .and_then(|v| v.as_str())
            .unwrap_or("execute");
        if session_mode == "plan" {
            match load_mode_instructions(ctx, temper_api_url, tenant, "plan") {
                Ok(content) if !content.is_empty() => parts.push(content),
                Ok(_) => {
                    parts.push(PLAN_MODE_FALLBACK.to_string());
                }
                Err(e) => {
                    ctx.log(
                        "warn",
                        &format!("assemble_system_prompt: plan mode instructions failed: {e}"),
                    );
                    parts.push(PLAN_MODE_FALLBACK.to_string());
                }
            }
        }
    }

    // 3c. Active plan injection — when executing after planning (ADR-004)
    {
        let session_mode = ctx
            .entity_state
            .get("fields")
            .and_then(|f| f.get("session_mode"))
            .and_then(|v| v.as_str())
            .unwrap_or("execute");
        if session_mode == "execute" {
            let plan_id = ctx
                .entity_state
                .get("fields")
                .and_then(|f| f.get("active_plan_id"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if !plan_id.is_empty() {
                match load_active_plan(ctx, temper_api_url, tenant, plan_id) {
                    Ok(content) if !content.is_empty() => {
                        parts.push(format!("<active_plan>\n{}\n</active_plan>", content));
                    }
                    Ok(_) => {}
                    Err(e) => ctx.log(
                        "warn",
                        &format!("assemble_system_prompt: failed to load active plan: {e}"),
                    ),
                }
            }
        }
    }

    // 4. Memory context — scoped to agent, not soul (ADR-0007)
    {
        let entity_id = ctx
            .entity_state
            .get("entity_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        match load_memory_block(ctx, temper_api_url, tenant, entity_id) {
            Ok(block) if !block.is_empty() => parts.push(block),
            Ok(_) => {}
            Err(e) => ctx.log(
                "warn",
                &format!("assemble_system_prompt: failed to load memory: {e}"),
            ),
        }
    }

    // 5. Temper SDK reference (available REPL commands)
    {
        let tools_enabled = ctx
            .entity_state
            .get("fields")
            .and_then(|f| f.get("tools_enabled"))
            .and_then(|v| v.as_str())
            .unwrap_or(DEFAULT_TOOLS_ENABLED);
        let sandbox_url = ctx
            .entity_state
            .get("fields")
            .and_then(|f| f.get("sandbox_url"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let workdir = ctx
            .entity_state
            .get("fields")
            .and_then(|f| f.get("workdir"))
            .and_then(|v| v.as_str())
            .unwrap_or("/workspace");
        parts.push(build_sdk_reference(tools_enabled, sandbox_url, workdir));
    }

    // Fall back to bare system_prompt if nothing loaded
    if parts.is_empty() {
        return Ok(system_prompt_override.to_string());
    }

    Ok(parts.join("\n\n"))
}

/// Build the Temper SDK usage guide for the system prompt.
///
/// Contains examples and constraints only — method signatures live in the
/// `execute` tool description so agents see them immediately.
fn build_sdk_reference(tools_enabled: &str, sandbox_url: &str, workdir: &str) -> String {
    let enabled = enabled_tool_set(tools_enabled);
    let has_sandbox = has_sandbox_surface(&enabled);

    let mut sections = Vec::new();
    let sandbox_note = if has_sandbox && sandbox_url.is_empty() {
        " Two objects are available: `temper` (platform API) and `sandbox` (remote shell/files, provisioned on demand when you first use a sandbox tool)."
    } else if has_sandbox {
        " Two objects are available: `temper` (platform API) and `sandbox` (remote shell/files)."
    } else {
        " One object is available: `temper` (platform API)."
    };

    sections.push(format!(
        "<temper_sdk>\n\
         ## Execution Environment\n\n\
         Your `execute` tool runs Python in a sandboxed REPL.{sandbox_note}\n\n\
         Constraints (Monty — restricted Python, NOT standard CPython):\n\
         - No 'import' statements at all (no import json, os, re, typing, sys)\n\
         - No enumerate(x, start=N) — use range(len(x)) instead\n\
         - No f-strings with nested quotes — use string concatenation\n\
         - No tuple comparison (<, >) — compare individual elements\n\
         - All temper.* calls return pre-parsed dicts/lists — no json.loads() needed\n\
         - No pip packages (no requests, httpx, numpy, pandas, etc.)\n\
         - No network access from Python — use sandbox.bash(\"curl ...\") for HTTP\n\
         - No filesystem access from Python — use sandbox.read/write/edit\n\
         - Python variables and helper definitions persist across execute calls within this session; persist important artifacts to Temper entities or Files so they survive crashes, handoffs, or later jobs\n\
         - Prefer short, focused execute scripts; avoid monolithic one-shot programs when the task can be split across turns\n\
         - If a script starts getting large, stop and continue in a follow-up execute call instead of building a giant helper framework\n\
         - Write substantial code blocks using simple Python: for/if/while, string concat, list indexing\n\
         - Sandbox working directory: {workdir}",
        sandbox_note = sandbox_note,
    ));

    // --- Examples ---
    let mut examples = String::from("## Examples\n");
    if has_sandbox {
        examples.push_str(
            "\n### Clone and explore\n\
             ```python\n\
             sandbox.bash(\"git clone https://github.com/org/repo.git /workspace/repo\")\n\
             content = sandbox.read(\"/workspace/repo/README.md\")\n\
             print(content[:500])\n\
             ```\n\
             \n### Edit + test + commit\n\
             ```python\n\
             sandbox.edit(\"/workspace/repo/src/main.py\",\n\
                 old=\"def hello():\",\n\
                 new=\"def hello(name='World'):\")\n\
             result = sandbox.bash(\"cd /workspace/repo && pytest tests/ -x -q\")\n\
             print(result)\n\
             sandbox.bash(\"cd /workspace/repo && git add -A && git commit -m 'fix: greet by name'\")\n\
             ```\n",
        );
    }
    examples.push_str(
        "\n### Entity CRUD + memory\n\
         ```python\n\
         issue = temper.create(\"Issues\", {\"description\": \"Fix login bug\"})\n\
         temper.action(\"Issues\", issue[\"entity_id\"], \"OpenPaw.PM.MoveToTriage\", {})\n\
         temper.save_memory(\"test_results\", \"pytest: 47 passed, 0 failed\", \"project\")\n\
         ```\n",
    );
    sections.push(examples);

    sections.push(
        "## Efficiency\n\n\
         Batch closely related steps in one execute call, but do not force an entire long-running workflow into one giant script.\n\
         BAD: 5 separate execute calls for 5 one-line operations\n\
         BAD: 1 monolithic execute call that tries to ingest, plan, synthesize, and publish everything at once\n\
         GOOD: 1 focused execute call per coherent chunk of work, with state carried in the session between calls\n\n\
         Each execute call is an LLM turn. Fewer turns help, but reliability matters more than forcing one oversized script."
            .to_string(),
    );

    sections.push("</temper_sdk>".to_string());

    sections.join("\n\n")
}

/// Load soul content from Soul entity.
fn load_soul_content(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    soul_id: &str,
) -> Result<String, String> {
    let soul = resolve_soul_entity(ctx, temper_api_url, tenant, soul_id)?;
    let content_file_id =
        entity_field_str(&soul, &["ContentFileId", "content_file_id"]).unwrap_or("");
    if content_file_id.is_empty() {
        return Ok(String::new());
    }
    read_temperfs_file_value(
        ctx,
        temper_api_url,
        tenant,
        content_file_id,
        Some("application/json"),
        "TemperFS soul content read failed",
    )
    .or_else(|_| Ok(String::new()))
}

fn resolve_soul_entity(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    soul_ref: &str,
) -> Result<Value, String> {
    let headers = agent_headers(ctx, tenant, None, Some("application/json"));
    let url = format!("{temper_api_url}/tdata/Souls('{soul_ref}')");
    let resp = ctx.http_call("GET", &url, &headers, "")?;
    if resp.status == 200 {
        return serde_json::from_str(&resp.body)
            .map_err(|e| format!("failed to parse soul JSON: {e}"));
    }

    let escaped = soul_ref.replace('\'', "''");
    let by_name_url =
        format!("{temper_api_url}/tdata/Souls?$filter=name eq '{escaped}' and Status eq 'Active'");
    let resp = ctx.http_call("GET", &by_name_url, &headers, "")?;
    if resp.status != 200 {
        return Err(format!("soul read failed (HTTP {})", resp.status));
    }
    let parsed: Value = serde_json::from_str(&resp.body).unwrap_or_else(|_| json!({}));
    parsed
        .get("value")
        .and_then(Value::as_array)
        .and_then(|souls| souls.first())
        .cloned()
        .ok_or_else(|| "soul read failed (no active soul matched reference)".to_string())
}

fn normalize_skill_key(name: &str) -> String {
    name.to_ascii_lowercase()
        .replace('_', "-")
        .replace(' ', "-")
}

/// Extract skill name from a TemperFS path.
/// "/skills/my-skill/SKILL.md" → "my-skill"
/// "/projects/pid/skills/my-skill/SKILL.md" → "my-skill"
fn skill_name_from_path(path: &str) -> String {
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    if segments.len() >= 2 {
        segments[segments.len() - 2].to_string()
    } else {
        "unknown".to_string()
    }
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Parse YAML or TOML frontmatter from SKILL.md content.
fn parse_skill_frontmatter(content: &str) -> (String, String, String) {
    let mut name = String::new();
    let mut description = String::new();
    let mut scope = "global".to_string();

    // Try YAML frontmatter (---)
    if content.starts_with("---") {
        if let Some(end_idx) = content[3..].find("\n---") {
            let fm_block = &content[3..3 + end_idx];
            for line in fm_block.lines() {
                let trimmed = line.trim();
                if let Some(val) = trimmed.strip_prefix("name:") {
                    name = val.trim().trim_matches('"').trim_matches('\'').to_string();
                } else if let Some(val) = trimmed.strip_prefix("description:") {
                    description = val.trim().trim_matches('"').trim_matches('\'').to_string();
                } else if let Some(val) = trimmed.strip_prefix("scope:") {
                    scope = val.trim().trim_matches('"').trim_matches('\'').to_string();
                }
            }
        }
    }
    // Fall back to TOML frontmatter (+++)
    else if content.starts_with("+++") {
        if let Some(end_idx) = content[3..].find("\n+++") {
            let fm_block = &content[3..3 + end_idx];
            for line in fm_block.lines() {
                let trimmed = line.trim();
                if let Some(val) = trimmed
                    .strip_prefix("name")
                    .and_then(|r| r.trim().strip_prefix('='))
                {
                    name = val.trim().trim_matches('"').trim_matches('\'').to_string();
                } else if let Some(val) = trimmed
                    .strip_prefix("description")
                    .and_then(|r| r.trim().strip_prefix('='))
                {
                    description = val.trim().trim_matches('"').trim_matches('\'').to_string();
                } else if let Some(val) = trimmed
                    .strip_prefix("scope")
                    .and_then(|r| r.trim().strip_prefix('='))
                {
                    scope = val.trim().trim_matches('"').trim_matches('\'').to_string();
                }
            }
        }
    }

    (name, description, scope)
}

/// Strip YAML or TOML frontmatter from skill content, returning the body after the closing delimiter.
fn strip_skill_frontmatter(content: &str) -> &str {
    if content.starts_with("---") {
        if let Some(end_idx) = content[3..].find("\n---") {
            let after = 3 + end_idx + 4; // skip past "\n---"
            if after <= content.len() {
                return content[after..].trim_start();
            }
        }
    } else if content.starts_with("+++") {
        if let Some(end_idx) = content[3..].find("\n+++") {
            let after = 3 + end_idx + 4;
            if after <= content.len() {
                return content[after..].trim_start();
            }
        }
    }
    content
}

/// Load skills from TemperFS SKILL.md files as an XML block for the system prompt.
///
/// Skills are discovered by path convention (ADR-002). Path = scope:
///   /system/skills/{name}/SKILL.md           — system-level (platform knowledge, all agents)
///   /agents/{agent-id}/skills/{name}/SKILL.md — agent-scoped (from app bootstrap or runtime)
///   /projects/{pid}/skills/{name}/SKILL.md   — project-scoped (runtime, created by leads)
///
/// No frontmatter scope filtering. Precedence on name collision: agent > project > system.
/// Agents use temper.read(path) for progressive disclosure.
fn load_skills_block(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    project_id: &str,
    agent_id: &str,
) -> Result<String, String> {
    let headers = agent_headers(ctx, tenant, None, Some("application/json"));

    // Build path prefixes to search — path = scope
    let mut prefixes = vec!["/system/skills/".to_string()];
    if !project_id.is_empty() {
        prefixes.push(format!("/projects/{project_id}/skills/"));
    }
    if !agent_id.is_empty() {
        prefixes.push(format!("/agents/{agent_id}/skills/"));
    }

    // Collect all SKILL.md files across all prefixes
    let mut file_entries: Vec<(String, String)> = Vec::new(); // (file_id, path)

    for prefix in &prefixes {
        let filter =
            format!("startswith(path,'{prefix}') and name eq 'SKILL.md' and Status ne 'Archived'");
        let url = format!("{temper_api_url}/tdata/Files?$filter={filter}");
        match ctx.http_call("GET", &url, &headers, "") {
            Ok(resp) if resp.status == 200 => {
                let parsed: Value = serde_json::from_str(&resp.body).unwrap_or(json!({}));
                if let Some(items) = parsed.get("value").and_then(|v| v.as_array()) {
                    for item in items {
                        let id = entity_field_str(item, &["Id", "entity_id"])
                            .unwrap_or("")
                            .to_string();
                        let path = entity_field_str(item, &["Path", "path"])
                            .unwrap_or("")
                            .to_string();
                        if !id.is_empty() {
                            file_entries.push((id, path));
                        }
                    }
                }
            }
            Ok(resp) => ctx.log(
                "warn",
                &format!(
                    "load_skills_block: file query for prefix {prefix} returned HTTP {}",
                    resp.status
                ),
            ),
            Err(e) => ctx.log(
                "warn",
                &format!("load_skills_block: file query for prefix {prefix} failed: {e}"),
            ),
        }
    }

    if file_entries.is_empty() {
        return Ok(String::new());
    }

    // Read each file's content, parse frontmatter for name + description.
    // Tuple: (norm_key, scope_priority, name, desc, path, body)
    // scope_priority: 0 = agent (most specific), 1 = project, 2 = system (least specific)
    // body: full content (sans frontmatter) for system skills; empty for others (L0 only)
    let mut entries: Vec<(String, u8, String, String, String, String)> = Vec::new();

    for (file_id, path) in &file_entries {
        let url = format!("{temper_api_url}/tdata/Files('{file_id}')/$value");
        match ctx.http_call("GET", &url, &headers, "") {
            Ok(resp) if resp.status == 200 && !resp.body.is_empty() => {
                let (fm_name, fm_desc, _) = parse_skill_frontmatter(&resp.body);

                let name = if fm_name.is_empty() {
                    skill_name_from_path(path)
                } else {
                    fm_name
                };

                let scope_priority = if path.starts_with("/agents/") {
                    0
                } else if path.starts_with("/projects/") {
                    1
                } else {
                    2
                };

                // System skills get fully injected; others stay L0 (name+desc only)
                let body = if scope_priority == 2 {
                    strip_skill_frontmatter(&resp.body).to_string()
                } else {
                    String::new()
                };

                entries.push((
                    normalize_skill_key(&name),
                    scope_priority,
                    name,
                    fm_desc,
                    path.clone(),
                    body,
                ));
            }
            Ok(_) => {} // silently skip empty or missing files
            Err(e) => ctx.log(
                "warn",
                &format!("load_skills_block: failed to read {file_id}: {e}"),
            ),
        }
    }

    if entries.is_empty() {
        return Ok(String::new());
    }

    // Sort by (norm_key, scope_priority) so most-specific scope wins dedup.
    entries.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    let mut seen_names = BTreeSet::new();
    let mut xml = String::from("<available_skills>\n");
    for (norm, _priority, name, desc, skill_path, body) in &entries {
        if !seen_names.insert(norm.clone()) {
            continue;
        }
        if body.is_empty() {
            // L0: name + description + path only (project/agent-scoped skills)
            xml.push_str(&format!(
                "  <skill name=\"{}\" description=\"{}\" path=\"{}\" />\n",
                xml_escape(name),
                xml_escape(desc),
                xml_escape(skill_path),
            ));
        } else {
            // Full injection: system skills include their complete content
            xml.push_str(&format!(
                "  <skill name=\"{}\" description=\"{}\" path=\"{}\">\n{}\n  </skill>\n",
                xml_escape(name),
                xml_escape(desc),
                xml_escape(skill_path),
                body,
            ));
        }
    }
    xml.push_str("</available_skills>");
    Ok(xml)
}

/// Load agent instructions from the Agent entity's instructions_file_id.
///
/// Queries the Agent entity by ID, reads the InstructionsFileId field,
/// and fetches the file content from TemperFS.
fn load_agent_instructions(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    agent_id: &str,
) -> Result<String, String> {
    let headers = agent_headers(ctx, tenant, None, Some("application/json"));
    let url = format!("{temper_api_url}/tdata/Agents('{agent_id}')");
    let resp = ctx.http_call("GET", &url, &headers, "")?;
    if resp.status != 200 {
        return Ok(String::new());
    }
    let agent: Value =
        serde_json::from_str(&resp.body).map_err(|e| format!("parse agent JSON: {e}"))?;
    let file_id =
        entity_field_str(&agent, &["InstructionsFileId", "instructions_file_id"]).unwrap_or("");
    if file_id.is_empty() {
        return Ok(String::new());
    }
    let file_url = format!("{temper_api_url}/tdata/Files('{file_id}')/$value");
    let file_resp = ctx.http_call("GET", &file_url, &headers, "")?;
    if file_resp.status == 200 && !file_resp.body.is_empty() {
        Ok(file_resp.body)
    } else {
        Ok(String::new())
    }
}

/// Minimal fallback plan-mode instructions if the TemperFS file is not deployed.
const PLAN_MODE_FALLBACK: &str = "\
# Plan Mode\n\
\n\
You are in PLAN MODE. Investigate thoroughly and produce a Plan entity.\n\
You CANNOT modify code or write sandbox files. You CAN read, explore, research,\n\
and write plan documents to TemperFS via temper.write().\n\
\n\
Use sandbox.read() and sandbox.bash() for read-only exploration.\n\
Create/update Plan entities with temper.create()/temper.action().\n\
When ready, call temper.switch_mode({\"mode\": \"execute\"}) to resume with full tools.";

/// Load mode-specific instructions from TemperFS at /system/mode-instructions/{mode}.md
fn load_mode_instructions(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    mode: &str,
) -> Result<String, String> {
    let headers = agent_headers(ctx, tenant, None, Some("application/json"));
    // Find the mode instruction file by path
    let path = format!("/system/mode-instructions/{mode}.md");
    let filter = format!("path eq '{path}' and Status ne 'Archived'");
    let url = format!("{temper_api_url}/tdata/Files?$filter={filter}");
    let resp = ctx.http_call("GET", &url, &headers, "")?;
    if resp.status != 200 {
        return Ok(String::new());
    }
    let parsed: Value = serde_json::from_str(&resp.body).unwrap_or(json!({}));
    let file_id = parsed
        .get("value")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .and_then(|item| entity_field_str(item, &["Id", "entity_id"]))
        .unwrap_or("");
    if file_id.is_empty() {
        return Ok(String::new());
    }
    let file_url = format!("{temper_api_url}/tdata/Files('{file_id}')/$value");
    let file_resp = ctx.http_call("GET", &file_url, &headers, "")?;
    if file_resp.status == 200 && !file_resp.body.is_empty() {
        Ok(strip_skill_frontmatter(&file_resp.body).to_string())
    } else {
        Ok(String::new())
    }
}

/// Load active plan content from Plan entity's plan_file_id.
fn load_active_plan(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    plan_id: &str,
) -> Result<String, String> {
    let headers = agent_headers(ctx, tenant, None, Some("application/json"));
    let url = format!("{temper_api_url}/tdata/Plans('{plan_id}')");
    let resp = ctx.http_call("GET", &url, &headers, "")?;
    if resp.status != 200 {
        return Ok(String::new());
    }
    let plan: Value =
        serde_json::from_str(&resp.body).map_err(|e| format!("parse plan JSON: {e}"))?;
    let file_id = entity_field_str(&plan, &["PlanFileId", "plan_file_id"]).unwrap_or("");
    if file_id.is_empty() {
        // Fall back to inline plan_text
        let text = entity_field_str(&plan, &["PlanText", "plan_text"]).unwrap_or("");
        return Ok(text.to_string());
    }
    let file_url = format!("{temper_api_url}/tdata/Files('{file_id}')/$value");
    let file_resp = ctx.http_call("GET", &file_url, &headers, "")?;
    if file_resp.status == 200 && !file_resp.body.is_empty() {
        Ok(file_resp.body)
    } else {
        Ok(String::new())
    }
}

/// Load agent memories as a context block for the system prompt.
fn load_memory_block(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    entity_id: &str,
) -> Result<String, String> {
    let url = format!(
        "{temper_api_url}/tdata/Memories?$filter=AgentId eq '{}' and Status eq 'Active'",
        entity_id
    );
    let headers = agent_headers(ctx, tenant, None, Some("application/json"));
    let resp = ctx.http_call("GET", &url, &headers, "")?;
    if resp.status != 200 {
        return Ok(String::new());
    }
    let parsed: Value = serde_json::from_str(&resp.body).unwrap_or(json!({}));
    let memories = parsed
        .get("value")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    if memories.is_empty() {
        return Ok(String::new());
    }
    let mut block = String::from("<agent_memory>\n");
    for mem in &memories {
        let key = entity_field_str(mem, &["Key"]).unwrap_or("unknown");
        let content = entity_field_str(mem, &["Content"]).unwrap_or("");
        let mem_type = entity_field_str(mem, &["MemoryType"]).unwrap_or("reference");
        block.push_str(&format!(
            "  <memory key=\"{key}\" type=\"{mem_type}\">\n    {content}\n  </memory>\n"
        ));
    }
    block.push_str("</agent_memory>");
    Ok(block)
}

fn direct_field_str<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
}

fn entity_field_str<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a str> {
    direct_field_str(value, keys).or_else(|| {
        value
            .get("fields")
            .and_then(|fields| direct_field_str(fields, keys))
    })
}

fn resolve_context_refs(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    refs: &[session_tree_lib::ContextRef],
) -> Result<Vec<Value>, String> {
    let mut messages = Vec::new();

    for ctx_ref in refs {
        match ctx_ref.entry_type {
            EntryType::Compaction => {
                let summary = if let Some(ref file_id) = ctx_ref.content_file_id {
                    read_content_file_raw(ctx, temper_api_url, tenant, file_id)
                        .unwrap_or_else(|_| ctx_ref.inline_summary.clone().unwrap_or_default())
                } else {
                    ctx_ref.inline_summary.clone().unwrap_or_default()
                };
                if !summary.is_empty() {
                    messages.push(json!({
                        "role": "user",
                        "content": format!("[Previous conversation summary]\n{summary}")
                    }));
                }
            }
            EntryType::Message | EntryType::Steering => {
                if let Some(ref file_id) = ctx_ref.content_file_id {
                    let raw = read_content_file_raw(ctx, temper_api_url, tenant, file_id)?;
                    if raw.is_empty() {
                        if let Some(ref inline) = ctx_ref.inline_content {
                            messages.push(json!({
                                "role": ctx_ref.role,
                                "content": inline.clone(),
                            }));
                        }
                        continue;
                    }
                    let content: Value = serde_json::from_str(&raw).unwrap_or(json!(raw));
                    messages.push(json!({
                        "role": ctx_ref.role,
                        "content": content,
                    }));
                } else if let Some(ref inline) = ctx_ref.inline_content {
                    messages.push(json!({
                        "role": ctx_ref.role,
                        "content": inline.clone(),
                    }));
                }
            }
            EntryType::Header => {}
        }
    }

    Ok(messages)
}

fn read_content_file_raw(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    file_id: &str,
) -> Result<String, String> {
    read_temperfs_file_value(
        ctx,
        temper_api_url,
        tenant,
        file_id,
        None,
        "Content file read failed",
    )
}

fn create_content_file_for_entry(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    workspace_id: &str,
    entry_id: &str,
    content: &str,
) -> Result<String, String> {
    let file_name = format!("msg-{entry_id}.txt");
    create_content_file(
        ctx,
        temper_api_url,
        tenant,
        workspace_id,
        &file_name,
        content,
    )
}

fn should_store_entry_as_file(content: &str) -> bool {
    content.len() > SESSION_ENTRY_FILE_THRESHOLD_BYTES
}

fn resolve_temper_api_url(ctx: &Context, fields: &Value) -> String {
    fields
        .get("temper_api_url")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .or_else(
            || match ctx.config.get("temper_api_url").map(String::as_str) {
                Some(value) if !value.trim().is_empty() && !value.contains("{secret:") => {
                    Some(value.to_string())
                }
                _ => None,
            },
        )
        .unwrap_or_else(|| "http://127.0.0.1:3000".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gen_ai_system_instructions_use_otel_schema() {
        let payload = build_gen_ai_system_instructions("You are a precise assistant.");
        let parsed: Value = serde_json::from_str(&payload).unwrap();
        assert_eq!(parsed[0]["type"], "text");
        assert_eq!(parsed[0]["content"], "You are a precise assistant.");
    }

    #[test]
    fn gen_ai_input_messages_preserve_chat_history_and_tool_results() {
        let payload = build_gen_ai_input_messages(&[
            json!({"role": "user", "content": "List the recent sessions."}),
            json!({
                "role": "assistant",
                "content": [{
                    "type": "tool_use",
                    "id": "tool_123",
                    "name": "temper.list_sessions",
                    "input": {"top": 3}
                }]
            }),
            json!({
                "role": "user",
                "content": [{
                    "type": "tool_result",
                    "tool_use_id": "tool_123",
                    "content": [{"type": "text", "text": "[\"s1\",\"s2\"]"}],
                    "is_error": false
                }]
            }),
        ]);

        let parsed: Value = serde_json::from_str(&payload).unwrap();
        assert_eq!(parsed[0]["role"], "user");
        assert_eq!(parsed[0]["parts"][0]["type"], "text");
        assert_eq!(parsed[1]["role"], "assistant");
        assert_eq!(parsed[1]["parts"][0]["type"], "tool_call");
        assert_eq!(parsed[1]["parts"][0]["name"], "temper.list_sessions");
        assert_eq!(parsed[2]["role"], "tool");
        assert_eq!(parsed[2]["parts"][0]["type"], "tool_call_response");
        assert_eq!(parsed[2]["parts"][0]["id"], "tool_123");
    }

    #[test]
    fn gen_ai_output_messages_preserve_text_and_tool_calls() {
        let payload = build_gen_ai_output_messages(
            &json!([
                {"type": "text", "text": "I need to inspect the latest sessions first."},
                {
                    "type": "tool_use",
                    "id": "tool_456",
                    "name": "temper.list_sessions",
                    "input": {"top": 5}
                }
            ]),
            "tool_use",
        );

        let parsed: Value = serde_json::from_str(&payload).unwrap();
        assert_eq!(parsed[0]["role"], "assistant");
        assert_eq!(parsed[0]["finish_reason"], "tool_use");
        assert_eq!(parsed[0]["parts"][0]["type"], "text");
        assert_eq!(parsed[0]["parts"][1]["type"], "tool_call");
        assert_eq!(parsed[0]["parts"][1]["id"], "tool_456");
    }

    #[test]
    fn strip_yaml_frontmatter_normal() {
        let input = "---\nname: foo\ndescription: bar\n---\n# Body\nContent here";
        assert_eq!(strip_skill_frontmatter(input), "# Body\nContent here");
    }

    #[test]
    fn strip_yaml_frontmatter_no_body() {
        let input = "---\nname: foo\n---";
        assert_eq!(strip_skill_frontmatter(input), "");
    }

    #[test]
    fn strip_yaml_frontmatter_trailing_newline_only() {
        let input = "---\nname: foo\n---\n";
        assert_eq!(strip_skill_frontmatter(input), "");
    }

    #[test]
    fn strip_toml_frontmatter_normal() {
        let input = "+++\nname = \"foo\"\n+++\n# Body";
        assert_eq!(strip_skill_frontmatter(input), "# Body");
    }

    #[test]
    fn strip_no_frontmatter() {
        let input = "# Just a heading\nSome content";
        assert_eq!(
            strip_skill_frontmatter(input),
            "# Just a heading\nSome content"
        );
    }

    #[test]
    fn strip_empty_string() {
        assert_eq!(strip_skill_frontmatter(""), "");
    }

    #[test]
    fn strip_frontmatter_with_blank_lines_before_body() {
        let input = "---\nname: foo\n---\n\n\n# Body";
        assert_eq!(strip_skill_frontmatter(input), "# Body");
    }

    #[test]
    fn enabled_tool_set_normalizes_legacy_aliases() {
        let enabled = enabled_tool_set("spawn_agent,save_memory,read_entity,temper_file_upload");
        assert!(enabled.contains("temper_spawn_session"));
        assert!(enabled.contains("temper_save_memory"));
        assert!(enabled.contains("temper_get"));
        assert!(enabled.contains("temper_write"));
    }

    #[test]
    fn build_tool_definitions_hides_disabled_methods() {
        let tools = build_tool_definitions("temper_get,temper_list", "", "/workspace");
        let description = tools[0]
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("");

        assert!(description.contains("temper.get(entity_set, entity_id)"));
        assert!(description.contains("temper.list(entity_set, filter_str)"));
        assert!(!description.contains("temper.submit_specs(files_dict)"));
        assert!(!description.contains("sandbox.bash(command)"));
    }

    #[test]
    fn build_tool_definitions_explains_submit_specs_requirements() {
        let tools = build_tool_definitions("temper_submit_specs", "", "/workspace");
        let description = tools[0]
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("");

        assert!(description.contains("temper.submit_specs(files_dict)"));
        assert!(description.contains("model.csdl.xml"));
        assert!(description.contains("nested paths allowed"));
    }

    #[test]
    fn build_tool_definitions_keeps_always_available_methods() {
        let tools = build_tool_definitions("", "", "/workspace");
        let description = tools[0]
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("");

        assert!(description.contains("temper.done(result)"));
        assert!(description.contains("temper.switch_mode({\"mode\": \"plan\"})"));
        assert!(description.contains("temper.get_session_id()"));
    }

    #[test]
    fn build_sdk_reference_mentions_lazy_sandbox_when_enabled() {
        let sdk = build_sdk_reference("bash", "", "/workspace");
        assert!(sdk.contains("sandbox` (remote shell/files, provisioned on demand"));
    }
}
