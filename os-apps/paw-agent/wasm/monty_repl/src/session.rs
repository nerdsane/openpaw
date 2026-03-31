//! Session tree persistence, hooks, heartbeat, and file sync.
//!
//! Ported from tool_runner to maintain full compatibility with the
//! agent state machine's conversation and session tracking.

use temper_wasm_sdk::prelude::*;
use wasm_helpers::{entity_field_str, create_content_file};

/// Persist tool results to the session tree (or conversation file, or inline).
///
/// Returns the params to include in the HandleToolResults callback.
pub fn persist_results(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    fields: &Value,
    tool_results: &[Value],
    repl_state: &str,
) -> Result<Value, String> {
    let conversation_file_id = field_str(fields, "conversation_file_id");
    let session_file_id = field_str(fields, "session_file_id");
    let session_leaf_id = field_str(fields, "session_leaf_id");
    let workspace_id = field_str(fields, "workspace_id");

    let results_json = serde_json::to_string(tool_results).unwrap_or_default();
    let mut params = json!({
        "pending_tool_calls": results_json,
        "repl_state": repl_state,
    });

    if !session_file_id.is_empty() && !session_leaf_id.is_empty() {
        // Session tree mode
        let session_jsonl = read_temperfs_file(ctx, temper_api_url, tenant, session_file_id)?;
        let mut tree = session_tree_lib::SessionTree::from_jsonl(&session_jsonl);
        let tool_results_value = json!(tool_results);
        let tokens_est = results_json.len() / 4;
        let content_str = serde_json::to_string(&tool_results_value).unwrap_or_default();

        let (new_leaf, _) = if !workspace_id.is_empty() {
            match create_content_file(
                ctx,
                temper_api_url,
                tenant,
                workspace_id,
                &format!("t-{}", tree.len()),
                &content_str,
            ) {
                Ok(content_file_id) => {
                    tree.append_tool_results_file(session_leaf_id, &content_file_id, tokens_est)
                }
                Err(_) => {
                    tree.append_tool_results(session_leaf_id, &tool_results_value, tokens_est)
                }
            }
        } else {
            tree.append_tool_results(session_leaf_id, &tool_results_value, tokens_est)
        };

        let updated_jsonl = tree.to_jsonl();
        write_temperfs_file(ctx, temper_api_url, tenant, session_file_id, &updated_jsonl)?;

        params["pending_tool_calls"] = json!(compact_tool_results_marker(tool_results));
        params["session_leaf_id"] = json!(new_leaf);
    } else if !conversation_file_id.is_empty() {
        // Legacy flat JSON mode
        let mut messages: Vec<Value> =
            read_conversation(ctx, temper_api_url, tenant, conversation_file_id)?;
        messages.push(json!({ "role": "user", "content": tool_results }));

        let updated = serde_json::to_string(&messages).unwrap_or_default();
        let body = format!("{{\"messages\":{updated}}}");
        let url = format!("{temper_api_url}/tdata/Files('{conversation_file_id}')/$value");
        let headers = file_headers(tenant);
        let resp = ctx.http_call("PUT", &url, &headers, &body)?;
        if resp.status >= 400 {
            return Err(format!(
                "TemperFS conversation write failed (HTTP {})",
                resp.status
            ));
        }
        params["conversation"] = json!(updated);
    } else {
        // Inline mode
        let mut messages: Vec<Value> = {
            let conv = fields
                .get("conversation")
                .and_then(|v| v.as_str())
                .unwrap_or("[]");
            serde_json::from_str(conv).unwrap_or_default()
        };
        messages.push(json!({ "role": "user", "content": tool_results }));
        let updated = serde_json::to_string(&messages).unwrap_or_default();
        params["conversation"] = json!(updated);
    }

    Ok(params)
}

/// Send heartbeat to keep agent alive.
pub fn send_heartbeat(ctx: &Context, temper_api_url: &str, tenant: &str) {
    let url = format!(
        "{temper_api_url}/tdata/Agents('{}')/OpenPaw.Heartbeat",
        ctx.entity_id
    );
    let body = json!({ "last_heartbeat_at": "alive" });
    let headers = odata_headers(tenant);
    let _ = ctx.http_call("POST", &url, &headers, &body.to_string());
}

/// Evaluate before-hooks for a tool/op name. Returns error string if blocked.
pub fn evaluate_before_hooks(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    soul_id: &str,
    hook_policy: &str,
    op_name: &str,
) -> Result<Option<String>, String> {
    if hook_policy == "none" || soul_id.is_empty() {
        return Ok(None);
    }
    let hooks = load_matching_hooks(ctx, temper_api_url, tenant, soul_id, "before", op_name)?;
    for hook in hooks {
        let action = entity_field_str(&hook, &["HookAction"]).unwrap_or("log");
        let name = entity_field_str(&hook, &["Name"]).unwrap_or("hook");
        match action {
            "block" => {
                return Ok(Some(format!(
                    "operation blocked by hook '{name}' for '{op_name}'"
                )));
            }
            "log" => ctx.log(
                "info",
                &format!("monty_repl: before hook '{name}' matched {op_name}"),
            ),
            _ => {}
        }
    }
    Ok(None)
}

/// Apply after-hooks to tool output.
pub fn apply_after_hooks(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    soul_id: &str,
    hook_policy: &str,
    op_name: &str,
    mut output: String,
) -> Result<String, String> {
    if hook_policy != "full_hooks" || soul_id.is_empty() {
        return Ok(output);
    }
    let hooks = load_matching_hooks(ctx, temper_api_url, tenant, soul_id, "after", op_name)?;
    for hook in hooks {
        let action = entity_field_str(&hook, &["HookAction"]).unwrap_or("log");
        let name = entity_field_str(&hook, &["Name"]).unwrap_or("hook");
        match action {
            "modify" => output = format!("[modified by hook:{name}]\n{output}"),
            "log" => ctx.log(
                "info",
                &format!("monty_repl: after hook '{name}' matched {op_name}"),
            ),
            _ => {}
        }
    }
    Ok(output)
}

// --- Internal helpers ---

fn load_matching_hooks(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    soul_id: &str,
    hook_type: &str,
    op_name: &str,
) -> Result<Vec<Value>, String> {
    let url = format!("{temper_api_url}/tdata/ToolHooks");
    let headers = odata_headers(tenant);
    let resp = ctx.http_call("GET", &url, &headers, "")?;
    if resp.status != 200 {
        return Ok(Vec::new());
    }
    let parsed: Value =
        serde_json::from_str(&resp.body).unwrap_or_else(|_| json!({ "value": [] }));
    let hooks = parsed
        .get("value")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|hook| {
            entity_field_str(hook, &["Status"]) == Some("Active")
                && entity_field_str(hook, &["SoulId"]).unwrap_or("") == soul_id
                && entity_field_str(hook, &["HookType"]).unwrap_or("") == hook_type
                && hook_matches(
                    entity_field_str(hook, &["ToolPattern"]).unwrap_or(".*"),
                    op_name,
                )
        })
        .collect();
    Ok(hooks)
}

fn hook_matches(pattern: &str, op_name: &str) -> bool {
    let pattern = pattern.trim();
    if pattern.is_empty() || pattern == ".*" || pattern == "*" {
        return true;
    }
    if pattern.contains('|') {
        return pattern.split('|').any(|part| part.trim() == op_name);
    }
    pattern == op_name
}

fn compact_tool_results_marker(tool_results: &[Value]) -> String {
    let markers: Vec<String> = tool_results
        .iter()
        .map(|r| {
            let tool_id = r.get("tool_use_id").and_then(|v| v.as_str()).unwrap_or("?");
            let is_error = r.get("is_error").and_then(|v| v.as_bool()).unwrap_or(false);
            if is_error {
                format!("{{\"tool_use_id\":\"{tool_id}\",\"is_error\":true}}")
            } else {
                format!("{{\"tool_use_id\":\"{tool_id}\"}}")
            }
        })
        .collect();
    format!("[{}]", markers.join(","))
}

fn read_temperfs_file(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    file_id: &str,
) -> Result<String, String> {
    let url = format!("{temper_api_url}/tdata/Files('{file_id}')/$value");
    let headers = file_headers(tenant);
    let resp = ctx.http_call("GET", &url, &headers, "")?;
    if resp.status == 200 {
        Ok(resp.body)
    } else if resp.status == 404 {
        Ok(String::new())
    } else {
        Err(format!("TemperFS read failed (HTTP {})", resp.status))
    }
}

fn write_temperfs_file(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    file_id: &str,
    content: &str,
) -> Result<(), String> {
    let url = format!("{temper_api_url}/tdata/Files('{file_id}')/$value");
    let headers = file_headers_text(tenant);
    let resp = ctx.http_call("PUT", &url, &headers, content)?;
    if resp.status >= 200 && resp.status < 300 {
        Ok(())
    } else {
        Err(format!("TemperFS write failed (HTTP {})", resp.status))
    }
}

fn read_conversation(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    file_id: &str,
) -> Result<Vec<Value>, String> {
    let content = read_temperfs_file(ctx, temper_api_url, tenant, file_id)?;
    if content.is_empty() {
        return Ok(Vec::new());
    }
    // Try parsing as {messages: [...]} or bare [...]
    if let Ok(wrapper) = serde_json::from_str::<Value>(&content) {
        if let Some(msgs) = wrapper.get("messages").and_then(Value::as_array) {
            return Ok(msgs.clone());
        }
    }
    serde_json::from_str(&content).map_err(|e| format!("failed to parse conversation: {e}"))
}

fn field_str<'a>(fields: &'a Value, key: &str) -> &'a str {
    fields.get(key).and_then(|v| v.as_str()).unwrap_or("")
}

fn odata_headers(tenant: &str) -> Vec<(String, String)> {
    vec![
        ("Content-Type".to_string(), "application/json".to_string()),
        ("X-Tenant-Id".to_string(), tenant.to_string()),
    ]
}

fn file_headers(tenant: &str) -> Vec<(String, String)> {
    vec![
        ("X-Tenant-Id".to_string(), tenant.to_string()),
        ("Accept".to_string(), "application/octet-stream".to_string()),
    ]
}

fn file_headers_text(tenant: &str) -> Vec<(String, String)> {
    vec![
        ("X-Tenant-Id".to_string(), tenant.to_string()),
        ("Content-Type".to_string(), "text/plain".to_string()),
    ]
}
