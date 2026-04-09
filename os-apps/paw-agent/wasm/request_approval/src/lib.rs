//! notify_approval_needed WASM — sends Discord buttons when an agent
//! is paused for Cedar approval.
//!
//! Triggered by Agent.PauseForApproval. Reads the pending_decision_id
//! from the agent's state, finds the Discord channel via ChannelSession,
//! and posts an Approve/Deny button message using the platform's
//! decision ID (PD-xxx).

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
        let parent_session_id =
            entity_field_str(&fields, &["parent_session_id", "ParentSessionId"]).unwrap_or("");

        // Read decision context from agent state
        let decision_id =
            entity_field_str(&fields, &["pending_decision_id", "PendingDecisionId"])
                .unwrap_or("");
        let tool_context_str =
            entity_field_str(&fields, &["pending_tool_context", "PendingToolContext"])
                .unwrap_or("{}");
        let tool_context: Value =
            serde_json::from_str(tool_context_str).unwrap_or_else(|_| json!({}));

        let action_desc = tool_context
            .get("action_description")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown action");

        if decision_id.is_empty() {
            ctx.log("warn", "notify_approval: no pending_decision_id, skipping");
            return Ok(());
        }

        // Find the ChannelSession for this agent
        let session = find_session_by_agent(
            &ctx,
            &temper_api_url,
            tenant,
            agent_id,
            parent_session_id,
        )?;
        let Some((session, bound_agent_id)) = session else {
            ctx.log(
                "warn",
                &format!("notify_approval: no channel session for agent {agent_id}, skipping"),
            );
            return Ok(());
        };
        if bound_agent_id != agent_id {
            ctx.log(
                "info",
                &format!(
                    "notify_approval: using parent channel session {bound_agent_id} for agent {agent_id}"
                ),
            );
        }

        let channel_id =
            entity_field_str(&session, &["ChannelId", "channel_id"]).unwrap_or("");
        let thread_id =
            entity_field_str(&session, &["ThreadId", "thread_id"]).unwrap_or("");
        if channel_id.is_empty() || thread_id.is_empty() {
            return Err("notify_approval: session missing channel_id or thread_id".to_string());
        }

        // Find the Channel entity to get the webhook_url
        let channel = find_channel_by_external_id(&ctx, &temper_api_url, tenant, channel_id)?
            .ok_or_else(|| {
                format!("notify_approval: no connected Channel for channel_id={channel_id}")
            })?;
        let webhook_url =
            entity_field_str(&channel, &["webhook_url", "WebhookUrl"]).unwrap_or("");
        if webhook_url.is_empty() {
            return Err("notify_approval: Channel has no webhook_url".to_string());
        }

        // Build the approval message with buttons.
        // custom_id uses the platform's decision ID (PD-xxx).
        let content = format!(
            "**Permission Required**\n\
             Agent wants to: **{action_desc}**\n\
             Decision: `{decision_id}`",
        );

        let body = json!({
            "thread_id": thread_id,
            "content": content,
            "agent_entity_id": agent_id,
            "components": [{
                "type": 1,
                "components": [
                    {
                        "type": 2,
                        "style": 3,
                        "label": "Approve",
                        "custom_id": format!("approve:{decision_id}")
                    },
                    {
                        "type": 2,
                        "style": 4,
                        "label": "Deny",
                        "custom_id": format!("deny:{decision_id}")
                    }
                ]
            }]
        });

        let headers = vec![
            ("content-type".to_string(), "application/json".to_string()),
            ("x-tenant-id".to_string(), tenant.to_string()),
        ];

        let resp = ctx.http_call("POST", webhook_url, &headers, &body.to_string())?;
        if !(200..300).contains(&resp.status) {
            return Err(format!(
                "notify_approval: webhook POST failed (HTTP {}): {}",
                resp.status,
                &resp.body[..resp.body.len().min(200)]
            ));
        }

        ctx.log(
            "info",
            &format!(
                "notify_approval: sent approval buttons for {decision_id} (agent {agent_id}) to thread {thread_id}"
            ),
        );
        set_success_result("", &json!({"status": "notified", "decision_id": decision_id}));
        Ok(())
    })();

    if let Err(error) = result {
        set_error_result(&error);
    }
    0
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
    let escaped = agent_id.replace('\'', "''");
    let active_filter = format!("$filter=Status eq 'Active' and agent_entity_id eq '{escaped}'&$top=1");
    let active_url = format!("{temper_api_url}/tdata/ChannelSessions?{active_filter}");
    if let Some(session) = list_entities(ctx, &active_url, tenant)?.into_iter().next() {
        return Ok(Some(session));
    }

    let any_filter = format!("$filter=agent_entity_id eq '{escaped}'&$top=1");
    let any_url = format!("{temper_api_url}/tdata/ChannelSessions?{any_filter}");
    Ok(list_entities(ctx, &any_url, tenant)?.into_iter().next())
}

fn find_channel_by_external_id(
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

fn list_entities(ctx: &Context, url: &str, tenant: &str) -> Result<Vec<Value>, String> {
    let fields = ctx
        .entity_state
        .get("fields")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let headers = runtime_headers(ctx, tenant, &fields, None, Some("application/json"));
    let resp = ctx.http_call("GET", url, &headers, "")?;
    if resp.status != 200 {
        return Err(format!("notify_approval: GET {url} failed (HTTP {})", resp.status));
    }
    let parsed: Value =
        serde_json::from_str(&resp.body).unwrap_or_else(|_| json!({"value": []}));
    Ok(parsed
        .get("value")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default())
}
