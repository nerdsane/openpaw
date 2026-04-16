//! Session tree persistence, hooks, heartbeat, and file sync.
//!
//! Ported from tool_runner to maintain full compatibility with the
//! agent state machine's conversation and session tracking.

use temper_wasm_sdk::prelude::*;
use wasm_helpers::{create_content_file, entity_field_str, runtime_headers_as};

const SESSION_ENTRY_FILE_THRESHOLD_BYTES: usize = 4096;

/// Persist tool results to the session tree (or conversation file, or inline).
///
/// Returns the params to include in the HandleToolResults callback.
pub fn persist_results(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    fields: &Value,
    tool_results: &[Value],
    repl_file_id: &str,
) -> Result<Value, String> {
    let conversation_file_id = field_str(fields, "conversation_file_id");
    let session_file_id = field_str(fields, "session_file_id");
    let session_leaf_id = field_str(fields, "session_leaf_id");
    let workspace_id = field_str(fields, "workspace_id");

    let results_json = serde_json::to_string(tool_results).unwrap_or_default();
    // Store repl_file_id (pointer to TemperFS file), not the full repl_state blob
    let mut params = json!({
        "pending_tool_calls": results_json,
        "repl_file_id": repl_file_id,
    });

    if !session_file_id.is_empty() && !session_leaf_id.is_empty() {
        // Session tree mode
        let session_jsonl = read_temperfs_file(ctx, temper_api_url, tenant, session_file_id)?;
        let mut tree = session_tree_lib::SessionTree::from_jsonl(&session_jsonl);
        let tool_results_value = json!(tool_results);
        let tokens_est = results_json.len() / 4;
        let content_str = serde_json::to_string(&tool_results_value).unwrap_or_default();

        let (new_leaf, _) =
            if !workspace_id.is_empty() && should_store_entry_as_file(&content_str) {
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
        let headers = file_headers(ctx, tenant);
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
    let fields = ctx
        .entity_state
        .get("fields")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let url = format!(
        "{temper_api_url}/tdata/Sessions('{}')/OpenPaw.Heartbeat",
        ctx.entity_id
    );
    let body = json!({ "last_heartbeat_at": "alive" });
    let headers = runtime_headers_as(
        ctx,
        tenant,
        &fields,
        "system",
        Some("application/json"),
        None,
    );
    let _ = ctx.http_call("POST", &url, &headers, &body.to_string());
}

/// Encode a tool-span JSONL document by appending new events to the existing content.
///
/// Each event in `new_events` is serialized as a single JSON object on its own line,
/// separated by '\n'. The returned string always ends with '\n' so that subsequent
/// appends stay line-delimited.
pub fn encode_tool_spans_jsonl(existing: &str, new_events: &[Value]) -> String {
    let mut out = String::with_capacity(existing.len() + new_events.len() * 128);
    if !existing.is_empty() {
        out.push_str(existing);
        if !existing.ends_with('\n') {
            out.push('\n');
        }
    }
    for event in new_events {
        let line = serde_json::to_string(event).unwrap_or_else(|_| "{}".to_string());
        out.push_str(&line);
        out.push('\n');
    }
    out
}

/// Append tool-span JSONL events to the session's tool_spans_file_id file.
///
/// Creates the file on first call (when `existing_file_id` is empty). Reads existing
/// JSONL, appends new events, writes back. Returns the file id on success. Returns
/// an empty string when `workspace_id` is empty (no workspace — spans are not persisted).
pub fn append_tool_spans_to_file(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    workspace_id: &str,
    existing_file_id: &str,
    new_events: &[Value],
) -> Result<String, String> {
    if workspace_id.is_empty() || new_events.is_empty() {
        return Ok(existing_file_id.to_string());
    }

    let file_id = if existing_file_id.is_empty() {
        let body = json!({
            "Name": "tool_spans.jsonl",
            "path": "/tool_spans.jsonl",
            "WorkspaceId": workspace_id,
        });
        let url = format!("{temper_api_url}/tdata/Files");
        let headers = file_headers(ctx, tenant);
        let resp = ctx.http_call("POST", &url, &headers, &body.to_string())?;
        if resp.status >= 400 {
            return Err(format!(
                "failed to create tool_spans file (status={}): {}",
                resp.status, resp.body
            ));
        }
        let created: Value = serde_json::from_str(&resp.body)
            .map_err(|e| format!("failed to parse file create response: {e}"))?;
        created
            .get("entity_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    } else {
        existing_file_id.to_string()
    };

    if file_id.is_empty() {
        return Ok(String::new());
    }

    let existing_content = read_temperfs_file_safe(ctx, temper_api_url, tenant, &file_id)?;
    let updated = encode_tool_spans_jsonl(&existing_content, new_events);

    let url = format!("{temper_api_url}/tdata/Files('{file_id}')/$value");
    let headers = workspace_headers(ctx, tenant, workspace_id);
    write_temperfs_file_with_retry(
        ctx,
        &url,
        &headers,
        &updated,
        "failed to write tool_spans file",
    )?;

    Ok(file_id)
}

/// Save REPL state to a TemperFS file. Creates the file on first call,
/// overwrites on subsequent calls. Returns the file ID.
pub fn save_repl_to_file(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    workspace_id: &str,
    existing_file_id: &str,
    repl_state_b64: &str,
) -> Result<String, String> {
    if workspace_id.is_empty() {
        // No workspace — fall back to storing nothing (fresh REPL each turn)
        return Ok(String::new());
    }

    let file_id = if existing_file_id.is_empty() {
        // Create a new file
        let body = json!({
            "Name": "repl_state.b64",
            "path": "/repl_state.b64",
            "WorkspaceId": workspace_id,
        });
        let url = format!("{temper_api_url}/tdata/Files");
        let headers = file_headers(ctx, tenant);
        let resp = ctx.http_call("POST", &url, &headers, &body.to_string())?;
        if resp.status >= 400 {
            return Err(format!(
                "failed to create REPL state file (status={}): {}",
                resp.status, resp.body
            ));
        }
        let created: Value = serde_json::from_str(&resp.body)
            .map_err(|e| format!("failed to parse file create response: {e}"))?;
        created
            .get("entity_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    } else {
        existing_file_id.to_string()
    };

    if file_id.is_empty() {
        return Ok(String::new());
    }

    // Write REPL state to file
    let url = format!("{temper_api_url}/tdata/Files('{file_id}')/$value");
    let headers = workspace_headers(ctx, tenant, workspace_id);
    write_temperfs_file_with_retry(
        ctx,
        &url,
        &headers,
        repl_state_b64,
        "failed to write REPL state file",
    )?;

    Ok(file_id)
}

/// Read a TemperFS file safely (returns empty string on error).
pub fn read_temperfs_file_safe(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    file_id: &str,
) -> Result<String, String> {
    if file_id.is_empty() {
        return Ok(String::new());
    }
    read_temperfs_file(ctx, temper_api_url, tenant, file_id)
}

fn workspace_headers(ctx: &Context, tenant: &str, workspace_id: &str) -> Vec<(String, String)> {
    let fields = ctx
        .entity_state
        .get("fields")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let mut headers = runtime_headers_as(
        ctx,
        tenant,
        &fields,
        "system",
        Some("text/plain"),
        None,
    );
    headers.push(("X-Workspace-Id".to_string(), workspace_id.to_string()));
    headers
}

/// Evaluate before-hooks for a tool/op name. Returns error string if blocked.
/// Not yet wired into the dispatch pipeline; will be called once hook policies are enabled.
#[allow(dead_code)]
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
/// Not yet wired into the dispatch pipeline; will be called once hook policies are enabled.
#[allow(dead_code)]
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

#[allow(dead_code)]
fn load_matching_hooks(
    ctx: &Context,
    temper_api_url: &str,
    _tenant: &str,
    soul_id: &str,
    hook_type: &str,
    op_name: &str,
) -> Result<Vec<Value>, String> {
    let url = format!("{temper_api_url}/tdata/ToolHooks");
    let headers = internal_headers();
    let resp = ctx.http_call("GET", &url, &headers, "")?;
    if resp.status != 200 {
        return Ok(Vec::new());
    }
    let parsed: Value = serde_json::from_str(&resp.body).unwrap_or_else(|_| json!({ "value": [] }));
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

#[allow(dead_code)]
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
    let headers = file_headers(ctx, tenant);

    const READ_ATTEMPTS: usize = 5;
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
                    "monty_repl: TemperFS read transient HTTP {}, retry {}/{}",
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
        "TemperFS read failed (HTTP {}): {}",
        last_status,
        &last_body[..last_body.len().min(200)]
    ))
}

fn is_retriable_write_failure(status: u16, body: &str) -> bool {
    (500..600).contains(&status)
        || body.contains("BlobUploadFailed")
        || body.contains("HTTP -1")
}

fn write_temperfs_file_with_retry(
    ctx: &Context,
    url: &str,
    headers: &[(String, String)],
    content: &str,
    label: &str,
) -> Result<(), String> {
    const WRITE_ATTEMPTS: usize = 5;
    let mut last_status = 0;
    let mut last_body = String::new();

    for attempt in 0..WRITE_ATTEMPTS {
        let resp = ctx.http_call("PUT", url, headers, content)?;
        if (200..300).contains(&resp.status) {
            return Ok(());
        }

        last_status = resp.status;
        last_body = resp.body;

        if is_retriable_write_failure(last_status, &last_body) && attempt + 1 < WRITE_ATTEMPTS {
            ctx.log(
                "warn",
                &format!(
                    "monty_repl: {label} transient HTTP {}, retry {}/{}",
                    last_status,
                    attempt + 2,
                    WRITE_ATTEMPTS
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

fn write_temperfs_file(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    file_id: &str,
    content: &str,
) -> Result<(), String> {
    let url = format!("{temper_api_url}/tdata/Files('{file_id}')/$value");
    let headers = file_headers_text(ctx, tenant);
    write_temperfs_file_with_retry(ctx, &url, &headers, content, "TemperFS write failed")
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

fn should_store_entry_as_file(content: &str) -> bool {
    content.len() > SESSION_ENTRY_FILE_THRESHOLD_BYTES
}

fn internal_headers() -> Vec<(String, String)> {
    vec![("Content-Type".to_string(), "application/json".to_string())]
}

fn file_headers(ctx: &Context, tenant: &str) -> Vec<(String, String)> {
    let fields = ctx
        .entity_state
        .get("fields")
        .cloned()
        .unwrap_or_else(|| json!({}));
    runtime_headers_as(
        ctx,
        tenant,
        &fields,
        "system",
        Some("application/json"),
        Some("application/octet-stream"),
    )
}

fn file_headers_text(ctx: &Context, tenant: &str) -> Vec<(String, String)> {
    let fields = ctx
        .entity_state
        .get("fields")
        .cloned()
        .unwrap_or_else(|| json!({}));
    runtime_headers_as(
        ctx,
        tenant,
        &fields,
        "system",
        Some("text/plain"),
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_tool_spans_jsonl_creates_three_lines_from_empty() {
        let events = vec![
            json!({"tool_call_id": "a", "tool_name": "read", "duration_ms": 12}),
            json!({"tool_call_id": "b", "tool_name": "write", "duration_ms": 45}),
            json!({"tool_call_id": "c", "tool_name": "bash", "duration_ms": 78}),
        ];
        let out = encode_tool_spans_jsonl("", &events);
        let lines: Vec<&str> = out.split_terminator('\n').collect();
        assert_eq!(lines.len(), 3, "expected 3 JSONL lines, got {}\n{}", lines.len(), out);
        for (i, line) in lines.iter().enumerate() {
            let parsed: Value =
                serde_json::from_str(line).expect("each line must be valid JSON");
            assert_eq!(parsed["tool_call_id"], events[i]["tool_call_id"]);
        }
        assert!(out.ends_with('\n'), "output must end with newline");
    }

    #[test]
    fn encode_tool_spans_jsonl_appends_to_existing() {
        let existing = "{\"tool_call_id\":\"prev\"}\n";
        let new_events = vec![json!({"tool_call_id": "next"})];
        let out = encode_tool_spans_jsonl(existing, &new_events);
        let lines: Vec<&str> = out.split_terminator('\n').collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(
            serde_json::from_str::<Value>(lines[0]).unwrap()["tool_call_id"],
            "prev"
        );
        assert_eq!(
            serde_json::from_str::<Value>(lines[1]).unwrap()["tool_call_id"],
            "next"
        );
    }

    #[test]
    fn encode_tool_spans_jsonl_handles_existing_without_trailing_newline() {
        let existing = "{\"tool_call_id\":\"prev\"}";
        let new_events = vec![json!({"tool_call_id": "next"})];
        let out = encode_tool_spans_jsonl(existing, &new_events);
        assert!(
            out.contains("{\"tool_call_id\":\"prev\"}\n{\"tool_call_id\":\"next\"}"),
            "missing newline separator between existing content and new event: {out}"
        );
    }

    #[test]
    fn encode_tool_spans_jsonl_empty_events_returns_existing() {
        let existing = "{\"tool_call_id\":\"prev\"}\n";
        let out = encode_tool_spans_jsonl(existing, &[]);
        assert_eq!(out, existing);
    }
}
