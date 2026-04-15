//! Shared helper functions for Agent WASM modules.
//!
//! Provides common TemperFS I/O, field extraction, URL resolution,
//! and sandbox provider abstraction to eliminate duplication across
//! WASM integration modules.

pub mod sandbox;

use temper_wasm_sdk::prelude::*;

const TEMPERFS_READ_ATTEMPTS: usize = 10;
const TEMPERFS_WRITE_ATTEMPTS: usize = 5;

fn read_temperfs_value_with_retry(
    ctx: &Context,
    url: &str,
    headers: &[(String, String)],
    label: &str,
) -> Result<String, String> {
    let mut last_status = 0;
    let mut last_body = String::new();

    for attempt in 0..TEMPERFS_READ_ATTEMPTS {
        let resp = ctx.http_call("GET", url, headers, "")?;
        if resp.status == 200 {
            return Ok(resp.body);
        }
        if resp.status == 404 {
            return Ok(String::new());
        }

        last_status = resp.status;
        last_body = resp.body;

        if (500..600).contains(&resp.status) && attempt + 1 < TEMPERFS_READ_ATTEMPTS {
            ctx.log(
                "warn",
                &format!(
                    "{label}: transient read failure (HTTP {}), retry {}/{}",
                    resp.status,
                    attempt + 2,
                    TEMPERFS_READ_ATTEMPTS
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

fn is_retriable_write_failure(status: u16, body: &str) -> bool {
    (500..600).contains(&status) || body.contains("BlobUploadFailed") || body.contains("HTTP -1")
}

pub fn write_temperfs_value_with_retry(
    ctx: &Context,
    url: &str,
    headers: &[(String, String)],
    body: &str,
    label: &str,
) -> Result<(), String> {
    let mut last_status = 0;
    let mut last_body = String::new();

    for attempt in 0..TEMPERFS_WRITE_ATTEMPTS {
        let resp = ctx.http_call("PUT", url, headers, body)?;
        if (200..300).contains(&resp.status) {
            return Ok(());
        }

        last_status = resp.status;
        last_body = resp.body;

        if is_retriable_write_failure(resp.status, &last_body)
            && attempt + 1 < TEMPERFS_WRITE_ATTEMPTS
        {
            ctx.log(
                "warn",
                &format!(
                    "{label}: transient write failure (HTTP {}), retry {}/{}",
                    resp.status,
                    attempt + 2,
                    TEMPERFS_WRITE_ATTEMPTS
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

/// Resolve the Temper API URL from entity fields or context config,
/// falling back to localhost.
pub fn resolve_temper_api_url(ctx: &Context, fields: &Value) -> String {
    fields
        .get("temper_api_url")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .or_else(|| {
            ctx.config
                .get("temper_api_url")
                .filter(|s| !s.is_empty())
                .cloned()
        })
        .unwrap_or_else(|| "http://127.0.0.1:3000".to_string())
}

/// Read session JSONL from TemperFS by file ID.
pub fn read_session_from_temperfs(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    fields: &Value,
    file_id: &str,
) -> Result<String, String> {
    let url = format!("{temper_api_url}/tdata/Files('{file_id}')/$value");
    let headers = runtime_headers(ctx, tenant, fields, None, None);
    read_temperfs_value_with_retry(ctx, &url, &headers, "TemperFS session read failed")
}

/// Write session JSONL to TemperFS by file ID.
pub fn write_session_to_temperfs(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    fields: &Value,
    file_id: &str,
    jsonl: &str,
) -> Result<(), String> {
    let url = format!("{temper_api_url}/tdata/Files('{file_id}')/$value");
    let headers = runtime_headers(ctx, tenant, fields, Some("text/plain"), None);
    write_temperfs_value_with_retry(ctx, &url, &headers, jsonl, "TemperFS session write failed")
}

/// Read raw file content from TemperFS by file ID.
pub fn read_content_file(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    fields: &Value,
    file_id: &str,
) -> Result<String, String> {
    let url = format!("{temper_api_url}/tdata/Files('{file_id}')/$value");
    let headers = runtime_headers(ctx, tenant, fields, None, None);
    read_temperfs_value_with_retry(ctx, &url, &headers, "TemperFS content file read failed")
}

/// Create a TemperFS file and write content into it.
pub fn create_content_file(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    workspace_id: &str,
    file_name: &str,
    content: &str,
) -> Result<String, String> {
    let headers = runtime_headers_with_workspace(
        ctx,
        tenant,
        &serde_json::json!({}),
        Some(workspace_id),
        None,
        Some("application/json"),
        None,
    );

    let file_body = serde_json::json!({
        "workspace_id": workspace_id,
        "name": file_name,
        "mime_type": "text/plain",
        "path": format!("/{file_name}")
    });
    let file_url = format!("{temper_api_url}/tdata/Files");
    let file_resp = ctx.http_call("POST", &file_url, &headers, &file_body.to_string())?;

    if file_resp.status < 200 || file_resp.status >= 300 {
        return Err(format!(
            "content file creation failed (HTTP {}): {}",
            file_resp.status,
            &file_resp.body[..file_resp.body.len().min(300)]
        ));
    }

    let file_parsed: Value = serde_json::from_str(&file_resp.body)
        .map_err(|e| format!("parse content file response: {e}"))?;
    let file_id = file_parsed
        .get("entity_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    if file_id.is_empty() {
        return Err("content file created but entity_id missing".to_string());
    }

    let value_url = format!("{temper_api_url}/tdata/Files('{file_id}')/$value");
    let value_headers = runtime_headers_with_workspace(
        ctx,
        tenant,
        &serde_json::json!({}),
        Some(workspace_id),
        None,
        Some("text/plain"),
        None,
    );
    write_temperfs_value_with_retry(
        ctx,
        &value_url,
        &value_headers,
        content,
        "content file write failed",
    )?;

    Ok(file_id)
}

/// Build standard OData headers for tenant-scoped requests.
pub fn odata_headers(ctx: &Context, tenant: &str, fields: &Value) -> Vec<(String, String)> {
    runtime_headers(
        ctx,
        tenant,
        fields,
        Some("application/json"),
        Some("application/json"),
    )
}

/// Build tenant-scoped runtime headers with an entity-derived principal.
pub fn runtime_headers(
    ctx: &Context,
    tenant: &str,
    fields: &Value,
    content_type: Option<&str>,
    accept: Option<&str>,
) -> Vec<(String, String)> {
    runtime_headers_with_workspace(ctx, tenant, fields, None, None, content_type, accept)
}

/// Build runtime headers while overriding the logical agent type.
pub fn runtime_headers_as(
    ctx: &Context,
    tenant: &str,
    fields: &Value,
    agent_type: &str,
    content_type: Option<&str>,
    accept: Option<&str>,
) -> Vec<(String, String)> {
    runtime_headers_with_workspace(
        ctx,
        tenant,
        fields,
        None,
        Some(agent_type),
        content_type,
        accept,
    )
}

/// Build runtime headers while explicitly supplying the workspace context.
pub fn runtime_headers_for_workspace(
    ctx: &Context,
    tenant: &str,
    fields: &Value,
    workspace_id: &str,
    content_type: Option<&str>,
    accept: Option<&str>,
) -> Vec<(String, String)> {
    runtime_headers_with_workspace(
        ctx,
        tenant,
        fields,
        Some(workspace_id),
        None,
        content_type,
        accept,
    )
}

fn runtime_headers_with_workspace(
    ctx: &Context,
    tenant: &str,
    fields: &Value,
    workspace_id_override: Option<&str>,
    agent_type_override: Option<&str>,
    content_type: Option<&str>,
    accept: Option<&str>,
) -> Vec<(String, String)> {
    let mut headers = Vec::new();
    headers.push(("x-tenant-id".to_string(), tenant.to_string()));
    headers.push(("x-temper-principal-kind".to_string(), "agent".to_string()));
    headers.push(("x-temper-principal-id".to_string(), ctx.entity_id.clone()));
    headers.push((
        "x-temper-agent-type".to_string(),
        agent_type_override
            .unwrap_or_else(|| default_agent_type(ctx))
            .to_string(),
    ));

    if let Some(content_type) = content_type {
        headers.push(("content-type".to_string(), content_type.to_string()));
    }
    if let Some(accept) = accept {
        headers.push(("accept".to_string(), accept.to_string()));
    }

    if let Some(soul_id) =
        entity_field_str(fields, &["soul_id", "SoulId"]).filter(|v| !v.is_empty())
    {
        headers.push(("x-temper-attr-soul_id".to_string(), soul_id.to_string()));
    }

    let workspace_id = workspace_id_override
        .filter(|value| !value.is_empty())
        .or_else(|| {
            entity_field_str(fields, &["workspace_id", "WorkspaceId"])
                .filter(|value| !value.is_empty())
        });
    if let Some(workspace_id) = workspace_id {
        headers.push((
            "x-temper-attr-workspaceid".to_string(),
            workspace_id.to_string(),
        ));
    }

    if let Some(key) = ctx.config.get("temper_api_key").filter(|k| !k.is_empty()) {
        headers.push(("authorization".to_string(), format!("Bearer {key}")));
    }

    headers
}

/// Derive agent type from entity type for WASM modules that construct their own headers.
/// Session integrations act on behalf of an agent; all others are platform-internal ("system")
/// dispatch callbacks. For modules covered by host-injected auth (ADR-0043), this is unused —
/// the host derives agent_type from WasmInvocationContext.entity_type directly.
fn default_agent_type(ctx: &Context) -> &'static str {
    if ctx.entity_type.eq_ignore_ascii_case("Session")
        || ctx.entity_type.eq_ignore_ascii_case("Sessions")
    {
        "agent"
    } else {
        "system"
    }
}

/// Look up a string field directly on a JSON value, trying multiple key names.
pub fn direct_field_str<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
}

/// Look up a string field on a JSON value, falling back to nested `fields` object.
pub fn entity_field_str<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a str> {
    direct_field_str(value, keys).or_else(|| {
        value
            .get("fields")
            .and_then(|fields| direct_field_str(fields, keys))
    })
}

/// List entities returned by an OData collection URL.
pub fn list_entities(ctx: &Context, url: &str, tenant: &str) -> Result<Vec<Value>, String> {
    let fields = ctx
        .entity_state
        .get("fields")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let headers = runtime_headers(ctx, tenant, &fields, None, Some("application/json"));
    let resp = ctx.http_call("GET", url, &headers, "")?;
    if resp.status != 200 {
        return Err(format!("GET {url} failed (HTTP {})", resp.status));
    }
    let parsed: Value = serde_json::from_str(&resp.body).unwrap_or_else(|_| json!({"value": []}));
    Ok(parsed
        .get("value")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default())
}

/// Find the best available ChannelSession for a bound agent.
pub fn find_channel_session_by_agent(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    agent_id: &str,
) -> Result<Option<Value>, String> {
    let escaped = agent_id.replace('\'', "''");
    let active_filter =
        format!("$filter=Status eq 'Active' and agent_entity_id eq '{escaped}'&$top=1");
    let active_url = format!("{temper_api_url}/tdata/ChannelSessions?{active_filter}");
    if let Some(session) = list_entities(ctx, &active_url, tenant)?.into_iter().next() {
        return Ok(Some(session));
    }

    let any_filter = format!("$filter=agent_entity_id eq '{escaped}'&$top=1");
    let any_url = format!("{temper_api_url}/tdata/ChannelSessions?{any_filter}");
    Ok(list_entities(ctx, &any_url, tenant)?.into_iter().next())
}

/// Find the connected Channel entity for a platform-specific channel ID.
pub fn find_connected_channel_by_external_id(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    channel_id: &str,
) -> Result<Option<Value>, String> {
    let escaped = channel_id.replace('\'', "''");
    let filter = format!("$filter=Status eq 'Connected' and channel_id eq '{escaped}'&$top=1");
    let url = format!("{temper_api_url}/tdata/Channels?{filter}");
    Ok(list_entities(ctx, &url, tenant)?.into_iter().next())
}

/// Parse a basic ISO 8601 timestamp (YYYY-MM-DDTHH:MM:SSZ) to Unix epoch seconds.
/// Returns None if the format is unrecognized.
pub fn parse_iso8601_to_epoch_secs(s: &str) -> Option<u64> {
    // Supported formats: "2026-03-24T12:30:00Z", "2026-03-24T12:30:00.000Z"
    let s = s.trim();
    if s.len() < 19 {
        return None;
    }

    let year: u64 = s.get(0..4)?.parse().ok()?;
    let month: u64 = s.get(5..7)?.parse().ok()?;
    let day: u64 = s.get(8..10)?.parse().ok()?;
    let hour: u64 = s.get(11..13)?.parse().ok()?;
    let minute: u64 = s.get(14..16)?.parse().ok()?;
    let second: u64 = s.get(17..19)?.parse().ok()?;

    if s.as_bytes().get(4) != Some(&b'-')
        || s.as_bytes().get(7) != Some(&b'-')
        || s.as_bytes().get(10) != Some(&b'T')
    {
        return None;
    }

    // Days in each month (non-leap)
    let days_in_month = [0, 31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let is_leap = (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0);

    // Days from epoch (1970-01-01) to start of `year`
    let mut days: u64 = 0;
    for y in 1970..year {
        let leap = (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0);
        days += if leap { 366 } else { 365 };
    }

    // Days from start of year to start of month
    for m in 1..month {
        days += days_in_month[m as usize];
        if m == 2 && is_leap {
            days += 1;
        }
    }

    // Days within month (1-indexed)
    days += day - 1;

    Some(days * 86400 + hour * 3600 + minute * 60 + second)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_iso8601() {
        // 2026-03-24T12:00:00Z
        let secs = parse_iso8601_to_epoch_secs("2026-03-24T12:00:00Z");
        assert!(secs.is_some());
        let s = secs.unwrap();
        // Rough sanity: should be > 2025-01-01 (~1735689600) and < 2027-01-01
        assert!(s > 1_735_000_000);
        assert!(s < 1_800_000_000);
    }

    #[test]
    fn test_parse_iso8601_with_millis() {
        let secs = parse_iso8601_to_epoch_secs("2026-03-24T12:00:00.123Z");
        assert!(secs.is_some());
    }

    #[test]
    fn test_parse_iso8601_invalid() {
        assert!(parse_iso8601_to_epoch_secs("").is_none());
        assert!(parse_iso8601_to_epoch_secs("not-a-date").is_none());
        assert!(parse_iso8601_to_epoch_secs("2026").is_none());
    }

    #[test]
    fn test_epoch_zero() {
        let secs = parse_iso8601_to_epoch_secs("1970-01-01T00:00:00Z");
        assert_eq!(secs, Some(0));
    }

    #[test]
    fn test_direct_field_str() {
        let val = serde_json::json!({"Name": "test", "id": "123"});
        assert_eq!(direct_field_str(&val, &["Name"]), Some("test"));
        assert_eq!(direct_field_str(&val, &["missing", "id"]), Some("123"));
        assert_eq!(direct_field_str(&val, &["missing"]), None);
    }

    #[test]
    fn test_entity_field_str() {
        let val = serde_json::json!({"fields": {"Status": "Active"}});
        assert_eq!(entity_field_str(&val, &["Status"]), Some("Active"));
    }
}

/// Send a typing indicator to Discord for the given agent.
/// Best-effort: silently ignores all errors.
///
/// Looks up ChannelSession → Channel → webhook_url, then POSTs to /typing.
pub fn send_typing_indicator(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    agent_entity_id: &str,
) {
    let _ = send_typing_indicator_inner(ctx, temper_api_url, tenant, agent_entity_id);
}

fn send_typing_indicator_inner(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    agent_entity_id: &str,
) -> Result<(), String> {
    if agent_entity_id.is_empty() {
        return Ok(());
    }

    let fields = ctx
        .entity_state
        .get("fields")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let headers = runtime_headers(ctx, tenant, &fields, None, Some("application/json"));
    let parent_session_id =
        entity_field_str(&ctx.entity_state, &["parent_session_id", "ParentSessionId"])
            .unwrap_or("");

    // Find ChannelSession for this agent
    let session = find_channel_session(ctx, temper_api_url, &headers, agent_entity_id)
        .or_else(|| {
            if parent_session_id.is_empty() || parent_session_id == agent_entity_id {
                None
            } else {
                find_channel_session(ctx, temper_api_url, &headers, parent_session_id)
            }
        })
        .ok_or("no session")?;

    let channel_id = entity_field_str(&session, &["ChannelId", "channel_id"]).unwrap_or("");
    let thread_id = entity_field_str(&session, &["ThreadId", "thread_id"]).unwrap_or("");
    if channel_id.is_empty() || thread_id.is_empty() {
        return Ok(());
    }

    // Find Channel to get webhook_url
    let escaped_ch = channel_id.replace('\'', "''");
    let channel_url = format!(
        "{temper_api_url}/tdata/Channels?$filter=Status eq 'Connected' and channel_id eq '{escaped_ch}'&$top=1"
    );
    let ch_resp = ctx.http_call("GET", &channel_url, &headers, "")?;
    if ch_resp.status != 200 {
        return Ok(());
    }
    let channels: Value =
        serde_json::from_str(&ch_resp.body).unwrap_or_else(|_| json!({"value": []}));
    let channel = channels
        .get("value")
        .and_then(Value::as_array)
        .and_then(|arr| arr.first())
        .ok_or("no channel")?;

    let webhook_url = entity_field_str(channel, &["webhook_url", "WebhookUrl"]).unwrap_or("");
    if webhook_url.is_empty() {
        return Ok(());
    }

    // POST to /typing endpoint
    let typing_url = format!(
        "{}/typing",
        webhook_url.trim_end_matches('/').trim_end_matches("/reply")
    );
    let body = json!({"thread_id": thread_id});
    let wh_headers = vec![("content-type".to_string(), "application/json".to_string())];
    let _ = ctx.http_call("POST", &typing_url, &wh_headers, &body.to_string());
    Ok(())
}

fn find_channel_session(
    ctx: &Context,
    temper_api_url: &str,
    headers: &[(String, String)],
    agent_entity_id: &str,
) -> Option<Value> {
    let escaped = agent_entity_id.replace('\'', "''");
    let active_url = format!(
        "{temper_api_url}/tdata/ChannelSessions?$filter=Status eq 'Active' and agent_entity_id eq '{escaped}'&$top=1"
    );
    let active_resp = ctx.http_call("GET", &active_url, headers, "").ok()?;
    if active_resp.status == 200 {
        let sessions: Value =
            serde_json::from_str(&active_resp.body).unwrap_or_else(|_| json!({"value": []}));
        if let Some(session) = sessions
            .get("value")
            .and_then(Value::as_array)
            .and_then(|arr| arr.first())
        {
            return Some(session.clone());
        }
    }

    let fallback_url = format!(
        "{temper_api_url}/tdata/ChannelSessions?$filter=agent_entity_id eq '{escaped}'&$top=1"
    );
    let fallback_resp = ctx.http_call("GET", &fallback_url, headers, "").ok()?;
    if fallback_resp.status != 200 {
        return None;
    }
    let sessions: Value =
        serde_json::from_str(&fallback_resp.body).unwrap_or_else(|_| json!({"value": []}));
    sessions
        .get("value")
        .and_then(Value::as_array)
        .and_then(|arr| arr.first())
        .cloned()
}
