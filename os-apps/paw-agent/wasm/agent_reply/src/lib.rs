use temper_wasm_sdk::prelude::*;
use wasm_helpers::{entity_field_str, resolve_temper_api_url, runtime_headers};

#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        let fields = ctx
            .entity_state
            .get("fields")
            .cloned()
            .unwrap_or_else(|| json!({}));
        let temper_api_url = resolve_temper_api_url(&ctx, &fields);
        let tenant = &ctx.tenant;
        // Use the persistent Agent entity ID from Session fields, not the Session's own ID.
        // ChannelSessions store agent_entity_id = aj-..., not ss-...
        let session_id = ctx.entity_id.as_str();
        let agent_id = entity_field_str(&fields, &["agent_id", "AgentId"])
            .unwrap_or(session_id);
        let status = entity_field_str(&ctx.entity_state, &["Status", "status"]).unwrap_or("");
        let parent_session_id =
            entity_field_str(&ctx.entity_state, &["parent_session_id", "ParentSessionId"])
                .unwrap_or("");
        let reply_text = build_reply_text(&ctx.entity_state, status);

        let Some((session, bound_agent_id)) =
            find_session_by_agent(&ctx, &temper_api_url, tenant, agent_id, parent_session_id)?
        else {
            ctx.log(
                "info",
                &format!("agent_reply: no channel session linked to agent {agent_id}; skipping"),
            );
            set_success_result(
                "",
                &json!({
                    "status": "skipped",
                    "reason": "no_channel_session",
                    "agent_entity_id": agent_id,
                }),
            );
            return Ok(());
        };
        if bound_agent_id != agent_id {
            ctx.log(
                "info",
                &format!(
                    "agent_reply: using parent channel session {bound_agent_id} for agent {agent_id}"
                ),
            );
        }

        let channel_id = entity_field_str(&session, &["ChannelId", "channel_id"]).unwrap_or("");
        let thread_id = entity_field_str(&session, &["ThreadId", "thread_id"]).unwrap_or("");
        if channel_id.is_empty() || thread_id.is_empty() {
            return Err("agent_reply: channel session missing channel_id/thread_id".to_string());
        }

        let channel = find_channel_by_external_id(&ctx, &temper_api_url, tenant, channel_id)?
            .ok_or_else(|| {
                format!("agent_reply: no connected Channel entity found for channel_id={channel_id}")
            })?;
        let channel_entity_id = entity_field_str(&channel, &["Id", "entity_id", "id"])
            .ok_or_else(|| "agent_reply: resolved Channel missing entity id".to_string())?;

        let mut headers = runtime_headers(
            &ctx,
            tenant,
            &fields,
            Some("application/json"),
            Some("application/json"),
        );
        headers.push((
            "x-temper-attr-channelid".to_string(),
            channel_id.to_string(),
        ));

        // Only treat as a child agent if agent_depth > 0 (explicitly spawned).
        // Session-resumed sessions have parent_session_id set but aren't children.
        let agent_depth = entity_field_str(&fields, &["session_depth", "SessionDepth"])
            .and_then(|v| v.parse::<i64>().ok())
            .unwrap_or(0);
        let is_child = agent_depth > 0;

        // For child agents, send an embed directly to the webhook (bypasses
        // Channel.SendReply which doesn't support embeds in its IOA fields).
        // For parent agents, use the normal Channel.SendReply flow.
        if is_child {
            let soul_id = entity_field_str(&fields, &["soul_id", "SoulId"]).unwrap_or("");
            let soul_name = if !soul_id.is_empty() {
                resolve_soul_name(&ctx, &temper_api_url, tenant, soul_id)
                    .unwrap_or_else(|| soul_id.to_string())
            } else {
                "Session".to_string()
            };
            let needs_follow_up = reply_text.len() > 3900;
            let desc = if needs_follow_up {
                format!(
                    "{}... (full reply in follow-up)",
                    &reply_text[..reply_text.floor_char_boundary(3900)]
                )
            } else {
                reply_text.clone()
            };
            let color = if status == "Completed" { 0x57F287u32 } else { 0xED4245u32 };

            // Get webhook_url from the Channel entity
            let webhook_url =
                entity_field_str(&channel, &["webhook_url", "WebhookUrl"]).unwrap_or("");
            if !webhook_url.is_empty() {
                let embed_body = json!({
                    "thread_id": thread_id,
                    "content": "",
                    "agent_entity_id": agent_id,
                    "embeds": [{
                        "title": soul_name,
                        "description": desc,
                        "color": color,
                        "footer": {"text": format!("Agent:{}", &agent_id[..agent_id.len().min(12)])}
                    }]
                });
                let wh_headers = vec![
                    ("content-type".to_string(), "application/json".to_string()),
                    ("x-tenant-id".to_string(), tenant.to_string()),
                ];
                let resp = ctx.http_call("POST", webhook_url, &wh_headers, &embed_body.to_string())?;
                if !(200..300).contains(&resp.status) {
                    return Err(format!(
                        "agent_reply: embed webhook POST failed (HTTP {})",
                        resp.status
                    ));
                }
                if needs_follow_up {
                    let follow_up_body = json!({
                        "thread_id": thread_id,
                        "content": reply_text,
                        "agent_entity_id": agent_id,
                    });
                    let follow_up_resp =
                        ctx.http_call("POST", webhook_url, &wh_headers, &follow_up_body.to_string())?;
                    if !(200..300).contains(&follow_up_resp.status) {
                        return Err(format!(
                            "agent_reply: follow-up webhook POST failed (HTTP {})",
                            follow_up_resp.status
                        ));
                    }
                }
                ctx.log(
                    "info",
                    &format!("agent_reply: sent embed reply for child agent {agent_id} ({soul_name})"),
                );
                set_success_result("", &json!({"status": "replied_embed", "agent_entity_id": agent_id}));
                return Ok(());
            }
        }

        let body = json!({
            "thread_id": thread_id,
            "content": reply_text,
            "agent_entity_id": agent_id,
        });
        let url = format!(
            "{temper_api_url}/tdata/Channels('{channel_entity_id}')/Paw.Channel.SendReply?await_integration=true"
        );
        let resp = ctx.http_call("POST", &url, &headers, &body.to_string())?;
        if !(200..300).contains(&resp.status) {
            return Err(format!(
                "agent_reply: Channel.SendReply failed (HTTP {}): {}",
                resp.status,
                truncate_body(&resp.body)
            ));
        }

        ctx.log(
            "info",
            &format!("agent_reply: dispatched reply for agent {agent_id} to thread {thread_id}"),
        );
        set_success_result(
            "",
            &json!({
                "status": "replied",
                "thread_id": thread_id,
                "agent_entity_id": agent_id,
            }),
        );
        Ok(())
    })();

    if let Err(error) = result {
        set_error_result(&error);
    }
    0
}

fn build_reply_text(entity_state: &Value, status: &str) -> String {
    if let Some(events) = entity_state.get("events").and_then(Value::as_array) {
        for event in events.iter().rev() {
            if let Some(result) = event
                .get("params")
                .and_then(|params| entity_field_str(params, &["result", "Result"]))
                .filter(|value| !value.is_empty())
            {
                return result.to_string();
            }
            if let Some(error) = event
                .get("params")
                .and_then(|params| {
                    entity_field_str(params, &["error_message", "ErrorMessage", "error"])
                })
                .filter(|value| !value.is_empty())
            {
                return error.to_string();
            }
        }
    }

    let fields = entity_state
        .get("fields")
        .cloned()
        .unwrap_or_else(|| json!({}));
    if let Some(result) = entity_field_str(&fields, &["result", "Result"]).filter(|v| !v.is_empty())
    {
        return result.to_string();
    }
    if let Some(error) = entity_field_str(&fields, &["error_message", "ErrorMessage", "error"])
        .filter(|v| !v.is_empty())
    {
        return error.to_string();
    }
    match status {
        "Cancelled" => "Agent was cancelled before it could produce a reply.".to_string(),
        "Failed" => "Agent failed before it could produce a reply.".to_string(),
        "Completed" => "Agent completed without returning any reply text.".to_string(),
        other if !other.is_empty() => format!("Agent ended with status={other}."),
        _ => "Agent finished without a reply.".to_string(),
    }
}

fn find_session_by_agent(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    agent_id: &str,
    parent_session_id: &str,
) -> Result<Option<(Value, String)>, String> {
    if let Some(session) = find_session_for_binding(ctx, temper_api_url, tenant, agent_id)? {
        return Ok(Some((session, agent_id.to_string())));
    }

    let parent_session_id = parent_session_id.trim();
    if !parent_session_id.is_empty() && parent_session_id != agent_id {
        if let Some(session) =
            find_session_for_binding(ctx, temper_api_url, tenant, parent_session_id)?
        {
            return Ok(Some((session, parent_session_id.to_string())));
        }
    }

    Ok(None)
}

fn find_session_for_binding(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    agent_id: &str,
) -> Result<Option<Value>, String> {
    let escaped_agent = escape_odata(agent_id);
    let active_filter =
        format!("$filter=Status eq 'Active' and agent_entity_id eq '{escaped_agent}'&$top=1");
    let active_url = format!("{temper_api_url}/tdata/ChannelSessions?{active_filter}");
    if let Some(session) = list_entities(ctx, &active_url, tenant)?.into_iter().next() {
        return Ok(Some(session));
    }

    let any_filter = format!("$filter=agent_entity_id eq '{escaped_agent}'&$top=1");
    let any_url = format!("{temper_api_url}/tdata/ChannelSessions?{any_filter}");
    Ok(list_entities(ctx, &any_url, tenant)?.into_iter().next())
}

fn find_channel_by_external_id(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    channel_id: &str,
) -> Result<Option<Value>, String> {
    let escaped_channel = escape_odata(channel_id);
    let connected_filter =
        format!("$filter=Status eq 'Connected' and channel_id eq '{escaped_channel}'&$top=1");
    let connected_url = format!("{temper_api_url}/tdata/Channels?{connected_filter}");
    if let Some(channel) = list_entities(ctx, &connected_url, tenant)?.into_iter().next() {
        return Ok(Some(channel));
    }

    let any_filter = format!("$filter=channel_id eq '{escaped_channel}'&$top=1");
    let any_url = format!("{temper_api_url}/tdata/Channels?{any_filter}");
    Ok(list_entities(ctx, &any_url, tenant)?.into_iter().next())
}

fn list_entities(ctx: &Context, url: &str, tenant: &str) -> Result<Vec<Value>, String> {
    let fields = ctx
        .entity_state
        .get("fields")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let headers = runtime_headers(ctx, tenant, &fields, None, Some("application/json"));
    let resp = ctx.http_call("GET", url, &headers, "")?;
    if resp.status == 404 {
        return Ok(Vec::new());
    }
    if resp.status != 200 {
        return Err(format!("agent_reply: GET {url} failed (HTTP {})", resp.status));
    }
    let parsed: Value = serde_json::from_str(&resp.body).unwrap_or_else(|_| json!({ "value": [] }));
    Ok(parsed
        .get("value")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default())
}

fn escape_odata(value: &str) -> String {
    value.replace('\'', "''")
}

fn resolve_soul_name(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    soul_ref: &str,
) -> Option<String> {
    if soul_ref.is_empty() {
        return None;
    }
    let fields = ctx
        .entity_state
        .get("fields")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let headers = runtime_headers(ctx, tenant, &fields, None, Some("application/json"));

    // Try by ID first
    let url = format!("{temper_api_url}/tdata/Souls('{soul_ref}')");
    if let Ok(resp) = ctx.http_call("GET", &url, &headers, "") {
        if resp.status == 200 {
            let parsed: Value = serde_json::from_str(&resp.body).unwrap_or_default();
            if let Some(name) = entity_field_str(&parsed, &["Name", "name"]) {
                return Some(name.to_string());
            }
        }
    }

    // Try by name filter
    let escaped = soul_ref.replace('\'', "''");
    let url = format!(
        "{temper_api_url}/tdata/Souls?$filter=Name eq '{escaped}' and Status eq 'Active'&$top=1"
    );
    if let Ok(resp) = ctx.http_call("GET", &url, &headers, "") {
        if resp.status == 200 {
            let parsed: Value = serde_json::from_str(&resp.body).unwrap_or_default();
            if let Some(soul) = parsed.get("value").and_then(Value::as_array).and_then(|a| a.first()) {
                if let Some(name) = entity_field_str(soul, &["Name", "name"]) {
                    return Some(name.to_string());
                }
            }
        }
    }

    // soul_ref itself might be the name
    Some(soul_ref.to_string())
}

fn truncate_body(body: &str) -> String {
    const LIMIT: usize = 240;
    if body.len() <= LIMIT {
        body.to_string()
    } else {
        format!("{}...", &body[..LIMIT])
    }
}
