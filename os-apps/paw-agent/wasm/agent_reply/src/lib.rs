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
        let agent_id = ctx.entity_id.as_str();
        let status = entity_field_str(&ctx.entity_state, &["Status", "status"]).unwrap_or("");
        let reply_text = build_reply_text(&fields, status);

        let Some(session) = find_session_by_agent(&ctx, &temper_api_url, tenant, agent_id)? else {
            ctx.log(
                "info",
                &format!("agent_reply: no channel session linked to agent {agent_id}; skipping"),
            );
            return Ok(());
        };

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

        let body = json!({
            "thread_id": thread_id,
            "content": reply_text,
            "agent_entity_id": agent_id,
        });
        let url =
            format!("{temper_api_url}/tdata/Channels('{channel_entity_id}')/Paw.Channel.SendReply");
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

fn build_reply_text(fields: &Value, status: &str) -> String {
    if let Some(result) = entity_field_str(fields, &["result", "Result"]).filter(|v| !v.is_empty())
    {
        return result.to_string();
    }
    if let Some(error) =
        entity_field_str(fields, &["error_message", "ErrorMessage", "error"]).filter(|v| !v.is_empty())
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

fn truncate_body(body: &str) -> String {
    const LIMIT: usize = 240;
    if body.len() <= LIMIT {
        body.to_string()
    } else {
        format!("{}...", &body[..LIMIT])
    }
}
