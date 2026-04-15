use std::collections::BTreeMap;

use temper_wasm_sdk::prelude::*;
use wasm_helpers::{entity_field_str, runtime_headers_as};

pub const MANAGED_NAMESPACE: &str = "ManagedAgents";
pub const DEFAULT_OPENPAW_TOOLS: &str = "temper_create,temper_get,temper_list,temper_action,temper_patch,temper_submit_specs,temper_show_spec,temper_specs,temper_upload_wasm,temper_get_trajectories,temper_get_insights,temper_get_decisions,temper_poll_decision,temper_approve_decision,temper_deny_decision,temper_submit_policy,temper_list_policies,temper_get_policy,temper_update_policy,temper_delete_policy,temper_install_app,temper_list_apps,temper_spawn_session,temper_list_sessions,temper_abort_session,temper_steer_session,temper_save_memory,temper_recall_memory,temper_write,temper_read,temper_run_coding_agent,temper_get_secret,temper_datadog_query,temper_railway,temper_vercel,temper_web_search,temper_web_fetch,read,write,edit,bash";

pub fn system_json_headers(
    ctx: &Context,
    tenant: &str,
    fields: &Value,
) -> Vec<(String, String)> {
    runtime_headers_as(
        ctx,
        tenant,
        fields,
        "system",
        Some("application/json"),
        Some("application/json"),
    )
}

pub fn entity_id(value: &Value) -> Option<String> {
    value
        .get("entity_id")
        .and_then(Value::as_str)
        .or_else(|| entity_field_str(value, &["Id", "id"]))
        .map(str::to_string)
}

pub fn field_string(value: &Value, keys: &[&str]) -> String {
    entity_field_str(value, keys).unwrap_or("").to_string()
}

pub fn field_i64(value: &Value, keys: &[&str]) -> i64 {
    for key in keys {
        if let Some(raw) = value.get(*key) {
            if let Some(num) = raw.as_i64() {
                return num;
            }
            if let Some(text) = raw.as_str() {
                if let Ok(parsed) = text.parse::<i64>() {
                    return parsed;
                }
            }
        }
    }
    if let Some(fields) = value.get("fields") {
        return field_i64(fields, keys);
    }
    0
}

pub fn status_of(value: &Value) -> String {
    field_string(value, &["Status", "status"])
}

pub fn is_terminal_status(status: &str) -> bool {
    matches!(status, "Completed" | "Failed" | "Cancelled" | "Archived" | "Destroyed")
}

pub fn infer_provider(model_id: &str) -> &'static str {
    if model_id.starts_with("gpt-") || model_id.starts_with("o1") || model_id.starts_with("o3") {
        "openai"
    } else if model_id.contains('/') {
        "openrouter"
    } else {
        "anthropic"
    }
}

pub fn escape_odata_string(value: &str) -> String {
    value.replace('\'', "''")
}

pub fn http_json(
    ctx: &Context,
    method: &str,
    url: &str,
    headers: &[(String, String)],
    body: &Value,
    label: &str,
) -> Result<Value, String> {
    let resp = ctx.http_call(method, url, headers, &body.to_string())?;
    parse_json_response(resp, label)
}

pub fn http_empty(
    ctx: &Context,
    method: &str,
    url: &str,
    headers: &[(String, String)],
    label: &str,
) -> Result<Value, String> {
    let resp = ctx.http_call(method, url, headers, "")?;
    parse_json_response(resp, label)
}

pub fn parse_json_response(resp: HttpResponse, label: &str) -> Result<Value, String> {
    if !(200..300).contains(&resp.status) {
        return Err(format!(
            "{label} failed (HTTP {}): {}",
            resp.status,
            truncate(&resp.body)
        ));
    }

    if resp.body.trim().is_empty() {
        return Ok(json!({}));
    }

    serde_json::from_str(&resp.body).map_err(|err| {
        format!(
            "{label} returned invalid JSON: {err}: {}",
            truncate(&resp.body)
        )
    })
}

pub fn get_entity(
    ctx: &Context,
    base_url: &str,
    headers: &[(String, String)],
    entity_set: &str,
    id: &str,
) -> Result<Value, String> {
    http_empty(
        ctx,
        "GET",
        &format!("{base_url}/tdata/{entity_set}('{id}')"),
        headers,
        &format!("GET {entity_set}('{id}')"),
    )
}

pub fn create_entity(
    ctx: &Context,
    base_url: &str,
    headers: &[(String, String)],
    entity_set: &str,
    body: &Value,
) -> Result<Value, String> {
    http_json(
        ctx,
        "POST",
        &format!("{base_url}/tdata/{entity_set}"),
        headers,
        body,
        &format!("create {entity_set}"),
    )
}

pub fn post_action(
    ctx: &Context,
    base_url: &str,
    headers: &[(String, String)],
    entity_set: &str,
    entity_id: &str,
    action: &str,
    body: &Value,
    await_integration: bool,
) -> Result<Value, String> {
    let mut url = format!("{base_url}/tdata/{entity_set}('{entity_id}')/{MANAGED_NAMESPACE}.{action}");
    if await_integration {
        url.push_str("?await_integration=true");
    }
    http_json(
        ctx,
        "POST",
        &url,
        headers,
        body,
        &format!("{action} on {entity_set}('{entity_id}')"),
    )
}

pub fn post_absolute_action(
    ctx: &Context,
    headers: &[(String, String)],
    url: &str,
    body: &Value,
    label: &str,
) -> Result<Value, String> {
    http_json(ctx, "POST", url, headers, body, label)
}

pub fn list_entities(
    ctx: &Context,
    base_url: &str,
    headers: &[(String, String)],
    relative_path: &str,
) -> Result<Vec<Value>, String> {
    let url = format!("{base_url}{relative_path}");
    let resp = ctx.http_call("GET", &url, headers, "")?;
    let parsed = parse_json_response(resp, &format!("list {relative_path}"))?;
    Ok(parsed
        .get("value")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default())
}

pub fn next_session_event_sequence(
    ctx: &Context,
    base_url: &str,
    headers: &[(String, String)],
    session_id: &str,
) -> Result<i64, String> {
    let escaped = escape_odata_string(session_id);
    let events = list_entities(
        ctx,
        base_url,
        headers,
        &format!(
            "/tdata/SessionEvents?$filter=SessionId%20eq%20'{escaped}'&$orderby=Sequence%20desc&$top=1"
        ),
    )?;
    let next = events
        .first()
        .map(|item| field_i64(item, &["Sequence", "sequence"]) + 1)
        .unwrap_or(1);
    Ok(next)
}

pub fn create_session_event(
    ctx: &Context,
    base_url: &str,
    headers: &[(String, String)],
    session_id: &str,
    sequence: i64,
    kind: &str,
    extra_fields: Value,
) -> Result<Value, String> {
    let mut body = BTreeMap::new();
    body.insert("SessionId".to_string(), json!(session_id));
    body.insert("Sequence".to_string(), json!(sequence));
    body.insert("Kind".to_string(), json!(kind));

    if let Some(extra) = extra_fields.as_object() {
        for (key, value) in extra {
            body.insert(key.clone(), value.clone());
        }
    }

    create_entity(ctx, base_url, headers, "SessionEvents", &json!(body))
}

pub fn managed_tools_enabled(tool_rows: &[Value]) -> String {
    if tool_rows.is_empty() {
        String::new()
    } else {
        DEFAULT_OPENPAW_TOOLS.to_string()
    }
}

pub fn message_blocks_json(text: &str) -> String {
    serde_json::to_string(&vec![json!({ "type": "text", "text": text })])
        .unwrap_or_else(|_| "[]".to_string())
}

pub fn content_string_to_text(raw: &str) -> String {
    if raw.trim().is_empty() {
        return String::new();
    }
    match serde_json::from_str::<Value>(raw) {
        Ok(value) => extract_text_from_value(&value),
        Err(_) => raw.to_string(),
    }
}

pub fn extract_text_from_value(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::Bool(boolean) => boolean.to_string(),
        Value::Number(number) => number.to_string(),
        Value::String(text) => text.clone(),
        Value::Array(items) => items
            .iter()
            .map(extract_text_from_value)
            .filter(|text| !text.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n\n"),
        Value::Object(map) => {
            if let Some(block_type) = map.get("type").and_then(Value::as_str) {
                match block_type {
                    "text" => {
                        return map
                            .get("text")
                            .map(extract_text_from_value)
                            .unwrap_or_default();
                    }
                    "thinking" => {
                        return map
                            .get("thinking")
                            .or_else(|| map.get("text"))
                            .map(extract_text_from_value)
                            .unwrap_or_default();
                    }
                    "tool_result" => {
                        return map
                            .get("content")
                            .map(extract_text_from_value)
                            .unwrap_or_default();
                    }
                    _ => {}
                }
            }

            for key in ["text", "thinking", "content", "summary", "result"] {
                if let Some(value) = map.get(key) {
                    let text = extract_text_from_value(value);
                    if !text.trim().is_empty() {
                        return text;
                    }
                }
            }

            serde_json::to_string(value).unwrap_or_default()
        }
    }
}

pub fn pending_user_prompt(events: &[Value], last_consumed_sequence: i64) -> (String, i64) {
    let mut ordered = events.to_vec();
    ordered.sort_by_key(|event| field_i64(event, &["Sequence", "sequence"]));

    let mut parts = Vec::new();
    let mut max_sequence = last_consumed_sequence;

    for event in ordered {
        let sequence = field_i64(&event, &["Sequence", "sequence"]);
        if sequence <= last_consumed_sequence {
            continue;
        }

        let kind = field_string(&event, &["Kind", "kind"]);
        let rendered = match kind.as_str() {
            "user.message" => content_string_to_text(&field_string(&event, &["Content", "content"])),
            "user.interrupt" => {
                let detail = content_string_to_text(&field_string(&event, &["Content", "content"]));
                if detail.is_empty() {
                    "The user interrupted the previous response.".to_string()
                } else {
                    format!("The user interrupted the previous response.\n\n{detail}")
                }
            }
            "user.tool_confirmation" => {
                let tool_use_id = field_string(&event, &["ToolUseId", "tool_use_id"]);
                let confirmation = field_string(
                    &event,
                    &["ConfirmationResult", "confirmation_result"],
                );
                let deny_message = field_string(&event, &["DenyMessage", "deny_message"]);
                if confirmation == "deny" && !deny_message.is_empty() {
                    format!(
                        "The user denied tool call {tool_use_id}. Reason: {deny_message}"
                    )
                } else {
                    format!("The user marked tool call {tool_use_id} as {confirmation}.")
                }
            }
            "user.custom_tool_result" => {
                let tool_id = field_string(
                    &event,
                    &["CustomToolUseId", "custom_tool_use_id"],
                );
                let tool_name = field_string(&event, &["ToolName", "tool_name"]);
                let content = content_string_to_text(&field_string(&event, &["Content", "content"]));
                let label = if tool_name.is_empty() { tool_id } else { format!("{tool_name} ({tool_id})") };
                format!("Custom tool result for {label}:\n\n{content}")
            }
            _ => String::new(),
        };

        if !rendered.trim().is_empty() {
            parts.push(rendered);
            max_sequence = max_sequence.max(sequence);
        }
    }

    (parts.join("\n\n"), max_sequence)
}

pub fn truncate(body: &str) -> String {
    body.chars().take(240).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_text_from_block_arrays() {
        let value = json!([
            { "type": "text", "text": "hello" },
            { "type": "thinking", "thinking": "reasoning" },
            { "type": "tool_result", "content": [{ "type": "text", "text": "done" }] }
        ]);

        assert_eq!(
            extract_text_from_value(&value),
            "hello\n\nreasoning\n\ndone"
        );
    }

    #[test]
    fn content_string_to_text_accepts_json_or_plain_text() {
        assert_eq!(
            content_string_to_text(r#"[{"type":"text","text":"hello"}]"#),
            "hello"
        );
        assert_eq!(content_string_to_text("plain"), "plain");
    }

    #[test]
    fn pending_user_prompt_renders_new_user_events() {
        let events = vec![
            json!({"Sequence": 2, "Kind": "user.message", "Content": r#"[{"type":"text","text":"second"}]"#}),
            json!({"Sequence": 1, "Kind": "user.message", "Content": r#"[{"type":"text","text":"first"}]"#}),
            json!({"Sequence": 3, "Kind": "user.tool_confirmation", "ToolUseId": "tool-1", "ConfirmationResult": "allow"}),
        ];

        let (prompt, last_seq) = pending_user_prompt(&events, 0);
        assert_eq!(
            prompt,
            "first\n\nsecond\n\nThe user marked tool call tool-1 as allow."
        );
        assert_eq!(last_seq, 3);
    }

    #[test]
    fn infers_provider_from_model_name() {
        assert_eq!(infer_provider("gpt-5.4"), "openai");
        assert_eq!(infer_provider("anthropic/claude-sonnet-4.6"), "openrouter");
        assert_eq!(infer_provider("claude-sonnet-4-6"), "anthropic");
    }
}
