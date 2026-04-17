use std::collections::{BTreeMap, BTreeSet};

use temper_wasm_sdk::prelude::*;
use wasm_helpers::{entity_field_str, runtime_headers_as};

pub const MANAGED_NAMESPACE: &str = "ManagedAgents";
pub const SAFE_DEFAULT_OPENPAW_TOOLS: &[&str] = &[
    "bash",
    "edit",
    "read",
    "temper_get",
    "temper_list",
    "temper_read",
    "temper_write",
    "temper_web_fetch",
    "temper_web_search",
    "write",
];
pub const MANAGED_AGENT_ALLOWED_TOOLS: &[&str] = &[
    "bash",
    "edit",
    "read",
    "temper_action",
    "temper_get",
    "temper_list",
    "temper_patch",
    "temper_read",
    "temper_recall_memory",
    "temper_save_memory",
    "temper_steer_session",
    "temper_web_fetch",
    "temper_web_search",
    "temper_write",
    "write",
];

pub fn system_json_headers(ctx: &Context, tenant: &str, fields: &Value) -> Vec<(String, String)> {
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

pub fn field_bool(value: &Value, keys: &[&str]) -> bool {
    for key in keys {
        if let Some(raw) = value.get(*key) {
            if let Some(boolean) = raw.as_bool() {
                return boolean;
            }
            if let Some(text) = raw.as_str() {
                match text.trim().to_ascii_lowercase().as_str() {
                    "true" => return true,
                    "false" => return false,
                    _ => {}
                }
            }
        }
    }
    if let Some(fields) = value.get("fields") {
        return field_bool(fields, keys);
    }
    false
}

pub fn status_of(value: &Value) -> String {
    field_string(value, &["Status", "status"])
}

pub fn is_terminal_status(status: &str) -> bool {
    matches!(
        status,
        "Completed" | "Failed" | "Cancelled" | "Archived" | "Destroyed"
    )
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

pub fn managed_agent_provider(managed_agent: &Value) -> String {
    let metadata = field_string(managed_agent, &["Metadata", "metadata"]);
    if !metadata.trim().is_empty() {
        if let Ok(parsed) = serde_json::from_str::<Value>(&metadata) {
            if let Some(provider) = json_string(&parsed, &["provider", "llm_provider"])
                .split_whitespace()
                .next()
                .map(str::to_ascii_lowercase)
                .filter(|value| {
                    matches!(
                        value.as_str(),
                        "anthropic" | "openai" | "openai_codex" | "openrouter" | "mock"
                    )
                })
            {
                return provider;
            }
        }
    }

    infer_provider(&field_string(managed_agent, &["ModelId", "model_id"])).to_string()
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

pub fn patch_entity(
    ctx: &Context,
    base_url: &str,
    headers: &[(String, String)],
    entity_set: &str,
    id: &str,
    body: &Value,
) -> Result<Value, String> {
    http_json(
        ctx,
        "PATCH",
        &format!("{base_url}/tdata/{entity_set}('{id}')"),
        headers,
        body,
        &format!("patch {entity_set}('{id}')"),
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
    let mut url =
        format!("{base_url}/tdata/{entity_set}('{entity_id}')/{MANAGED_NAMESPACE}.{action}");
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

fn is_managed_agent_toolset(kind: &str) -> bool {
    matches!(kind, "agent_toolset_20260401" | "agent_toolset")
}

fn normalize_managed_tool_name(name: &str) -> Option<String> {
    let normalized = name.trim();
    if normalized.is_empty() {
        return None;
    }
    MANAGED_AGENT_ALLOWED_TOOLS
        .iter()
        .find(|allowed| **allowed == normalized)
        .map(|allowed| (*allowed).to_string())
}

pub fn managed_tools_enabled(tool_rows: &[Value], config_rows: &[Value]) -> String {
    let mut enabled = BTreeSet::new();

    for tool_row in tool_rows {
        let kind = field_string(tool_row, &["Kind", "kind"]);
        if !is_managed_agent_toolset(&kind) {
            continue;
        }

        let tool_id = entity_id(tool_row).unwrap_or_default();
        let explicit = config_rows
            .iter()
            .filter(|config| field_string(config, &["ToolId", "tool_id"]) == tool_id)
            .filter_map(|config| {
                normalize_managed_tool_name(&field_string(config, &["ToolName", "tool_name"]))
            })
            .collect::<BTreeSet<_>>();

        if explicit.is_empty() {
            enabled.extend(
                SAFE_DEFAULT_OPENPAW_TOOLS
                    .iter()
                    .map(|tool| (*tool).to_string()),
            );
        } else {
            enabled.extend(explicit);
        }
    }

    enabled.into_iter().collect::<Vec<_>>().join(",")
}

pub fn managed_environment_sandbox_params(
    managed_environment: &Value,
    package_rows: &[Value],
) -> Value {
    let metadata = parsed_environment_metadata(&field_string(
        managed_environment,
        &["Metadata", "metadata"],
    ));
    let allowed_hosts_json = {
        let raw = field_string(
            managed_environment,
            &["AllowedHostsJson", "allowed_hosts_json"],
        );
        if raw.is_empty() {
            "[]".to_string()
        } else {
            raw
        }
    };
    let packages = package_rows
        .iter()
        .map(|package| {
            json!({
                "manager": field_string(package, &["Manager", "manager"]),
                "name": field_string(package, &["Name", "name"]),
                "version": field_string(package, &["Version", "version"]),
            })
        })
        .filter(|package| {
            package
                .get("manager")
                .and_then(Value::as_str)
                .is_some_and(|value| !value.is_empty())
                && package
                    .get("name")
                    .and_then(Value::as_str)
                    .is_some_and(|value| !value.is_empty())
        })
        .collect::<Vec<_>>();

    let mut params = serde_json::Map::new();
    params.insert(
        "sandbox_networking_type".to_string(),
        json!(field_string(
            managed_environment,
            &["NetworkingType", "networking_type"]
        )),
    );
    params.insert(
        "sandbox_allowed_hosts_json".to_string(),
        json!(allowed_hosts_json),
    );
    params.insert(
        "sandbox_allow_mcp_servers".to_string(),
        json!(field_bool(
            managed_environment,
            &["AllowMcpServers", "allow_mcp_servers"]
        )),
    );
    params.insert(
        "sandbox_allow_package_managers".to_string(),
        json!(field_bool(
            managed_environment,
            &["AllowPackageManagers", "allow_package_managers"]
        )),
    );
    params.insert(
        "sandbox_packages_json".to_string(),
        json!(serde_json::to_string(&packages).unwrap_or_else(|_| "[]".to_string())),
    );

    if !metadata.sandbox_provider.is_empty() {
        params.insert(
            "sandbox_provider".to_string(),
            json!(metadata.sandbox_provider),
        );
    }
    if !metadata.sandbox_url.is_empty() {
        params.insert("sandbox_url".to_string(), json!(metadata.sandbox_url));
    }
    if !metadata.sandbox_id.is_empty() {
        params.insert("sandbox_id".to_string(), json!(metadata.sandbox_id));
    }

    Value::Object(params)
}

#[derive(Default)]
struct EnvironmentMetadata {
    sandbox_provider: String,
    sandbox_url: String,
    sandbox_id: String,
}

fn parsed_environment_metadata(raw: &str) -> EnvironmentMetadata {
    if raw.trim().is_empty() {
        return EnvironmentMetadata::default();
    }
    let Ok(parsed) = serde_json::from_str::<Value>(raw) else {
        return EnvironmentMetadata::default();
    };
    EnvironmentMetadata {
        sandbox_provider: json_string(&parsed, &["sandbox_provider", "sandboxProvider"]),
        sandbox_url: json_string(&parsed, &["sandbox_url", "sandboxUrl"]),
        sandbox_id: json_string(&parsed, &["sandbox_id", "sandboxId"]),
    }
}

fn json_string(value: &Value, keys: &[&str]) -> String {
    for key in keys {
        if let Some(text) = value.get(*key).and_then(Value::as_str) {
            return text.to_string();
        }
    }
    String::new()
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
            "user.message" => {
                content_string_to_text(&field_string(&event, &["Content", "content"]))
            }
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
                let confirmation =
                    field_string(&event, &["ConfirmationResult", "confirmation_result"]);
                let deny_message = field_string(&event, &["DenyMessage", "deny_message"]);
                if confirmation == "deny" && !deny_message.is_empty() {
                    format!("The user denied tool call {tool_use_id}. Reason: {deny_message}")
                } else {
                    format!("The user marked tool call {tool_use_id} as {confirmation}.")
                }
            }
            "user.custom_tool_result" => {
                let tool_id = field_string(&event, &["CustomToolUseId", "custom_tool_use_id"]);
                let tool_name = field_string(&event, &["ToolName", "tool_name"]);
                let content =
                    content_string_to_text(&field_string(&event, &["Content", "content"]));
                let label = if tool_name.is_empty() {
                    tool_id
                } else {
                    format!("{tool_name} ({tool_id})")
                };
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

pub fn trigger_string(params: &Value, keys: &[&str]) -> String {
    for key in keys {
        if let Some(raw) = params.get(*key) {
            if let Some(text) = raw.as_str() {
                return text.to_string();
            }
        }
    }
    String::new()
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

    #[test]
    fn managed_agent_provider_honors_metadata_override() {
        let managed_agent = json!({
            "ModelId": "claude-sonnet-4-6",
            "Metadata": r#"{"provider":"mock"}"#,
        });

        assert_eq!(managed_agent_provider(&managed_agent), "mock");
    }

    #[test]
    fn managed_tools_enabled_uses_explicit_agent_tool_config_rows() {
        let tool_rows = vec![json!({
            "entity_id": "toolset-1",
            "fields": {
                "Kind": "agent_toolset_20260401"
            }
        })];
        let config_rows = vec![
            json!({
                "fields": {
                    "ToolId": "toolset-1",
                    "ToolName": "bash",
                    "PermissionPolicy": "always_allow"
                }
            }),
            json!({
                "fields": {
                    "ToolId": "toolset-1",
                    "ToolName": "temper_get",
                    "PermissionPolicy": "always_ask"
                }
            }),
        ];

        assert_eq!(
            managed_tools_enabled(&tool_rows, &config_rows),
            "bash,temper_get"
        );
    }

    #[test]
    fn managed_tools_enabled_defaults_to_safe_subset() {
        let tool_rows = vec![json!({
            "entity_id": "toolset-1",
            "fields": {
                "Kind": "agent_toolset_20260401"
            }
        })];

        let enabled = managed_tools_enabled(&tool_rows, &[]);
        assert!(enabled.contains("temper_get"));
        assert!(enabled.contains("bash"));
        assert!(!enabled.contains("temper_approve_decision"));
        assert!(!enabled.contains("temper_deny_decision"));
        assert!(!enabled.contains("temper_delete_policy"));
    }

    #[test]
    fn managed_environment_sandbox_params_include_template_settings_and_packages() {
        let managed_environment = json!({
            "NetworkingType": "Limited",
            "AllowedHostsJson": "[\"github.com\"]",
            "AllowMcpServers": true,
            "AllowPackageManagers": false,
            "Metadata": r#"{"sandbox_provider":"modal","sandbox_url":"https://sandbox.example","sandbox_id":"sb-123"}"#,
        });
        let package_rows = vec![
            json!({ "Manager": "apt", "Name": "jq", "Version": "1.7" }),
            json!({ "Manager": "pip", "Name": "rich", "Version": "13.9.4" }),
        ];

        let params = managed_environment_sandbox_params(&managed_environment, &package_rows);
        assert_eq!(params["sandbox_networking_type"], "Limited");
        assert_eq!(params["sandbox_allowed_hosts_json"], "[\"github.com\"]");
        assert_eq!(params["sandbox_allow_mcp_servers"], true);
        assert_eq!(params["sandbox_allow_package_managers"], false);
        assert_eq!(params["sandbox_provider"], "modal");
        assert_eq!(params["sandbox_url"], "https://sandbox.example");
        assert_eq!(params["sandbox_id"], "sb-123");
        assert_eq!(
            params["sandbox_packages_json"],
            r#"[{"manager":"apt","name":"jq","version":"1.7"},{"manager":"pip","name":"rich","version":"13.9.4"}]"#
        );
    }
}
