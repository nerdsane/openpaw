//! Context Preparer — staged Session-turn WASM for bounded context assembly.
//!
//! Owns the `PreparingContext` phase:
//! - load conversation or session-tree context
//! - repair interrupted tool-use history
//! - prune stale tool results
//! - assemble and cache the system prompt
//! - compute byte/token budgets
//! - write the prepared-context artifact
//! - route to `ContextReady` or `NeedsCompaction`
//!
//! Build: `cargo build --target wasm32-unknown-unknown --release`

use session_tree_lib::{ContextRef, EntryType, SessionTree};
use session_turn_artifacts::{PreparedContextArtifact, parse_prepared_context_artifact};
use std::collections::{BTreeMap, BTreeSet};
use temper_wasm_sdk::prelude::*;
use tool_catalog::{
    DEFAULT_TOOLS_ENABLED, build_method_listing, enabled_tool_set, has_sandbox_surface,
};
use wasm_helpers::{
    create_content_file, read_text_file_versions_batch, read_text_files_batch, runtime_headers,
    runtime_headers_as, timestamp_millis_string, write_temperfs_value_with_retry,
};

const DEFAULT_CONTEXT_PREPARE_BUDGET_MS: i64 = 120_000;

#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    if let Err(err) = run_context_preparer() {
        set_error_result(&err);
    }
    0
}

fn normalize_provider(provider: &str) -> String {
    let norm = provider.trim().to_ascii_lowercase();
    match norm.as_str() {
        "open_router" => "openrouter".to_string(),
        "codex" | "openai-codex" => "openai_codex".to_string(),
        _ => norm,
    }
}

#[derive(Debug, PartialEq)]
enum PreparedContextReuse {
    Reused {
        messages: Vec<Value>,
        entries_loaded: usize,
        content_files_loaded: usize,
        delta_entries_loaded: usize,
        delta_content_files_loaded: usize,
    },
    RebuildRequired {
        reason: &'static str,
    },
}

fn stringify_content(value: &Value) -> String {
    if let Some(s) = value.as_str() {
        s.to_string()
    } else {
        value.to_string()
    }
}

#[allow(dead_code)]
fn emit_progress_ignore(ctx: &Context, payload: Value) {
    let _ = (ctx, payload);
}

fn send_progress(ctx: &Context, temper_api_url: &str, tenant: &str) -> Result<(), String> {
    let url = format!(
        "{temper_api_url}/tdata/Sessions('{}')/TemperPaw.ProgressMade",
        ctx.entity_id
    );
    let body = json!({ "last_progress_at": timestamp_millis_string() });
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

fn context_progress_dispatch_enabled(ctx: &Context) -> bool {
    let fields = ctx
        .entity_state
        .get("fields")
        .and_then(|value| value.as_object());
    let value = fields
        .and_then(|fields| fields.get("context_progress_enabled"))
        .or_else(|| fields.and_then(|fields| fields.get("ContextProgressEnabled")));

    match value {
        Some(Value::Bool(enabled)) => *enabled,
        Some(Value::String(enabled)) => matches!(
            enabled.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        ),
        _ => false,
    }
}

fn send_progress_ignore(ctx: &Context, temper_api_url: &str, tenant: &str, phase: &str) {
    if !context_progress_dispatch_enabled(ctx) {
        return;
    }
    if let Err(err) = send_progress(ctx, temper_api_url, tenant) {
        ctx.log(
            "warn",
            &format!("context_preparer: ProgressMade dispatch failed phase={phase}: {err}"),
        );
    }
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

fn build_tool_definitions(tools_enabled: &str, _sandbox_url: &str, _workdir: &str) -> Vec<Value> {
    let enabled = enabled_tool_set(tools_enabled);
    if enabled.is_empty() {
        return Vec::new();
    }

    let method_listing = build_method_listing(&enabled);
    let description = format!(
        "Execute Python code in the Temper REPL. Treat each call as self-contained: normal sessions may reset the Python heap between provider turns, so do not rely on variables or helper definitions from an earlier call.\n\n\
         Available methods:\n\
         {method_listing}\n\n\
         IMPORTANT PYTHON RULES (this is Monty, a restricted Python — NOT standard CPython):\n\
         - No 'import' statements at all (no import json, os, re, typing, sys — NOTHING)\n\
         - `json` is preloaded for `json.dumps(...)` and `json.loads(...)`; use it without importing\n\
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
                "session_turn: repair_interrupted_tool_use_messages injected {repair_count} synthetic tool_result(s) — session_recoverer should have handled this (ADR-0025)"
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
            if let Some(content) = msg.get_mut("content")
                && let Some(arr) = content.as_array_mut()
            {
                for block in arr.iter_mut() {
                    if block.get("type").and_then(Value::as_str) == Some("tool_result")
                        && let Some(result_content) = block.get_mut("content")
                    {
                        if let Some(parts) = result_content.as_array_mut() {
                            parts.retain(|part| {
                                part.get("type").and_then(Value::as_str) != Some("image")
                            });
                            for part in parts.iter_mut() {
                                if let Some(text) =
                                    part.get("text").and_then(Value::as_str).map(String::from)
                                    && text.len() > 200
                                {
                                    part["text"] =
                                        json!(format!("[text pruned — {} chars]", text.len()));
                                }
                            }
                        } else {
                            let content_str = match result_content.as_str() {
                                Some(text) => text.to_string(),
                                None => serde_json::to_string(&*result_content).unwrap_or_default(),
                            };
                            if content_str.len() > 200 {
                                *result_content = json!(format!(
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
                    "session_turn: TemperFS file has no content, initializing",
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
                            "session_turn: TemperFS conversation read transient HTTP {}, retry {}/{}",
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
                    &format!("session_turn: TemperFS read error: {e}, falling back to inline"),
                );
                return Ok(vec![json!({ "role": "user", "content": user_message })]);
            }
        }
    }

    ctx.log(
        "warn",
        &format!(
            "session_turn: TemperFS read failed (HTTP {}): {}, falling back to inline",
            last_status,
            &last_body[..last_body.len().min(200)]
        ),
    );
    Ok(vec![json!({ "role": "user", "content": user_message })])
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
                    "session_turn: {label} transient HTTP {}, retry {}/{}",
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
    if wasm_helpers::is_session_entries_ref(file_id) {
        let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));
        return wasm_helpers::read_session_from_temperfs(
            ctx,
            temper_api_url,
            tenant,
            &fields,
            file_id,
        );
    }

    read_temperfs_file_value(
        ctx,
        temper_api_url,
        tenant,
        file_id,
        None,
        "TemperFS session read failed",
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
    skills_prompt_mode: &str,
    tools_enabled: &str,
    sandbox_url: &str,
    workdir: &str,
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
        .chain(skills_prompt_mode.bytes())
        .chain(b"|".iter().copied())
        .chain(tools_enabled.bytes())
        .chain(b"|".iter().copied())
        .chain(sandbox_url.bytes())
        .chain(b"|".iter().copied())
        .chain(workdir.bytes())
        .chain(b"|".iter().copied())
        .chain(system_prompt_override.bytes())
    {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3); // FNV prime
    }
    format!("{:016x}", hash)
}

pub fn run_context_preparer() -> Result<(), String> {
    let started_at = Context::get_time_millis();
    let ctx = Context::from_host()?;
    ctx.log("info", "context_preparer: starting");

    let fields = ctx
        .entity_state
        .get("fields")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let user_message = fields
        .get("user_message")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if user_message.is_empty() {
        return Err("user_message is empty — nothing to send to the LLM".to_string());
    }

    let model = fields
        .get("model")
        .and_then(|v| v.as_str())
        .filter(|value| !value.trim().is_empty())
        .ok_or("context_preparer requires Session.model")?;
    let provider = fields
        .get("provider")
        .and_then(|v| v.as_str())
        .filter(|value| !value.trim().is_empty())
        .ok_or("context_preparer requires Session.provider")?;
    let tools_enabled = fields
        .get("tools_enabled")
        .and_then(|v| v.as_str())
        .unwrap_or(DEFAULT_TOOLS_ENABLED);
    let system_prompt_override = fields
        .get("system_prompt")
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
    let temper_api_url = resolve_temper_api_url(&ctx, &fields);
    let tenant = &ctx.tenant;
    let conversation_file_id = fields
        .get("conversation_file_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let session_file_id = fields
        .get("session_file_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let session_leaf_id = fields
        .get("session_leaf_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let workspace_id = fields
        .get("workspace_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let soul_id = fields.get("soul_id").and_then(|v| v.as_str()).unwrap_or("");
    let reserve_tokens: usize = fields
        .get("reserve_tokens")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse().ok())
        .unwrap_or(20_000);
    let max_live_context_bytes: usize = fields
        .get("max_live_context_bytes")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse().ok())
        .or_else(|| {
            ctx.config
                .get("max_live_context_bytes")
                .and_then(|s| s.parse().ok())
        })
        .unwrap_or(48 * 1024 * 1024);
    let context_prepare_budget_ms = configured_budget_ms(
        &ctx,
        &fields,
        "context_prepare_budget_ms",
        DEFAULT_CONTEXT_PREPARE_BUDGET_MS,
    );
    let use_session_tree = !session_file_id.is_empty() && !session_leaf_id.is_empty();
    let prune_after_turns: usize = fields
        .get("prune_tool_results_after_turns")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse().ok())
        .unwrap_or(4);
    let existing_prepared =
        try_read_existing_prepared_context_inline(&ctx, &fields).or_else(|| {
            fields
                .get("prepared_context_file_id")
                .and_then(|v| v.as_str())
                .and_then(|file_id| {
                    try_read_existing_prepared_context_artifact(
                        &ctx,
                        &temper_api_url,
                        tenant,
                        file_id,
                    )
                })
        });

    let load_started_at = Context::get_time_millis();
    let load_result = load_messages_for_prepare(
        &ctx,
        &fields,
        &temper_api_url,
        tenant,
        user_message,
        &conversation_file_id,
        &session_file_id,
        &session_leaf_id,
        &workspace_id,
        prune_after_turns,
        existing_prepared.as_ref(),
    );
    emit_phase_step_duration(
        &ctx,
        "context_preparer",
        "load_messages",
        load_started_at,
        if load_result.is_ok() { "ok" } else { "error" },
    );
    let (mut messages, session_tree, entries_loaded, content_files_loaded) = load_result?;
    check_phase_budget(
        &ctx,
        "context_preparer",
        started_at,
        context_prepare_budget_ms,
        "load_messages",
    )?;
    send_progress_ignore(&ctx, &temper_api_url, tenant, "context_loaded");
    messages = repair_interrupted_tool_use_messages(&ctx, messages);
    prune_old_tool_results(&mut messages, prune_after_turns);

    let tools = build_tool_definitions(tools_enabled, sandbox_url, workdir);
    let context_tokens = if use_session_tree {
        session_tree
            .as_ref()
            .map(|tree| tree.estimate_tokens(&session_leaf_id))
            .unwrap_or_else(|| estimate_message_tokens(&messages) as usize)
    } else {
        estimate_message_tokens(&messages) as usize
    };

    let metric_tags = session_metric_tags(normalize_provider(provider).as_str(), model);
    emit_metric_ignore(
        &ctx,
        "temper_session_context_tokens",
        context_tokens as f64,
        &metric_tags,
        Some("gauge"),
    );
    emit_metric_ignore(
        &ctx,
        "temper_session_context_entries_loaded",
        entries_loaded as f64,
        &metric_tags,
        Some("gauge"),
    );
    emit_metric_ignore(
        &ctx,
        "temper_session_context_content_files_loaded",
        content_files_loaded as f64,
        &metric_tags,
        Some("gauge"),
    );

    let context_window = model_context_window(model);
    if context_tokens > context_window.saturating_sub(reserve_tokens) {
        emit_metric_ignore(
            &ctx,
            "temper_session_compaction_trigger_total",
            1.0,
            &metric_tags,
            Some("count"),
        );
        emit_prepare_duration_metric(&ctx, &metric_tags, started_at);
        emit_phase_total_duration(&ctx, "context_preparer", started_at, "needs_compaction");
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
                "prepared_context_bytes": 0,
                "prepared_context_entries_loaded": entries_loaded,
                "prepared_context_content_files_loaded": content_files_loaded,
                "session_leaf_id": session_leaf_id,
                "system_prompt_hash": existing_hash,
                "system_prompt_file_id": existing_file_id,
            }),
        );
        return Ok(());
    }

    let prompt_started_at = Context::get_time_millis();
    let (assembled_system_prompt, system_prompt_hash, system_prompt_file_id) =
        assemble_cached_system_prompt(
            &ctx,
            &fields,
            existing_prepared.as_ref(),
            &temper_api_url,
            tenant,
            soul_id,
            system_prompt_override,
            tools_enabled,
            sandbox_url,
            workdir,
            &workspace_id,
        )?;
    emit_phase_step_duration(
        &ctx,
        "context_preparer",
        "assemble_system_prompt",
        prompt_started_at,
        "ok",
    );
    check_phase_budget(
        &ctx,
        "context_preparer",
        started_at,
        context_prepare_budget_ms,
        "assemble_system_prompt",
    )?;
    send_progress_ignore(&ctx, &temper_api_url, tenant, "system_prompt_ready");
    let context_bytes =
        estimate_prepared_context_bytes(&messages, &tools, &assembled_system_prompt);
    emit_metric_ignore(
        &ctx,
        "temper_session_context_bytes",
        context_bytes as f64,
        &metric_tags,
        Some("gauge"),
    );

    if context_bytes > max_live_context_bytes {
        emit_metric_ignore(
            &ctx,
            "temper_session_compaction_trigger_total",
            1.0,
            &metric_tags,
            Some("count"),
        );
        emit_metric_ignore(
            &ctx,
            "temper_session_memory_limit_exceeded_total",
            1.0,
            &metric_tags,
            Some("count"),
        );
        emit_prepare_duration_metric(&ctx, &metric_tags, started_at);
        emit_phase_total_duration(&ctx, "context_preparer", started_at, "needs_compaction");
        set_success_result(
            "NeedsCompaction",
            &json!({
                "context_tokens": context_tokens,
                "prepared_context_bytes": context_bytes,
                "prepared_context_entries_loaded": entries_loaded,
                "prepared_context_content_files_loaded": content_files_loaded,
                "session_leaf_id": session_leaf_id,
                "system_prompt_hash": system_prompt_hash,
                "system_prompt_file_id": system_prompt_file_id,
            }),
        );
        return Ok(());
    }

    let artifact = PreparedContextArtifact {
        version: 1,
        messages,
        tools,
        system_prompt: assembled_system_prompt,
        system_prompt_hash: system_prompt_hash.clone(),
        system_prompt_file_id: system_prompt_file_id.clone(),
        conversation_file_id,
        session_file_id,
        session_leaf_id,
        workspace_id: workspace_id.clone(),
        use_session_tree,
        context_tokens,
        context_bytes,
        entries_loaded,
        content_files_loaded,
        prune_tool_results_after_turns: prune_after_turns,
    };
    let artifact_json = serde_json::to_string(&artifact)
        .map_err(|e| format!("prepared context artifact serialize: {e}"))?;
    let stage_started_at = Context::get_time_millis();
    emit_phase_step_duration(
        &ctx,
        "context_preparer",
        "write_prepared_artifact",
        stage_started_at,
        "ok",
    );
    check_phase_budget(
        &ctx,
        "context_preparer",
        started_at,
        context_prepare_budget_ms,
        "write_prepared_artifact",
    )?;
    send_progress_ignore(&ctx, &temper_api_url, tenant, "prepared_context_written");

    emit_prepare_duration_metric(&ctx, &metric_tags, started_at);
    emit_phase_total_duration(&ctx, "context_preparer", started_at, "context_ready");
    set_success_result(
        "ContextReady",
        &json!({
            "prepared_context_file_id": "",
            "prepared_context_inline_json": artifact_json,
            "prepared_context_bytes": context_bytes,
            "prepared_context_entries_loaded": entries_loaded,
            "prepared_context_content_files_loaded": content_files_loaded,
            "context_tokens": context_tokens,
            "system_prompt_hash": system_prompt_hash,
            "system_prompt_file_id": system_prompt_file_id,
        }),
    );
    Ok(())
}

fn read_state_string_field(ctx: &Context, fields: &Value, field_name: &str) -> String {
    match ctx.read_field_string(field_name) {
        Ok(value) if !value.is_empty() => value,
        _ => fields
            .get(field_name)
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
    }
}

fn load_messages_for_prepare(
    ctx: &Context,
    fields: &Value,
    temper_api_url: &str,
    tenant: &str,
    user_message: &str,
    conversation_file_id: &str,
    session_file_id: &str,
    session_leaf_id: &str,
    workspace_id: &str,
    prune_after_turns: usize,
    existing_prepared: Option<&PreparedContextArtifact>,
) -> Result<(Vec<Value>, Option<SessionTree>, usize, usize), String> {
    let use_session_tree = !session_file_id.is_empty() && !session_leaf_id.is_empty();
    if use_session_tree {
        let session_jsonl =
            read_session_from_temperfs(ctx, temper_api_url, tenant, session_file_id)?;
        if session_jsonl.is_empty() {
            let tree = SessionTree::from_jsonl(&session_jsonl);
            return Ok((
                vec![json!({ "role": "user", "content": user_message })],
                Some(tree),
                1,
                0,
            ));
        }

        let tree = SessionTree::from_jsonl(&session_jsonl);
        if let Some(prepared) = existing_prepared {
            match try_reuse_prepared_context(
                prepared,
                &tree,
                conversation_file_id,
                session_file_id,
                session_leaf_id,
                workspace_id,
                prune_after_turns,
                |refs| resolve_context_refs(ctx, temper_api_url, tenant, refs),
            )? {
                PreparedContextReuse::Reused {
                    messages,
                    entries_loaded,
                    content_files_loaded,
                    delta_entries_loaded,
                    delta_content_files_loaded,
                } => {
                    ctx.log(
                        "info",
                        &format!(
                            "context_preparer: reused prepared context delta_entries={delta_entries_loaded} delta_content_files={delta_content_files_loaded}"
                        ),
                    );
                    return Ok((messages, Some(tree), entries_loaded, content_files_loaded));
                }
                PreparedContextReuse::RebuildRequired { reason } => {
                    ctx.log(
                        "info",
                        &format!("context_preparer: prepared context reuse miss: {reason}"),
                    );
                }
            }
        }

        let context_refs = tree.build_context_refs(session_leaf_id);
        let messages = if context_refs.is_empty() {
            vec![json!({ "role": "user", "content": user_message })]
        } else {
            resolve_context_refs(ctx, temper_api_url, tenant, &context_refs)?
        };
        let entries_loaded = usize::max(1, context_refs.len());
        let content_files_loaded = context_refs
            .iter()
            .filter(|ctx_ref| {
                ctx_ref.content_file_id.is_some() || ctx_ref.content_file_version_id.is_some()
            })
            .count();
        if messages.is_empty() {
            Ok((
                vec![json!({ "role": "user", "content": user_message })],
                Some(tree),
                entries_loaded,
                content_files_loaded,
            ))
        } else {
            Ok((messages, Some(tree), entries_loaded, content_files_loaded))
        }
    } else if !conversation_file_id.is_empty() {
        let messages = read_conversation_from_temperfs(
            ctx,
            temper_api_url,
            tenant,
            conversation_file_id,
            user_message,
        )?;
        let content_files_loaded = messages
            .iter()
            .filter(|message| {
                message
                    .get("content")
                    .and_then(Value::as_array)
                    .map(|blocks| {
                        blocks.iter().any(|block| {
                            block.get("type").and_then(Value::as_str) == Some("tool_result")
                        })
                    })
                    .unwrap_or(false)
            })
            .count();
        Ok((messages.clone(), None, messages.len(), content_files_loaded))
    } else {
        let conversation_json = fields
            .get("conversation")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if conversation_json.is_empty() {
            Ok((
                vec![json!({ "role": "user", "content": user_message })],
                None,
                1,
                0,
            ))
        } else {
            let messages = serde_json::from_str(conversation_json)
                .unwrap_or_else(|_| vec![json!({ "role": "user", "content": user_message })]);
            Ok((messages.clone(), None, messages.len(), 0))
        }
    }
}

fn try_read_existing_prepared_context_artifact(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    file_id: &str,
) -> Option<PreparedContextArtifact> {
    if file_id.is_empty() {
        return None;
    }

    match read_prepared_context_artifact(ctx, temper_api_url, tenant, file_id) {
        Ok(prepared) => Some(prepared),
        Err(err) => {
            ctx.log(
                "warn",
                &format!(
                    "context_preparer: prepared context reuse unavailable, ignoring cached artifact: {err}"
                ),
            );
            None
        }
    }
}

fn try_read_existing_prepared_context_inline(
    ctx: &Context,
    fields: &Value,
) -> Option<PreparedContextArtifact> {
    let raw = read_state_string_field(ctx, fields, "prepared_context_inline_json");
    if raw.is_empty() {
        return None;
    }

    match parse_prepared_context_artifact(&raw) {
        Ok(prepared) => Some(prepared),
        Err(err) => {
            ctx.log(
                "warn",
                &format!(
                    "context_preparer: prepared context reuse unavailable, ignoring inline artifact: {err}"
                ),
            );
            None
        }
    }
}

fn read_prepared_context_artifact(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    file_id: &str,
) -> Result<PreparedContextArtifact, String> {
    let raw = read_content_file_raw(ctx, temper_api_url, tenant, file_id)?;
    parse_prepared_context_artifact(&raw)
}

fn try_reuse_prepared_context(
    prepared: &PreparedContextArtifact,
    tree: &SessionTree,
    conversation_file_id: &str,
    session_file_id: &str,
    session_leaf_id: &str,
    workspace_id: &str,
    prune_after_turns: usize,
    resolve_delta: impl FnOnce(&[ContextRef]) -> Result<Vec<Value>, String>,
) -> Result<PreparedContextReuse, String> {
    if !prepared.use_session_tree {
        return Ok(PreparedContextReuse::RebuildRequired {
            reason: "prepared artifact is not session-tree based",
        });
    }
    if prepared.conversation_file_id != conversation_file_id {
        return Ok(PreparedContextReuse::RebuildRequired {
            reason: "conversation file changed",
        });
    }
    if prepared.session_file_id != session_file_id {
        return Ok(PreparedContextReuse::RebuildRequired {
            reason: "session file changed",
        });
    }
    if prepared.workspace_id != workspace_id {
        return Ok(PreparedContextReuse::RebuildRequired {
            reason: "workspace changed",
        });
    }
    if prepared.prune_tool_results_after_turns != prune_after_turns {
        return Ok(PreparedContextReuse::RebuildRequired {
            reason: "prune window changed",
        });
    }
    if prepared.session_leaf_id.is_empty() {
        return Ok(PreparedContextReuse::RebuildRequired {
            reason: "prepared artifact has no session leaf",
        });
    }

    let Some(delta) = tree.build_context_refs_since(session_leaf_id, &prepared.session_leaf_id)
    else {
        return Ok(PreparedContextReuse::RebuildRequired {
            reason: "prepared leaf is not an ancestor of current leaf",
        });
    };

    if delta.includes_compaction {
        return Ok(PreparedContextReuse::RebuildRequired {
            reason: "delta includes compaction",
        });
    }

    let delta_entries_loaded = delta.refs.len();
    let delta_content_files_loaded = delta
        .refs
        .iter()
        .filter(|ctx_ref| {
            ctx_ref.content_file_id.is_some() || ctx_ref.content_file_version_id.is_some()
        })
        .count();
    let delta_messages = if delta.refs.is_empty() {
        Vec::new()
    } else {
        resolve_delta(&delta.refs)?
    };

    let mut messages = prepared.messages.clone();
    messages.extend(delta_messages);

    Ok(PreparedContextReuse::Reused {
        messages,
        entries_loaded: prepared.entries_loaded + delta_entries_loaded,
        content_files_loaded: prepared.content_files_loaded + delta_content_files_loaded,
        delta_entries_loaded,
        delta_content_files_loaded,
    })
}

fn assemble_cached_system_prompt(
    ctx: &Context,
    fields: &Value,
    existing_prepared: Option<&PreparedContextArtifact>,
    temper_api_url: &str,
    tenant: &str,
    soul_id: &str,
    system_prompt_override: &str,
    tools_enabled: &str,
    sandbox_url: &str,
    workdir: &str,
    workspace_id: &str,
) -> Result<(String, String, String), String> {
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
    let skills_prompt_mode = configured_string(ctx, fields, "skills_prompt_mode", "index");

    let new_prompt_hash = compute_system_prompt_hash(
        soul_id,
        agent_id,
        project_harness_id,
        project_id,
        session_mode,
        active_plan_id,
        &skills_prompt_mode,
        tools_enabled,
        sandbox_url,
        workdir,
        system_prompt_override,
    );
    let prev_hash = fields
        .get("system_prompt_hash")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let prev_file_id = fields
        .get("system_prompt_file_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let inline_cached_prompt = existing_prepared.filter(|prepared| {
        prepared.system_prompt_hash == new_prompt_hash && !prepared.system_prompt.is_empty()
    });
    let (assembled_system_prompt, system_prompt_file_id) = if let Some(prepared) =
        inline_cached_prompt
    {
        ctx.log("info", "context_preparer: system prompt inline cache HIT");
        (
            prepared.system_prompt.clone(),
            prepared.system_prompt_file_id.clone(),
        )
    } else if !prev_hash.is_empty() && prev_hash == new_prompt_hash && !prev_file_id.is_empty() {
        match read_temperfs_file(ctx, temper_api_url, tenant, prev_file_id) {
            Ok(cached) if !cached.is_empty() => {
                ctx.log("info", "context_preparer: system prompt cache HIT");
                (cached, prev_file_id.to_string())
            }
            _ => {
                ctx.log(
                    "warn",
                    "context_preparer: system prompt cache file unreadable, rebuilding",
                );
                let prompt = assemble_system_prompt(
                    ctx,
                    temper_api_url,
                    tenant,
                    soul_id,
                    system_prompt_override,
                    tools_enabled,
                    sandbox_url,
                    workdir,
                )?;
                let file_id = write_system_prompt_cache_if_enabled(
                    ctx,
                    fields,
                    temper_api_url,
                    tenant,
                    workspace_id,
                    &prompt,
                );
                (prompt, file_id)
            }
        }
    } else {
        ctx.log(
            "info",
            "context_preparer: system prompt cache MISS, assembling",
        );
        let prompt = assemble_system_prompt(
            ctx,
            temper_api_url,
            tenant,
            soul_id,
            system_prompt_override,
            tools_enabled,
            sandbox_url,
            workdir,
        )?;
        let file_id = write_system_prompt_cache_if_enabled(
            ctx,
            fields,
            temper_api_url,
            tenant,
            workspace_id,
            &prompt,
        );
        (prompt, file_id)
    };

    Ok((
        assembled_system_prompt,
        new_prompt_hash,
        system_prompt_file_id,
    ))
}

fn bool_field_or_config(ctx: &Context, fields: &Value, key: &str, default_value: bool) -> bool {
    fields
        .get(key)
        .and_then(Value::as_str)
        .or_else(|| ctx.config.get(key).map(String::as_str))
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes"
            )
        })
        .unwrap_or(default_value)
}

fn configured_string(ctx: &Context, fields: &Value, key: &str, default_value: &str) -> String {
    fields
        .get(key)
        .and_then(Value::as_str)
        .or_else(|| ctx.config.get(key).map(String::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(default_value)
        .to_ascii_lowercase()
}

fn write_system_prompt_cache_if_enabled(
    ctx: &Context,
    fields: &Value,
    temper_api_url: &str,
    tenant: &str,
    workspace_id: &str,
    prompt: &str,
) -> String {
    if !bool_field_or_config(ctx, fields, "system_prompt_cache_file_enabled", false) {
        return String::new();
    }
    write_system_prompt_cache(ctx, temper_api_url, tenant, workspace_id, prompt).unwrap_or_default()
}

fn estimate_prepared_context_bytes(
    messages: &[Value],
    tools: &[Value],
    system_prompt: &str,
) -> usize {
    system_prompt.len()
        + serde_json::to_string(messages).unwrap_or_default().len()
        + serde_json::to_string(tools).unwrap_or_default().len()
}

fn upsert_artifact_file(
    ctx: &Context,
    fields: &Value,
    temper_api_url: &str,
    tenant: &str,
    workspace_id: &str,
    existing_file_id: &str,
    file_name: &str,
    body: &str,
    content_type: &str,
) -> Result<String, String> {
    if !existing_file_id.is_empty() {
        let value_url = format!("{temper_api_url}/tdata/Files('{existing_file_id}')/$value");
        let headers = runtime_headers(ctx, tenant, fields, Some(content_type), None);
        if write_temperfs_value_with_retry(ctx, &value_url, &headers, body, file_name).is_ok() {
            return Ok(existing_file_id.to_string());
        }
    }

    let effective_workspace = if workspace_id.is_empty() {
        "default"
    } else {
        workspace_id
    };
    create_content_file(
        ctx,
        temper_api_url,
        tenant,
        effective_workspace,
        file_name,
        body,
    )
}

fn session_metric_tags(provider: &str, model: &str) -> Value {
    json!({
        "provider": provider,
        "model": model,
    })
}

fn emit_metric_ignore(ctx: &Context, name: &str, value: f64, tags: &Value, kind: Option<&str>) {
    let _ = ctx.emit_metric(name, value, tags, kind);
}

fn elapsed_ms_since(started_at: i64) -> i64 {
    Context::get_time_millis().saturating_sub(started_at)
}

fn configured_budget_ms(ctx: &Context, fields: &Value, key: &str, default_value: i64) -> i64 {
    fields
        .get(key)
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<i64>().ok())
        .or_else(|| ctx.config.get(key).and_then(|s| s.parse::<i64>().ok()))
        .filter(|value| *value > 0)
        .unwrap_or(default_value)
}

fn emit_phase_step_duration(
    ctx: &Context,
    phase: &str,
    step: &str,
    started_at: i64,
    result: &str,
) -> i64 {
    let elapsed_ms = elapsed_ms_since(started_at);
    emit_metric_ignore(
        ctx,
        "temper_session_phase_step_duration_ms",
        elapsed_ms as f64,
        &json!({
            "phase": phase,
            "step": step,
            "result": result,
        }),
        Some("histogram"),
    );
    ctx.log(
        "info",
        &format!("session_phase phase={phase} step={step} result={result} elapsed_ms={elapsed_ms}"),
    );
    elapsed_ms
}

fn emit_phase_total_duration(ctx: &Context, phase: &str, started_at: i64, result: &str) -> i64 {
    let elapsed_ms = elapsed_ms_since(started_at);
    emit_metric_ignore(
        ctx,
        "temper_session_phase_duration_ms",
        elapsed_ms as f64,
        &json!({
            "phase": phase,
            "result": result,
        }),
        Some("histogram"),
    );
    elapsed_ms
}

fn check_phase_budget(
    ctx: &Context,
    phase: &str,
    started_at: i64,
    budget_ms: i64,
    last_step: &str,
) -> Result<(), String> {
    let elapsed_ms = elapsed_ms_since(started_at);
    if elapsed_ms <= budget_ms {
        return Ok(());
    }

    emit_metric_ignore(
        ctx,
        "temper_session_phase_budget_exceeded_total",
        1.0,
        &json!({
            "phase": phase,
            "last_step": last_step,
        }),
        Some("count"),
    );
    Err(format!(
        "{phase}: exceeded local budget after {last_step} (elapsed_ms={elapsed_ms}, budget_ms={budget_ms})"
    ))
}

fn emit_prepare_duration_metric(ctx: &Context, tags: &Value, started_at: i64) {
    let elapsed = elapsed_ms_since(started_at);
    emit_metric_ignore(
        ctx,
        "temper_session_context_prepare_duration_ms",
        elapsed as f64,
        tags,
        Some("gauge"),
    );
}

fn model_context_window(_model: &str) -> usize {
    200_000
}

/// Assemble the full system prompt from soul + override + harness + skills + memory.
fn assemble_system_prompt(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    soul_id: &str,
    system_prompt_override: &str,
    tools_enabled: &str,
    sandbox_url: &str,
    workdir: &str,
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
        let empty_fields = json!({});
        let fields_for_config = fields_val.unwrap_or(&empty_fields);
        let skills_prompt_mode =
            configured_string(ctx, fields_for_config, "skills_prompt_mode", "index");
        if !matches!(
            skills_prompt_mode.as_str(),
            "off" | "none" | "disabled" | "false"
        ) {
            let include_bodies = matches!(
                skills_prompt_mode.as_str(),
                "full" | "body" | "bodies" | "legacy"
            );
            match load_skills_block(
                ctx,
                temper_api_url,
                tenant,
                project_id,
                agent_id,
                include_bodies,
            ) {
                Ok(block) if !block.is_empty() => parts.push(block),
                Ok(_) => {}
                Err(e) => ctx.log(
                    "warn",
                    &format!("assemble_system_prompt: failed to load skills: {e}"),
                ),
            }
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
    if !enabled_tool_set(tools_enabled).is_empty() {
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
    if enabled.is_empty() {
        return String::new();
    }

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
         - `json` is preloaded for `json.dumps(...)` and `json.loads(...)`; use it without importing\n\
         - No enumerate(x, start=N) — use range(len(x)) instead\n\
         - No f-strings with nested quotes — use string concatenation\n\
         - No tuple comparison (<, >) — compare individual elements\n\
         - All temper.* calls return pre-parsed dicts/lists — no json.loads() needed\n\
         - No pip packages (no requests, httpx, numpy, pandas, etc.)\n\
         - No network access from Python — use sandbox.bash(\"curl ...\") for HTTP\n\
         - No filesystem access from Python — use sandbox.read/write/edit\n\
         - Treat each execute call as self-contained. Normal sessions may reset the Python heap between provider turns, so do not rely on variables or helper definitions created in an earlier call\n\
         - Persist important artifacts to Temper entities or Files so they survive crashes, handoffs, or later jobs\n\
         - Prefer focused execute scripts that complete a coherent unit of work in one call; avoid splitting tiny dependent snippets across turns\n\
         - If a script starts getting large, persist intermediate results to Temper entities/files and continue in a follow-up execute call with explicit IDs\n\
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
         temper.action(\"Issues\", issue[\"entity_id\"], \"TemperPaw.PM.MoveToTriage\", {})\n\
         temper.save_memory(\"test_results\", \"pytest: 47 passed, 0 failed\", \"project\")\n\
         ```\n",
    );
    sections.push(examples);

    sections.push(
        "## Efficiency\n\n\
         Batch closely related dependent steps in one execute call, but do not force an entire long-running workflow into one giant script.\n\
         BAD: 5 separate execute calls for 5 one-line operations\n\
         BAD: 1 monolithic execute call that tries to ingest, plan, synthesize, and publish everything at once\n\
         GOOD: 1 focused execute call per coherent chunk of work, with durable IDs/results carried explicitly between calls\n\n\
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
    include_bodies: bool,
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

    // Collect all SKILL.md files across all prefixes.
    // Tuple: (file_id, path, workspace_id).
    // workspace_id is threaded into the `<skill>` advertisement so the agent
    // can resolve the skill via `temper.read(path, {workspace_id})` against
    // the workspace the skill actually lives in — typically the shared
    // `os-app-docs` workspace, not the session's own workspace.
    let mut file_entries: Vec<(String, String, String)> = Vec::new();

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
                        let workspace_id = entity_field_str(item, &["WorkspaceId", "workspace_id"])
                            .unwrap_or("")
                            .to_string();
                        if !id.is_empty() {
                            file_entries.push((id, path, workspace_id));
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

    // Split into system skills (always full body) and project/agent skills
    // (body controlled by `include_bodies`).
    //
    // System skills carry universal platform knowledge — agents need their full
    // content to understand how the platform works (installed apps, entity
    // discovery, etc.). They must load on every session regardless of the
    // configured `skills_prompt_mode`. Project/agent-scoped skills stay
    // mode-controlled to keep the prompt budget bounded.
    let (system_files, scoped_files): (
        Vec<(String, String, String)>,
        Vec<(String, String, String)>,
    ) = file_entries
        .into_iter()
        .partition(|(_, path, _)| !path.starts_with("/agents/") && !path.starts_with("/projects/"));

    // Read each file's content, parse frontmatter for name + description.
    // Tuple: (norm_key, scope_priority, name, desc, path, workspace_id, body)
    // scope_priority: 0 = agent (most specific), 1 = project, 2 = system (least specific)
    // body: full content (sans frontmatter) for system skills; empty for others (L0 only)
    let mut entries: Vec<(String, u8, String, String, String, String, String)> = Vec::new();

    // Always fetch system skill bodies.
    for (file_id, path, workspace_id) in &system_files {
        let url = format!("{temper_api_url}/tdata/Files('{file_id}')/$value");
        match ctx.http_call("GET", &url, &headers, "") {
            Ok(resp) if resp.status == 200 && !resp.body.is_empty() => {
                let (fm_name, fm_desc, _) = parse_skill_frontmatter(&resp.body);
                let name = if fm_name.is_empty() {
                    skill_name_from_path(path)
                } else {
                    fm_name
                };
                let body = strip_skill_frontmatter(&resp.body).to_string();
                entries.push((
                    normalize_skill_key(&name),
                    2,
                    name,
                    fm_desc,
                    path.clone(),
                    workspace_id.clone(),
                    body,
                ));
            }
            Ok(_) => {}
            Err(e) => ctx.log(
                "warn",
                &format!("load_skills_block: failed to read system skill {file_id}: {e}"),
            ),
        }
    }

    // Project/agent skills: fetch bodies only when caller asked for full mode;
    // otherwise emit them as L0 index entries (no extra HTTP fetch).
    if include_bodies {
        for (file_id, path, workspace_id) in &scoped_files {
            let url = format!("{temper_api_url}/tdata/Files('{file_id}')/$value");
            match ctx.http_call("GET", &url, &headers, "") {
                Ok(resp) if resp.status == 200 && !resp.body.is_empty() => {
                    let (fm_name, fm_desc, _) = parse_skill_frontmatter(&resp.body);
                    let name = if fm_name.is_empty() {
                        skill_name_from_path(path)
                    } else {
                        fm_name
                    };
                    let scope_priority = if path.starts_with("/agents/") { 0 } else { 1 };
                    entries.push((
                        normalize_skill_key(&name),
                        scope_priority,
                        name,
                        fm_desc,
                        path.clone(),
                        workspace_id.clone(),
                        String::new(),
                    ));
                }
                Ok(_) => {}
                Err(e) => ctx.log(
                    "warn",
                    &format!("load_skills_block: failed to read scoped skill {file_id}: {e}"),
                ),
            }
        }
    } else {
        for (_file_id, path, workspace_id) in &scoped_files {
            let name = skill_name_from_path(path);
            let scope_priority = if path.starts_with("/agents/") { 0 } else { 1 };
            entries.push((
                normalize_skill_key(&name),
                scope_priority,
                name,
                String::new(),
                path.clone(),
                workspace_id.clone(),
                String::new(),
            ));
        }
    }

    if entries.is_empty() {
        return Ok(String::new());
    }

    // Sort by (norm_key, scope_priority) so most-specific scope wins dedup.
    entries.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    let mut seen_names = BTreeSet::new();
    let mut xml = String::from("<available_skills>\n");
    for (norm, _priority, name, desc, skill_path, workspace_id, body) in &entries {
        if !seen_names.insert(norm.clone()) {
            continue;
        }
        let ws_attr = if workspace_id.is_empty() {
            String::new()
        } else {
            format!(" workspace_id=\"{}\"", xml_escape(workspace_id))
        };
        if body.is_empty() {
            // L0: name + description + path + workspace_id (project/agent-scoped skills).
            xml.push_str(&format!(
                "  <skill name=\"{}\" description=\"{}\" path=\"{}\"{} />\n",
                xml_escape(name),
                xml_escape(desc),
                xml_escape(skill_path),
                ws_attr,
            ));
        } else {
            // Full injection: system skills include their complete content.
            xml.push_str(&format!(
                "  <skill name=\"{}\" description=\"{}\" path=\"{}\"{}>\n{}\n  </skill>\n",
                xml_escape(name),
                xml_escape(desc),
                xml_escape(skill_path),
                ws_attr,
                body,
            ));
        }
    }
    xml.push_str("</available_skills>");
    Ok(xml)
}

fn render_skill_index(mut file_entries: Vec<(String, String, String)>) -> String {
    file_entries.sort_by(|a, b| {
        let a_name = normalize_skill_key(&skill_name_from_path(&a.1));
        let b_name = normalize_skill_key(&skill_name_from_path(&b.1));
        a_name
            .cmp(&b_name)
            .then(scope_priority(&a.1).cmp(&scope_priority(&b.1)))
    });

    let mut seen_names = BTreeSet::new();
    let mut xml = String::from("<available_skills mode=\"index\">\n");
    for (_file_id, path, workspace_id) in file_entries {
        let name = skill_name_from_path(&path);
        let norm = normalize_skill_key(&name);
        if !seen_names.insert(norm) {
            continue;
        }
        let ws_attr = if workspace_id.is_empty() {
            String::new()
        } else {
            format!(" workspace_id=\"{}\"", xml_escape(&workspace_id))
        };
        xml.push_str(&format!(
            "  <skill name=\"{}\" path=\"{}\"{} />\n",
            xml_escape(&name),
            xml_escape(&path),
            ws_attr,
        ));
    }
    xml.push_str("</available_skills>");
    xml
}

fn scope_priority(path: &str) -> u8 {
    if path.starts_with("/agents/") {
        0
    } else if path.starts_with("/projects/") {
        1
    } else {
        2
    }
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
    let mut unique_file_version_ids = Vec::new();
    let mut unique_file_ids = Vec::new();
    let mut seen = BTreeSet::new();
    for ctx_ref in refs {
        if let Some(file_version_id) = &ctx_ref.content_file_version_id
            && seen.insert(format!("version:{file_version_id}"))
        {
            unique_file_version_ids.push(file_version_id.clone());
        }
        if let Some(file_id) = &ctx_ref.content_file_id
            && seen.insert(format!("file:{file_id}"))
        {
            unique_file_ids.push(file_id.clone());
        }
    }

    let version_batch_results = if !unique_file_version_ids.is_empty() {
        match read_text_file_versions_batch(
            ctx,
            temper_api_url,
            tenant,
            &json!({}),
            &unique_file_version_ids,
        ) {
            Ok(results) => results,
            Err(err) => {
                ctx.log(
                    "warn",
                    &format!(
                        "context_preparer: batch file version read unavailable, falling back: {err}"
                    ),
                );
                BTreeMap::new()
            }
        }
    } else {
        BTreeMap::new()
    };

    let file_batch_results = if !unique_file_ids.is_empty() {
        match read_text_files_batch(ctx, temper_api_url, tenant, &json!({}), &unique_file_ids) {
            Ok(results) => results,
            Err(err) => {
                ctx.log(
                    "warn",
                    &format!("context_preparer: batch file read unavailable, falling back: {err}"),
                );
                BTreeMap::new()
            }
        }
    } else {
        BTreeMap::new()
    };

    render_context_refs(refs, |ctx_ref| {
        if let Some(file_version_id) = &ctx_ref.content_file_version_id {
            if let Some(item) = version_batch_results.get(file_version_id) {
                return Ok(if item.found {
                    item.text.clone()
                } else {
                    String::new()
                });
            }
            match read_content_file_version_raw(ctx, temper_api_url, tenant, file_version_id) {
                Ok(raw) if !raw.is_empty() => return Ok(raw),
                Ok(_) => {}
                Err(err) => ctx.log(
                    "warn",
                    &format!(
                        "context_preparer: immutable version read unavailable for {file_version_id}, falling back to file head: {err}"
                    ),
                ),
            }
        }

        if let Some(file_id) = &ctx_ref.content_file_id {
            if let Some(item) = file_batch_results.get(file_id) {
                return Ok(if item.found {
                    item.text.clone()
                } else {
                    String::new()
                });
            }
            return read_content_file_raw(ctx, temper_api_url, tenant, file_id);
        }

        Ok(String::new())
    })
}

fn render_context_refs(
    refs: &[session_tree_lib::ContextRef],
    mut read_file: impl FnMut(&session_tree_lib::ContextRef) -> Result<String, String>,
) -> Result<Vec<Value>, String> {
    let mut messages = Vec::new();
    for ctx_ref in refs {
        match ctx_ref.entry_type {
            EntryType::Compaction => {
                let summary = if ctx_ref.content_file_id.is_some()
                    || ctx_ref.content_file_version_id.is_some()
                {
                    read_file(ctx_ref)
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
                if ctx_ref.content_file_id.is_some() || ctx_ref.content_file_version_id.is_some() {
                    let raw = read_file(ctx_ref)?;
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

fn read_content_file_version_raw(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    file_version_id: &str,
) -> Result<String, String> {
    let results = read_text_file_versions_batch(
        ctx,
        temper_api_url,
        tenant,
        &json!({}),
        &[file_version_id.to_string()],
    )?;
    Ok(results
        .get(file_version_id)
        .filter(|item| item.found)
        .map(|item| item.text.clone())
        .unwrap_or_default())
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
    fn extract_tool_ids_reads_tool_calls_and_results() {
        let tool_use_ids = extract_tool_use_ids(&json!({
            "role": "assistant",
            "content": [{"type": "tool_use", "id": "tool-1", "name": "execute", "input": {}}]
        }));
        let tool_result_ids = extract_tool_result_ids(&json!({
            "role": "user",
            "content": [{"type": "tool_result", "tool_use_id": "tool-1", "content": "done"}]
        }));

        assert!(tool_use_ids.contains("tool-1"));
        assert!(tool_result_ids.contains("tool-1"));
    }

    #[test]
    fn prune_old_tool_results_truncates_old_payloads() {
        let mut messages = vec![
            json!({"role": "assistant", "content": [{"type": "text", "text": "old turn"}]}),
            json!({
                "role": "user",
                "content": [{
                    "type": "tool_result",
                    "tool_use_id": "tool-1",
                    "content": "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"
                }]
            }),
            json!({"role": "assistant", "content": [{"type": "text", "text": "middle turn"}]}),
            json!({"role": "assistant", "content": [{"type": "text", "text": "recent turn"}]}),
        ];

        prune_old_tool_results(&mut messages, 1);

        let pruned = messages[1]["content"][0]["content"].as_str().unwrap();
        assert!(pruned.contains("tool result pruned"));
    }

    #[test]
    fn build_tool_definitions_reflect_enabled_methods() {
        let tools = build_tool_definitions("temper_get,temper_list", "", "/workspace");
        let description = tools[0]["description"].as_str().unwrap();

        assert!(description.contains("temper.get(entity_set, entity_id)"));
        assert!(description.contains("temper.list(entity_set, filter_str)"));
        assert!(description.contains("Treat each call as self-contained"));
        assert!(!description.contains("Variables persist across calls"));
        assert!(!description.contains("temper.submit_specs(files_dict)"));
    }

    #[test]
    fn build_tool_definitions_empty_when_no_tools_enabled() {
        assert!(build_tool_definitions("", "", "/workspace").is_empty());
        assert!(build_sdk_reference("", "", "/workspace").is_empty());
    }

    #[test]
    fn render_skill_index_avoids_body_injection() {
        let block = render_skill_index(vec![
            (
                "file-1".to_string(),
                "/system/skills/platform-awareness/SKILL.md".to_string(),
                "os-app-docs".to_string(),
            ),
            (
                "file-2".to_string(),
                "/agents/paw/skills/platform-awareness/SKILL.md".to_string(),
                "agent-docs".to_string(),
            ),
        ]);

        assert!(block.contains("mode=\"index\""));
        assert!(block.contains("path=\"/agents/paw/skills/platform-awareness/SKILL.md\""));
        assert!(block.contains("workspace_id=\"agent-docs\""));
        assert!(!block.contains("file-1"));
        assert!(!block.contains("</skill>"));
    }

    #[test]
    fn try_reuse_prepared_context_appends_only_delta_messages() {
        let tree = SessionTree::from_jsonl(
            r#"{"id":"h-1","parentId":null,"type":"header","version":1,"tokens":0}
{"id":"u-1","parentId":"h-1","type":"message","role":"user","content":"hello","tokens":10}
{"id":"a-1","parentId":"u-1","type":"message","role":"assistant","content":[{"type":"text","text":"hi"}],"tokens":5}
{"id":"u-2","parentId":"a-1","type":"message","role":"user","content":"next","tokens":7}"#,
        );
        let prepared = PreparedContextArtifact {
            version: 1,
            messages: vec![
                json!({"role": "user", "content": "hello"}),
                json!({"role": "assistant", "content": [{"type":"text","text":"hi"}]}),
            ],
            tools: vec![],
            system_prompt: "You are concise.".to_string(),
            system_prompt_hash: "hash-123".to_string(),
            system_prompt_file_id: "file-system".to_string(),
            conversation_file_id: String::new(),
            session_file_id: "session-1".to_string(),
            session_leaf_id: "a-1".to_string(),
            workspace_id: "workspace-1".to_string(),
            use_session_tree: true,
            context_tokens: 12,
            context_bytes: 128,
            entries_loaded: 2,
            content_files_loaded: 0,
            prune_tool_results_after_turns: 4,
        };

        let outcome = try_reuse_prepared_context(
            &prepared,
            &tree,
            "",
            "session-1",
            "u-2",
            "workspace-1",
            4,
            |refs| {
                assert_eq!(refs.len(), 1);
                assert_eq!(refs[0].entry_id, "u-2");
                Ok(vec![json!({"role": "user", "content": "next"})])
            },
        )
        .expect("reuse prepared context");

        match outcome {
            PreparedContextReuse::Reused {
                messages,
                entries_loaded,
                content_files_loaded,
                delta_entries_loaded,
                delta_content_files_loaded,
            } => {
                assert_eq!(messages.len(), 3);
                assert_eq!(messages[2]["content"], "next");
                assert_eq!(entries_loaded, 3);
                assert_eq!(content_files_loaded, 0);
                assert_eq!(delta_entries_loaded, 1);
                assert_eq!(delta_content_files_loaded, 0);
            }
            other => panic!("expected reuse, got {other:?}"),
        }
    }

    #[test]
    fn try_reuse_prepared_context_requires_rebuild_when_compaction_enters_delta() {
        let tree = SessionTree::from_jsonl(
            r#"{"id":"h-1","parentId":null,"type":"header","version":1,"tokens":0}
{"id":"u-1","parentId":"h-1","type":"message","role":"user","content":"hello","tokens":10}
{"id":"a-1","parentId":"u-1","type":"message","role":"assistant","content":[{"type":"text","text":"hi"}],"tokens":5}
{"id":"c-1","parentId":"a-1","type":"compaction","summary":"summary","first_kept":"a-1","tokens":3}
{"id":"u-2","parentId":"c-1","type":"message","role":"user","content":"next","tokens":7}"#,
        );
        let prepared = PreparedContextArtifact {
            version: 1,
            messages: vec![
                json!({"role": "user", "content": "hello"}),
                json!({"role": "assistant", "content": [{"type":"text","text":"hi"}]}),
            ],
            tools: vec![],
            system_prompt: "You are concise.".to_string(),
            system_prompt_hash: "hash-123".to_string(),
            system_prompt_file_id: "file-system".to_string(),
            conversation_file_id: String::new(),
            session_file_id: "session-1".to_string(),
            session_leaf_id: "a-1".to_string(),
            workspace_id: "workspace-1".to_string(),
            use_session_tree: true,
            context_tokens: 12,
            context_bytes: 128,
            entries_loaded: 2,
            content_files_loaded: 0,
            prune_tool_results_after_turns: 4,
        };

        let outcome = try_reuse_prepared_context(
            &prepared,
            &tree,
            "",
            "session-1",
            "u-2",
            "workspace-1",
            4,
            |_| panic!("compaction delta should not resolve file content"),
        )
        .expect("reuse decision");

        assert!(matches!(
            outcome,
            PreparedContextReuse::RebuildRequired {
                reason: "delta includes compaction"
            }
        ));
    }
}
