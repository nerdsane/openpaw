//! Context Compactor — WASM module for compacting long agent conversations.
//!
//! When the session tree exceeds the context window (minus reserve_tokens),
//! this module is triggered. It summarizes older messages using an LLM call
//! and replaces them with a compaction entry in the session tree.
//!
//! Build: `cargo build --target wasm32-unknown-unknown --release`

use session_tree_lib::SessionTree;
use temper_wasm_sdk::prelude::*;
use wasm_helpers::{
    create_content_file, read_content_file, read_session_from_temperfs, resolve_temper_api_url,
    write_session_to_temperfs,
};

/// Entry point.
#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        ctx.log("info", "context_compactor: starting");

        let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));

        // Read compaction parameters
        let keep_recent_tokens: usize = fields
            .get("keep_recent_tokens")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse().ok())
            .unwrap_or(10000);

        let session_file_id = fields
            .get("session_file_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let session_leaf_id = fields
            .get("session_leaf_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if session_file_id.is_empty() || session_leaf_id.is_empty() {
            return Err("context_compactor: missing session_file_id or session_leaf_id".to_string());
        }

        let temper_api_url = resolve_temper_api_url(&ctx, &fields);
        let tenant = &ctx.tenant;
        let workspace_id = fields
            .get("workspace_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // 1. Read session tree from TemperFS
        let session_jsonl =
            read_session_from_temperfs(&ctx, &temper_api_url, tenant, &fields, session_file_id)?;
        let mut tree = SessionTree::from_jsonl(&session_jsonl);

        ctx.log("info", &format!(
            "context_compactor: tree has {} entries, estimating tokens from leaf {}",
            tree.len(), session_leaf_id
        ));

        // 2. Find cut point
        let cut_point = match tree.find_cut_point(session_leaf_id, keep_recent_tokens) {
            Some(cp) => cp,
            None => {
                ctx.log("warn", "context_compactor: no valid cut point found, skipping compaction");
                set_success_result("CompactionComplete", &json!({
                    "session_leaf_id": session_leaf_id,
                    "context_tokens": tree.estimate_tokens(session_leaf_id),
                }));
                return Ok(());
            }
        };

        ctx.log("info", &format!("context_compactor: cut point at entry {}", cut_point));

        // 3. Build compaction prompt from messages being cut
        let context_refs = tree.build_context_refs(&cut_point);
        let messages_to_summarize =
            resolve_context_refs_for_compaction(&ctx, &temper_api_url, tenant, &context_refs);
        if messages_to_summarize.is_empty() {
            ctx.log("warn", "context_compactor: no messages to summarize");
            set_success_result("CompactionComplete", &json!({
                "session_leaf_id": session_leaf_id,
                "context_tokens": tree.estimate_tokens(session_leaf_id),
            }));
            return Ok(());
        }

        let conversation_text = format_messages_for_summary(&messages_to_summarize);

        // 4. Call LLM for structured summary
        let configured_provider = fields
            .get("provider")
            .and_then(|v| v.as_str())
            .unwrap_or("anthropic");

        // Resolve provider and API key with auto-fallback
        let (provider, api_key) = resolve_compaction_provider(&ctx, configured_provider);

        let compaction_model = fields
            .get("compaction_model")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| {
                fields.get("model").and_then(|v| v.as_str()).unwrap_or_else(|| {
                    match provider.as_str() {
                        "openai" | "openai_codex" => "gpt-5.4",
                        "openrouter" => "anthropic/claude-sonnet-4.6",
                        _ => "claude-sonnet-4-6",
                    }
                })
            });

        let summary = if provider == "mock" || api_key.trim().is_empty() {
            build_mock_summary(&conversation_text)
        } else {
            call_compaction_llm(&ctx, &provider, &api_key, compaction_model, &conversation_text)?
        };

        ctx.log("info", &format!(
            "context_compactor: generated summary ({} chars)",
            summary.len()
        ));

        // 5. Append compaction entry to session tree
        let summary_tokens = estimate_summary_tokens(&summary);

        let (compaction_id, _line) = if !workspace_id.is_empty() {
            match create_content_file(
                &ctx,
                &temper_api_url,
                tenant,
                workspace_id,
                &format!("compaction-{}.txt", tree.len()),
                &summary,
            ) {
                Ok(summary_file_id) => {
                    tree.append_compaction_file(
                        session_leaf_id,
                        &summary_file_id,
                        &cut_point,
                        summary_tokens,
                    )
                }
                Err(_) => tree.append_compaction(session_leaf_id, &summary, &cut_point),
            }
        } else {
            tree.append_compaction(session_leaf_id, &summary, &cut_point)
        };

        // 6. Write updated session tree back to TemperFS
        let updated_jsonl = tree.to_jsonl();
        write_session_to_temperfs(
            &ctx,
            &temper_api_url,
            tenant,
            &fields,
            session_file_id,
            &updated_jsonl,
        )?;

        // 7. Return CompactionComplete with new leaf pointing after compaction
        let new_token_estimate = tree.estimate_tokens(&compaction_id);
        set_success_result("CompactionComplete", &json!({
            "session_leaf_id": compaction_id,
            "context_tokens": new_token_estimate,
        }));

        Ok(())
    })();

    if let Err(e) = result {
        set_error_result(&e);
    }
    0
}

fn resolve_context_refs_for_compaction(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    refs: &[session_tree_lib::ContextRef],
) -> Vec<Value> {
    let mut messages = Vec::new();

    for ctx_ref in refs {
        match ctx_ref.entry_type {
            session_tree_lib::EntryType::Compaction => {
                let summary = if let Some(ref file_id) = ctx_ref.content_file_id {
                    read_content_file(ctx, temper_api_url, tenant, &json!({}), file_id)
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
            session_tree_lib::EntryType::Message | session_tree_lib::EntryType::Steering => {
                if let Some(ref file_id) = ctx_ref.content_file_id {
                    if let Ok(raw) =
                        read_content_file(ctx, temper_api_url, tenant, &json!({}), file_id)
                    {
                        if !raw.is_empty() {
                            let content: Value = serde_json::from_str(&raw).unwrap_or(json!(raw));
                            messages.push(json!({
                                "role": ctx_ref.role,
                                "content": content,
                            }));
                            continue;
                        }
                    }
                }
                if let Some(ref inline) = ctx_ref.inline_content {
                    messages.push(json!({
                        "role": ctx_ref.role,
                        "content": inline.clone(),
                    }));
                }
            }
            session_tree_lib::EntryType::Header => {}
        }
    }

    messages
}

fn estimate_summary_tokens(summary: &str) -> usize {
    let trimmed = summary.trim();
    if trimmed.is_empty() {
        0
    } else {
        usize::max(1, trimmed.len().div_ceil(4))
    }
}

fn build_mock_summary(conversation_text: &str) -> String {
    let truncated: String = conversation_text.chars().take(600).collect();
    format!(
        "## Active Goal\nContinue the active task.\n\n## Episodes\n\n### Episode: Prior conversation\n- **Goal:** Earlier conversation context\n- **Worked:** Conversation compacted using mock path\n- **Failed:** None recorded\n- **Discoveries:** No real model configured for compaction\n- **Artifacts:** None\n\n## Current State\n- **Where we are:** Resuming after compaction\n- **Next:** Continue the active task\n- **Open questions:** None\n\n---\nRaw context tail:\n{}",
        truncated
    )
}

/// Format messages into a text block for the compaction LLM prompt.
fn format_messages_for_summary(messages: &[Value]) -> String {
    let mut text = String::new();
    for msg in messages {
        let role = msg.get("role").and_then(|v| v.as_str()).unwrap_or("unknown");
        let content = msg.get("content").cloned().unwrap_or(json!(""));
        let content_str = match content {
            Value::String(s) => s,
            Value::Array(arr) => {
                arr.iter()
                    .filter_map(|block| {
                        if block.get("type").and_then(|v| v.as_str()) == Some("text") {
                            block.get("text").and_then(|v| v.as_str()).map(String::from)
                        } else if block.get("type").and_then(|v| v.as_str()) == Some("tool_use") {
                            Some(format!("[tool_use: {}]", block.get("name").and_then(|v| v.as_str()).unwrap_or("unknown")))
                        } else if block.get("type").and_then(|v| v.as_str()) == Some("tool_result") {
                            let content = block.get("content").and_then(|v| v.as_str()).unwrap_or("...");
                            let truncated = if content.len() > 200 { &content[..200] } else { content };
                            Some(format!("[tool_result: {}]", truncated))
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            }
            _ => serde_json::to_string(&content).unwrap_or_default(),
        };
        text.push_str(&format!("## {role}\n{content_str}\n\n"));
    }
    text
}

/// Check if a value is an unresolved secret template like `{secret:key_name}`.
fn is_unresolved_secret_template(value: &str) -> bool {
    value.contains("{secret:")
}

/// Resolve the best available provider and API key, with auto-fallback.
fn resolve_compaction_provider(ctx: &Context, configured_provider: &str) -> (String, String) {
    let provider_keys: &[(&str, &[&str])] = &[
        ("anthropic", &["anthropic_api_key", "api_key"]),
        ("openai", &["openai_codex_token"]),
        ("openrouter", &["openrouter_api_key"]),
    ];

    // Try configured provider first
    for &(prov, keys) in provider_keys {
        if prov != configured_provider { continue; }
        for &key_name in keys {
            if let Some(val) = ctx.config.get(key_name) {
                if !val.trim().is_empty() && !is_unresolved_secret_template(val) {
                    return (prov.to_string(), val.clone());
                }
            }
        }
    }

    // Fallback: try any provider with a valid key
    ctx.log("warn", &format!(
        "context_compactor: provider={configured_provider} has no valid key, trying alternatives"
    ));
    for &(prov, keys) in provider_keys {
        if prov == configured_provider { continue; }
        for &key_name in keys {
            if let Some(val) = ctx.config.get(key_name) {
                if !val.trim().is_empty() && !is_unresolved_secret_template(val) {
                    ctx.log("warn", &format!(
                        "context_compactor: falling back to provider={prov}"
                    ));
                    return (prov.to_string(), val.clone());
                }
            }
        }
    }

    // No valid key found — return configured provider with empty key (will use mock path)
    (configured_provider.to_string(), String::new())
}

/// Call the LLM with a compaction-specific system prompt.
fn call_compaction_llm(
    ctx: &Context,
    provider: &str,
    api_key: &str,
    model: &str,
    conversation_text: &str,
) -> Result<String, String> {
    let system_prompt = "You are a conversation compactor. Extract distinct episodes from this conversation — each a coherent task or sub-task the agent attempted. Be concise but preserve the trajectory: what was tried, what worked, what failed, and why.\n\nOutput the summary in this exact format:\n\n## Active Goal\n<the current overarching objective>\n\n## Episodes\n\n### Episode: <short title>\n- **Goal:** <what was attempted>\n- **Worked:** <actions that succeeded and why>\n- **Failed:** <approaches tried and abandoned, what went wrong>\n- **Discoveries:** <facts learned, decisions made>\n- **Artifacts:** <files changed, entities created, useful outputs>\n\n(Repeat chronologically for each distinct episode)\n\n## Current State\n- **Where we are:** <what just completed or is in progress>\n- **Next:** <immediate next steps>\n- **Open questions:** <unresolved issues>\n\nIMPORTANT: Preserve the trajectory. A future model reading this needs to know which approaches were already tried and failed, not just what worked. This prevents repeating failed approaches.";

    let (url, headers, body_str) = match provider {
        "openai" | "openai_codex" => {
            let body = json!({
                "model": model,
                "instructions": system_prompt,
                "input": format!("Summarize this conversation:\n\n{conversation_text}")
            });
            let headers = vec![
                ("authorization".to_string(), format!("Bearer {api_key}")),
                ("content-type".to_string(), "application/json".to_string()),
            ];
            let body_str = serde_json::to_string(&body).map_err(|e| format!("JSON serialize error: {e}"))?;
            ("https://api.openai.com/v1/responses".to_string(), headers, body_str)
        }
        "openrouter" => {
            let body = json!({
                "model": model,
                "max_tokens": 2048,
                "messages": [
                    { "role": "system", "content": system_prompt },
                    { "role": "user", "content": format!("Summarize this conversation:\n\n{conversation_text}") }
                ]
            });
            let headers = vec![
                ("authorization".to_string(), format!("Bearer {api_key}")),
                ("content-type".to_string(), "application/json".to_string()),
            ];
            let body_str = serde_json::to_string(&body).map_err(|e| format!("JSON serialize error: {e}"))?;
            ("https://openrouter.ai/api/v1/chat/completions".to_string(), headers, body_str)
        }
        _ => {
            // Anthropic (default)
            let body = json!({
                "model": model,
                "max_tokens": 2048,
                "system": system_prompt,
                "messages": [{
                    "role": "user",
                    "content": format!("Summarize this conversation:\n\n{conversation_text}")
                }]
            });
            let is_oauth = api_key.contains("sk-ant-oat");
            let headers = if is_oauth {
                vec![
                    ("authorization".to_string(), format!("Bearer {api_key}")),
                    ("anthropic-version".to_string(), "2023-06-01".to_string()),
                    ("anthropic-beta".to_string(), "oauth-2025-04-20".to_string()),
                    ("content-type".to_string(), "application/json".to_string()),
                ]
            } else {
                vec![
                    ("x-api-key".to_string(), api_key.to_string()),
                    ("anthropic-version".to_string(), "2023-06-01".to_string()),
                    ("content-type".to_string(), "application/json".to_string()),
                ]
            };
            let body_str = serde_json::to_string(&body).map_err(|e| format!("JSON serialize error: {e}"))?;
            ("https://api.anthropic.com/v1/messages".to_string(), headers, body_str)
        }
    };

    ctx.log("info", &format!("context_compactor: calling {provider} at {url} with model={model}"));
    let resp = ctx.http_call("POST", &url, &headers, &body_str)?;
    if resp.status != 200 {
        return Err(format!(
            "Compaction LLM call failed (HTTP {}): {}",
            resp.status,
            &resp.body[..resp.body.len().min(500)]
        ));
    }

    let parsed: Value = serde_json::from_str(&resp.body)
        .map_err(|e| format!("failed to parse compaction LLM response: {e}"))?;

    // Extract text — format varies by provider
    let text = match provider {
        "openai" => {
            // OpenAI responses API
            parsed.get("output")
                .and_then(|v| v.as_array())
                .and_then(|arr| arr.iter().find(|b| b.get("type").and_then(|v| v.as_str()) == Some("message")))
                .and_then(|b| b.get("content"))
                .and_then(|v| v.as_array())
                .and_then(|arr| arr.first())
                .and_then(|b| b.get("text").and_then(|v| v.as_str()))
                .unwrap_or("Summary unavailable")
                .to_string()
        }
        "openrouter" => {
            // OpenRouter chat completions
            parsed.get("choices")
                .and_then(|v| v.as_array())
                .and_then(|arr| arr.first())
                .and_then(|c| c.get("message"))
                .and_then(|m| m.get("content"))
                .and_then(|v| v.as_str())
                .unwrap_or("Summary unavailable")
                .to_string()
        }
        _ => {
            // Anthropic messages
            parsed
                .get("content")
                .and_then(|v| v.as_array())
                .and_then(|arr| arr.iter().find(|b| b.get("type").and_then(|v| v.as_str()) == Some("text")))
                .and_then(|b| b.get("text").and_then(|v| v.as_str()))
                .unwrap_or("Summary unavailable")
                .to_string()
        }
    };

    Ok(text)
}
