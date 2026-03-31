//! request_approval WASM — sends Discord buttons when a PendingApproval is created.
//!
//! Triggered by PendingApproval.Request. Looks up the ChannelSession for the
//! requesting agent, finds the Channel's webhook_url, and POSTs a message
//! with Approve/Deny button components to Discord.

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
        let approval_id = ctx.entity_id.as_str();

        let agent_entity_id =
            entity_field_str(&fields, &["agent_entity_id", "AgentEntityId"]).unwrap_or("");
        let action_description =
            entity_field_str(&fields, &["action_description", "ActionDescription"]).unwrap_or("");
        let entity_set =
            entity_field_str(&fields, &["entity_set", "EntitySet"]).unwrap_or("");
        let target_entity_id =
            entity_field_str(&fields, &["target_entity_id", "TargetEntityId"]).unwrap_or("");
        let target_action =
            entity_field_str(&fields, &["target_action", "TargetAction"]).unwrap_or("");
        let decision_id =
            entity_field_str(&fields, &["decision_id", "DecisionId"]).unwrap_or("");

        if agent_entity_id.is_empty() {
            ctx.log("warn", "request_approval: no agent_entity_id, skipping");
            return Ok(());
        }

        // Find the ChannelSession linked to this agent
        let session = find_session_by_agent(&ctx, &temper_api_url, tenant, agent_entity_id)?;
        let Some(session) = session else {
            ctx.log(
                "warn",
                &format!(
                    "request_approval: no channel session for agent {agent_entity_id}, skipping"
                ),
            );
            return Ok(());
        };

        let channel_id =
            entity_field_str(&session, &["ChannelId", "channel_id"]).unwrap_or("");
        let thread_id =
            entity_field_str(&session, &["ThreadId", "thread_id"]).unwrap_or("");
        if channel_id.is_empty() || thread_id.is_empty() {
            return Err(
                "request_approval: session missing channel_id or thread_id".to_string(),
            );
        }

        // Find the Channel entity to get the webhook_url
        let channel = find_channel_by_external_id(&ctx, &temper_api_url, tenant, channel_id)?
            .ok_or_else(|| {
                format!("request_approval: no connected Channel for channel_id={channel_id}")
            })?;
        let webhook_url =
            entity_field_str(&channel, &["webhook_url", "WebhookUrl"]).unwrap_or("");
        if webhook_url.is_empty() {
            return Err("request_approval: Channel has no webhook_url".to_string());
        }

        // Build the approval message with buttons
        let content = format!(
            "**Permission Required**\n\
             Agent wants to: **{target_action}** on `{entity_set}('{target_entity_id}')`\n\
             {}\n\
             {}\n\
             \n\
             Click a button to approve or deny this action.",
            if !action_description.is_empty() {
                format!("Description: {action_description}")
            } else {
                String::new()
            },
            if !decision_id.is_empty() {
                format!("Decision: `{decision_id}`")
            } else {
                String::new()
            },
        );

        let body = json!({
            "thread_id": thread_id,
            "content": content,
            "agent_entity_id": agent_entity_id,
            "components": [{
                "type": 1,
                "components": [
                    {
                        "type": 2,
                        "style": 3,
                        "label": "Approve",
                        "custom_id": format!("approve:{approval_id}")
                    },
                    {
                        "type": 2,
                        "style": 4,
                        "label": "Deny",
                        "custom_id": format!("deny:{approval_id}")
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
                "request_approval: webhook POST failed (HTTP {}): {}",
                resp.status,
                &resp.body[..resp.body.len().min(200)]
            ));
        }

        ctx.log(
            "info",
            &format!(
                "request_approval: sent approval buttons for {approval_id} to thread {thread_id}"
            ),
        );
        set_success_result("", &json!({ "status": "notified", "approval_id": approval_id }));
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
) -> Result<Option<Value>, String> {
    let escaped = agent_id.replace('\'', "''");
    let filter =
        format!("$filter=Status eq 'Active' and agent_entity_id eq '{escaped}'&$top=1");
    let url = format!("{temper_api_url}/tdata/ChannelSessions?{filter}");
    let entities = list_entities(ctx, &url, tenant)?;
    if let Some(session) = entities.into_iter().next() {
        return Ok(Some(session));
    }
    // Fallback: any session for this agent
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
    let filter =
        format!("$filter=Status eq 'Connected' and channel_id eq '{escaped}'&$top=1");
    let url = format!("{temper_api_url}/tdata/Channels?{filter}");
    let entities = list_entities(ctx, &url, tenant)?;
    if let Some(channel) = entities.into_iter().next() {
        return Ok(Some(channel));
    }
    let any_filter = format!("$filter=channel_id eq '{escaped}'&$top=1");
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
        return Err(format!(
            "request_approval: GET {url} failed (HTTP {})",
            resp.status
        ));
    }
    let parsed: Value =
        serde_json::from_str(&resp.body).unwrap_or_else(|_| json!({ "value": [] }));
    Ok(parsed
        .get("value")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default())
}

