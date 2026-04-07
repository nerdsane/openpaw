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
        let tools_enabled = fields
            .get("tools_enabled")
            .and_then(|v| v.as_str())
            .unwrap_or("read,write,edit,bash");
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
        let api_key = if provider == "mock" {
            String::new()
        } else {
            resolve_provider_api_key(&ctx, &provider)?
        };
        if provider != "mock" && is_unresolved_secret_template(&api_key) {
            return Err(format!(
                "provider={provider} api key is unresolved secret template: '{api_key}'. \
set tenant secret and retry"
            ));
        }
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
        messages = repair_interrupted_tool_use_messages(messages);

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
                    set_success_result(
                        "NeedsCompaction",
                        &json!({
                            "context_tokens": context_tokens,
                            "session_leaf_id": session_leaf_id,
                        }),
                    );
                    return Ok(());
                }
            }
        }

        // System prompt assembly (Pi architecture):
        // 1. Soul content (from Soul entity via TemperFS)
        // 2. system_prompt override (from Configure action)
        // 3. Available skills XML block
        // 4. Memory context
        let assembled_system_prompt =
            assemble_system_prompt(&ctx, &temper_api_url, tenant, soul_id, system_prompt)?;

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

        // Send typing indicator to Discord before LLM call
        send_typing_indicator(&ctx, &temper_api_url, tenant, &ctx.entity_id);

        // Call LLM API
        let response = match provider.as_str() {
            "mock" => call_mock(&ctx, &messages, &assembled_system_prompt, &tools)?,
            "anthropic" => call_anthropic(
                &ctx,
                &api_key,
                &anthropic_api_url,
                model,
                &assembled_system_prompt,
                &messages,
                &tools,
                &anthropic_auth_mode,
            )?,
            "openrouter" => call_openrouter(
                &ctx,
                &api_key,
                &openrouter_api_url,
                model,
                &assembled_system_prompt,
                &messages,
                &tools,
                &openrouter_site_url,
                &openrouter_app_name,
            )?,
            "openai" => call_openai(
                &ctx,
                &api_key,
                &openai_api_url,
                model,
                &assembled_system_prompt,
                &messages,
                &tools,
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
                        let (leaf, _) =
                            if !workspace_id.is_empty()
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
                        let (new_leaf, _) =
                            if !workspace_id.is_empty()
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

                        // Route through steering check if follow-ups are enabled
                        if max_follow_ups > 0 {
                            set_success_result(
                                "CheckSteering",
                                &json!({
                                    "result": result_text,
                                    "session_leaf_id": new_leaf,
                                    "input_tokens": response.input_tokens,
                                    "output_tokens": response.output_tokens,
                                }),
                            );
                        } else {
                            let params = json!({
                                "result": result_text,
                                "session_leaf_id": new_leaf,
                                "input_tokens": response.input_tokens,
                                "output_tokens": response.output_tokens,
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
}

fn normalize_provider(provider: &str) -> String {
    let norm = provider.trim().to_ascii_lowercase();
    if norm == "open_router" {
        "openrouter".to_string()
    } else {
        norm
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
            ctx.config.get("openai_codex_token").cloned(),
            ctx.config.get("api_key").cloned(),
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
    });

    if !effective_system.is_empty() {
        body["system"] = json!(effective_system);
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
                "oauth-2025-04-20,computer-use-2025-01-24".to_string(),
            ),
            ("content-type".to_string(), "application/json".to_string()),
            ("user-agent".to_string(), "claude-cli/2.1.75".to_string()),
            ("x-app".to_string(), "cli".to_string()),
        ]
    } else {
        vec![
            ("x-api-key".to_string(), api_key.to_string()),
            ("anthropic-version".to_string(), "2023-06-01".to_string()),
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

    ctx.log(
        "info",
        &format!("llm_caller: usage: input={input_tokens}, output={output_tokens}"),
    );

    Ok(LlmResponse {
        content,
        stop_reason,
        input_tokens,
        output_tokens,
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
) -> Result<LlmResponse, String> {
    // Convert Anthropic-format messages to Responses API input format
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
                            let call_id = block.get("tool_use_id").and_then(Value::as_str).unwrap_or("");
                            let output = if let Some(inner) = block.get("content").and_then(Value::as_array) {
                                inner.iter()
                                    .filter_map(|b| b.get("text").and_then(Value::as_str))
                                    .collect::<Vec<_>>()
                                    .join("\n")
                            } else if let Some(text) = block.get("content").and_then(Value::as_str) {
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
                        let text: String = blocks.iter()
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
                                let call_id = block.get("id").and_then(Value::as_str).unwrap_or("").to_string();
                                let name = block.get("name").and_then(Value::as_str).unwrap_or("").to_string();
                                let arguments = serde_json::to_string(
                                    block.get("input").unwrap_or(&json!({}))
                                ).unwrap_or_else(|_| "{}".to_string());
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
                    blocks.iter()
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
    let codex_tools: Vec<Value> = tools.iter().map(|t| {
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
    }).collect();

    let mut body = json!({
        "model": model,
        "instructions": system_prompt,
        "input": input,
        "stream": true,
        "store": false,
    });
    if !codex_tools.is_empty() {
        body["tools"] = json!(codex_tools);
        // Force tool use — without this, GPT-5 writes text analysis
        // instead of calling the execute tool
        body["tool_choice"] = json!("required");
    }

    let body_str = serde_json::to_string(&body)
        .map_err(|e| format!("JSON serialize error: {e}"))?;

    let headers = vec![
        ("authorization".to_string(), format!("Bearer {api_key}")),
        ("content-type".to_string(), "application/json".to_string()),
    ];

    // Log input types for debugging conversation format issues
    let input_types: Vec<String> = input.iter().map(|i| {
        let t = i.get("type").and_then(Value::as_str)
            .or_else(|| i.get("role").and_then(Value::as_str))
            .unwrap_or("?");
        t.to_string()
    }).collect();
    ctx.log(
        "info",
        &format!(
            "llm_caller: calling OpenAI API, model={model}, input={}, types={:?}, url={api_url}",
            input.len(), input_types,
        ),
    );

    let mut last_err = String::new();
    let mut resp = None;
    for attempt in 0..5 {
        if attempt > 0 {
            ctx.log("warn", &format!("llm_caller: OpenAI Codex retry {}/{}", attempt + 1, 5));
        }
        match ctx.http_call("POST", api_url, &headers, &body_str) {
            Ok(r) if r.status >= 200 && r.status < 300 => {
                resp = Some(r);
                break;
            }
            Ok(r) if r.status == 429 => {
                last_err = format!("OpenAI Codex API rate limited (429)");
                continue;
            }
            Ok(r) => {
                let snippet = &r.body[..r.body.len().min(300)];
                return Err(format!("OpenAI Codex API returned {}: {snippet}", r.status));
            }
            Err(e) => {
                last_err = e;
                continue;
            }
        }
    }
    let resp = resp.ok_or_else(|| format!("OpenAI Codex API failed after 5 attempts: {last_err}"))?;

    // Parse SSE data payloads (newline-separated JSON lines from host).
    // The Codex endpoint streams individual events — output_item.done events
    // contain the actual tool calls and messages. response.completed may have
    // empty output (Codex strips it for bandwidth). So we accumulate output
    // items from output_item.done events and usage from response.completed.
    let body = &resp.body;
    let mut output_items = Vec::<Value>::new();
    let mut usage = json!({});

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
                "response.completed" => {
                    if let Some(resp) = event.get("response") {
                        if let Some(u) = resp.get("usage") {
                            usage = u.clone();
                        }
                        // If response has non-empty output, use it (standard API behavior)
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
        return Err(format!(
            "OpenAI: no output items found in {} lines ({}B)",
            body.lines().count(),
            body.len()
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
                    let call_id = item.get("call_id").and_then(Value::as_str).unwrap_or("").to_string();
                    let name = item.get("name").and_then(Value::as_str).unwrap_or("").to_string();
                    let arguments = item.get("arguments").and_then(Value::as_str).unwrap_or("{}");
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
    let input_tokens = usage.get("input_tokens").and_then(|v| v.as_i64()).unwrap_or(0);
    let output_tokens = usage.get("output_tokens").and_then(|v| v.as_i64()).unwrap_or(0);

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

fn repair_interrupted_tool_use_messages(messages: Vec<Value>) -> Vec<Value> {
    let mut repaired = Vec::new();

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

fn mock_text_response(messages: &[Value], text: String) -> LlmResponse {
    LlmResponse {
        content: json!([{ "type": "text", "text": text.clone() }]),
        stop_reason: "end_turn".to_string(),
        input_tokens: estimate_message_tokens(messages),
        output_tokens: text.len() as i64,
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
fn build_tool_definitions(_tools_enabled: &str, _sandbox_url: &str, _workdir: &str) -> Vec<Value> {
    // Single execute tool — method listing here (not in system prompt) so agents see it when choosing the tool.
    return vec![json!({
        "name": "execute",
        "description": concat!(
            "Execute Python code in the Temper REPL. Variables persist across calls.\n\n",
            "Available methods:\n",
            "- sandbox.bash(command) → run shell command, returns stdout\n",
            "- sandbox.read(path) → read file content\n",
            "- sandbox.write(path, content) → write file\n",
            "- sandbox.edit(path, old, new) → search-replace in file\n",
            "- temper.create(entity_set, fields_dict) → create entity, returns dict with entity_id\n",
            "- temper.get(entity_set, entity_id) → get entity by id\n",
            "- temper.list(entity_set, filter_str) → list entities with OData $filter\n",
            "- temper.action(entity_set, entity_id, action_name, params_dict) → dispatch action\n",
            "- temper.patch(entity_set, entity_id, fields_dict) → partial update\n",
            "- temper.spawn_session(task, soul_id=None, model=None, tools=None, workdir=None, sandbox_url=None, max_turns=None, background=False) → spawn sub-session\n",
            "- temper.list_sessions(filter=None, top=50) → list sessions\n",
            "- temper.abort_session(session_id) → cancel session\n",
            "- temper.steer_session(session_id, message) → inject message\n",
            "- temper.save_memory(key, content, memory_type='project') → persist memory\n",
            "- temper.recall_memory(query) → search memories, returns list\n",
            "- temper.write(path, content, opts?) → write file by path (auto-creates workspace/dirs), returns {file_id, path, workspace_id}\n",
            "- temper.read(path, opts?) → read file content by path\n",
            "- temper.run_coding_agent(agent_type, task) → spawn coding session\n",
            "- temper.submit_specs(files_dict) → load specs into Temper\n",
            "- temper.show_spec(entity_name) → inspect entity spec\n",
            "- temper.install_app(app_name, reason, payload=None, capability_type='os_app') → request capability install\n",
            "- temper.upload_wasm(module_name, wasm_base64) → upload WASM module\n",
            "- temper.get_secret(key) → read secret from vault (Cedar-gated)\n",
            "- temper.switch_provider(model=None, provider=None) → change LLM provider/model mid-session (takes effect on next turn)\n",
            "- temper.done(result) → signal session completion with result\n",
            "- temper.submit_policy(policy_id, cedar_text) → create Cedar policy (Cedar-gated)\n",
            "- temper.list_policies() → list all Cedar policies\n",
            "- temper.get_policy(policy_id) → read a specific Cedar policy\n",
            "- temper.update_policy(policy_id, cedar_text) → update Cedar policy (Cedar-gated)\n",
            "- temper.delete_policy(policy_id) → delete Cedar policy (Cedar-gated)\n",
            "- temper.get_trajectories(entity_type, include_actions, limit=10) → evolution data\n",
            "- temper.get_insights() → evolution insights\n",
            "- temper.get_decisions() → pending governance decisions\n",
            "- temper.poll_decision(decision_id) → wait for decision\n",
            "- temper.approve_decision(decision_id, scope_dict) → approve governance decision (Cedar-gated)\n",
            "- temper.deny_decision(decision_id) → deny governance decision (Cedar-gated)\n",
            "- temper.datadog_query(query_kind, monitor_id=None, query=None, ...) → Datadog API\n",
            "- temper.railway(action, project_id=None, ...) → Railway API\n",
            "- temper.vercel(action, deployment_id=None, ...) → Vercel API\n",
            "- temper.web_search(query) → search the web via Exa, returns list of {title, url, text}\n",
            "- temper.web_fetch(url) → fetch a URL, returns text content (HTML tags stripped)\n\n",
            "IMPORTANT: No pip packages available (no requests, httpx, subprocess, os). ",
            "Use sandbox.bash() for ALL shell commands. Write complete multi-step scripts, not one-liners."
        ),
        "input_schema": {
            "type": "object",
            "properties": {
                "code": { "type": "string", "description": "Python code to execute" }
            },
            "required": ["code"]
        }
    })];
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

    write_temperfs_value_with_retry(
        ctx,
        &url,
        &headers,
        &body,
        "TemperFS $value write failed",
    )?;
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
    write_temperfs_value_with_retry(
        ctx,
        &url,
        &headers,
        jsonl,
        "TemperFS session write failed",
    )
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
    let url = format!(
        "{temper_api_url}/tdata/Harnesses('{project_harness_id}')"
    );
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
            .and_then(|f| f.get("project_harness_id").or_else(|| f.get("ProjectHarnessId")))
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

    // 3. Available skills (filtered by scope: global + soul-specific + agent-specific)
    {
        let agent_id = ctx
            .entity_state
            .get("fields")
            .and_then(|f| f.get("agent_id").or_else(|| f.get("AgentId")))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let agent_name = if !agent_id.is_empty() {
            resolve_agent_name(ctx, temper_api_url, tenant, agent_id).unwrap_or_default()
        } else {
            String::new()
        };
        match load_skills_block(ctx, temper_api_url, tenant, soul_id, &agent_name) {
            Ok(block) if !block.is_empty() => parts.push(block),
            Ok(_) => {}
            Err(e) => ctx.log(
                "warn",
                &format!("assemble_system_prompt: failed to load skills: {e}"),
            ),
        }
    }

    // 4. Memory context — scoped to agent, not soul (ADR-0007)
    {
        let entity_id = ctx.entity_state.get("entity_id").and_then(|v| v.as_str()).unwrap_or("");
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
            .unwrap_or("read,write,edit,bash");
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
    let enabled: Vec<&str> = tools_enabled.split(',').map(str::trim).collect();
    let has_sandbox = !sandbox_url.is_empty()
        && (enabled.contains(&"read")
            || enabled.contains(&"write")
            || enabled.contains(&"edit")
            || enabled.contains(&"bash"));

    let mut sections = Vec::new();

    sections.push(format!(
        "<temper_sdk>\n\
         ## Execution Environment\n\n\
         Your `execute` tool runs Python in a sandboxed REPL.{sandbox_note}\n\n\
         Constraints:\n\
         - No pip packages (no requests, httpx, numpy, pandas, etc.)\n\
         - No network access from Python — use sandbox.bash(\"curl ...\") for HTTP\n\
         - No filesystem access from Python — use sandbox.read/write/edit\n\
         - Variables persist across execute calls within the same session\n\
         - Write substantial code blocks, not one-liners\n\
         - Sandbox working directory: {workdir}",
        sandbox_note = if has_sandbox {
            " Two objects available: `temper` (platform API) and `sandbox` (remote shell/files)."
        } else {
            " One object available: `temper` (platform API)."
        },
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
         Write complete workflows in a single execute call when possible.\n\
         BAD: 5 separate execute calls for 5 one-line operations\n\
         GOOD: 1 execute call with a multi-line script doing all 5 operations\n\n\
         Each execute call is an LLM turn. Fewer turns = faster completion."
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
    let content_file_id = entity_field_str(&soul, &["ContentFileId"]).unwrap_or("");
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
        format!("{temper_api_url}/tdata/Souls?$filter=Name eq '{escaped}' and Status eq 'Active'");
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

/// Load active skills as an XML block for the system prompt.
///
/// Skills are loaded for ALL agents (not gated on soul_id). Scope filtering
/// includes global skills, plus skills scoped to the soul name or agent name.
fn load_skills_block(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    soul_id: &str,
    agent_name: &str,
) -> Result<String, String> {
    let headers = agent_headers(ctx, tenant, None, Some("application/json"));

    // Resolve the soul name for scope matching
    let soul_name = if !soul_id.is_empty() {
        match resolve_soul_entity(ctx, temper_api_url, tenant, soul_id) {
            Ok(soul) => entity_field_str(&soul, &["Name"])
                .unwrap_or(soul_id)
                .to_string(),
            Err(_) => soul_id.to_string(),
        }
    } else {
        String::new()
    };

    // Build scope filter: global + soul name + agent name
    let mut scope_parts = vec!["Scope eq 'global'".to_string()];
    if !soul_name.is_empty() {
        let escaped = soul_name.replace('\'', "''");
        scope_parts.push(format!("Scope eq '{escaped}'"));
    }
    if !agent_name.is_empty() {
        let escaped = agent_name.replace('\'', "''");
        scope_parts.push(format!("Scope eq '{escaped}'"));
    }
    let filter = format!(
        "Status eq 'Active' and ({})",
        scope_parts.join(" or ")
    );
    let url = format!("{temper_api_url}/tdata/Skills?$filter={filter}");
    let resp = ctx.http_call("GET", &url, &headers, "")?;

    // If parenthesized OR isn't supported, fall back to separate queries merged client-side
    let skills = if resp.status == 200 {
        let parsed: Value = serde_json::from_str(&resp.body).unwrap_or(json!({}));
        parsed
            .get("value")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default()
    } else {
        // Fallback: separate queries merged client-side
        let mut merged = Vec::new();
        let mut seen_ids = BTreeSet::new();
        for scope_filter in &scope_parts {
            let fallback_url = format!(
                "{temper_api_url}/tdata/Skills?$filter=Status eq 'Active' and {scope_filter}"
            );
            if let Ok(r) = ctx.http_call("GET", &fallback_url, &headers, "") {
                if r.status == 200 {
                    if let Ok(p) = serde_json::from_str::<Value>(&r.body) {
                        if let Some(arr) = p.get("value").and_then(|v| v.as_array()) {
                            for item in arr {
                                let id = entity_field_str(item, &["Id", "entity_id"])
                                    .unwrap_or("")
                                    .to_string();
                                if seen_ids.insert(id) {
                                    merged.push(item.clone());
                                }
                            }
                        }
                    }
                }
            }
        }
        merged
    };

    if skills.is_empty() {
        return Ok(String::new());
    }
    let mut xml = String::from("<available_skills>\n");
    for skill in &skills {
        let name = entity_field_str(skill, &["Name"]).unwrap_or("unknown");
        let desc = entity_field_str(skill, &["Description"]).unwrap_or("");
        let file_id = entity_field_str(skill, &["ContentFileId"]).unwrap_or("");
        xml.push_str(&format!(
            "  <skill name=\"{name}\" description=\"{desc}\" file_id=\"{file_id}\" />\n"
        ));
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
    let file_id = entity_field_str(&agent, &["InstructionsFileId", "instructions_file_id"])
        .unwrap_or("");
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

/// Resolve an Agent entity's name by ID.
fn resolve_agent_name(
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
    Ok(entity_field_str(&agent, &["Name", "name"])
        .unwrap_or("")
        .to_string())
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
    create_content_file(ctx, temper_api_url, tenant, workspace_id, &file_name, content)
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
