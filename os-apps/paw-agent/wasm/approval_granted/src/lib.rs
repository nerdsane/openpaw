//! approval_granted WASM — executes the denied action after human approval.
//!
//! Triggered by PendingApproval.Approve. Reads the stored action context
//! (target_url, target_body), replays the action with system identity,
//! creates a ChannelSession for child agents, and notifies Discord.

use temper_wasm_sdk::prelude::*;
use wasm_helpers::{entity_field_str, resolve_temper_api_url, runtime_headers, runtime_headers_as};

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

        let target_url = entity_field_str(&fields, &["target_url", "TargetUrl"]).unwrap_or("");
        let raw_body = entity_field_str(&fields, &["target_body", "TargetBody"]).unwrap_or("{}");
        let action_desc =
            entity_field_str(&fields, &["action_description", "ActionDescription"]).unwrap_or("");
        let agent_entity_id =
            entity_field_str(&fields, &["agent_entity_id", "AgentEntityId"]).unwrap_or("");
        let target_entity_id =
            entity_field_str(&fields, &["target_entity_id", "TargetEntityId"]).unwrap_or("");
        let target_action =
            entity_field_str(&fields, &["target_action", "TargetAction"]).unwrap_or("");
        let entity_set =
            entity_field_str(&fields, &["entity_set", "EntitySet"]).unwrap_or("");
        let reviewer_id =
            entity_field_str(&fields, &["reviewer_id", "ReviewerId"]).unwrap_or("unknown");

        if target_url.is_empty() {
            ctx.log("warn", "approval_granted: no target_url, nothing to retry");
            return Ok(());
        }

        // Fix potential double-serialization of target_body.
        // handle_cedar_denial stores body.to_string() which is valid JSON.
        // But if the IOA string field escaped it further, unwrap one layer.
        let body_to_send = match serde_json::from_str::<Value>(raw_body) {
            Ok(v) if v.is_string() => {
                // Double-encoded: the JSON string contains another JSON string
                v.as_str().unwrap_or("{}").to_string()
            }
            Ok(_) => raw_body.to_string(),
            Err(_) => raw_body.to_string(),
        };

        ctx.log(
            "info",
            &format!("approval_granted: retrying {action_desc} as system (approved by {reviewer_id})"),
        );

        // Execute the originally denied action with system identity
        let system_headers = runtime_headers_as(
            &ctx,
            tenant,
            &fields,
            "system",
            Some("application/json"),
            Some("application/json"),
        );
        let retry_resp = ctx.http_call("POST", target_url, &system_headers, &body_to_send)?;

        let (success, result_summary) = if retry_resp.status >= 200 && retry_resp.status < 300 {
            (true, format!("Action succeeded (HTTP {})", retry_resp.status))
        } else {
            (
                false,
                format!(
                    "Action failed (HTTP {}): {}",
                    retry_resp.status,
                    &retry_resp.body[..retry_resp.body.len().min(200)]
                ),
            )
        };

        ctx.log("info", &format!("approval_granted: {result_summary}"));

        // If we just Configured an Agent (child), create a ChannelSession so
        // its replies reach Discord when it completes.
        if success
            && entity_set.contains("Agent")
            && (target_action == "Configure" || target_action == "configure")
            && !target_entity_id.is_empty()
            && !agent_entity_id.is_empty()
        {
            create_child_session(
                &ctx,
                &temper_api_url,
                tenant,
                agent_entity_id,    // parent agent
                target_entity_id,   // child agent
            );
        }

        // Notify the user via Discord
        if !agent_entity_id.is_empty() {
            let discord_msg = if success {
                format!("**Approved and executed**: {action_desc}")
            } else {
                format!("**Approved but failed**: {action_desc}\n{result_summary}")
            };
            send_discord_notification(&ctx, &temper_api_url, tenant, agent_entity_id, &discord_msg);
        }

        set_success_result(
            "",
            &json!({
                "status": if success { "executed" } else { "failed" },
                "action_description": action_desc,
                "result": result_summary,
            }),
        );
        Ok(())
    })();

    if let Err(error) = result {
        set_error_result(&error);
    }
    0
}

/// Create a ChannelSession for a child agent, linking it to the parent's
/// Discord thread so `agent_reply` can deliver the child's results.
fn create_child_session(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    parent_agent_id: &str,
    child_agent_id: &str,
) {
    let fields = ctx
        .entity_state
        .get("fields")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let headers = runtime_headers_as(
        ctx, tenant, &fields, "system",
        Some("application/json"), Some("application/json"),
    );

    // Find the parent's ChannelSession to get channel_id + thread_id
    let escaped = parent_agent_id.replace('\'', "''");
    let session_url = format!(
        "{temper_api_url}/tdata/ChannelSessions?$filter=agent_entity_id eq '{escaped}'&$top=1"
    );
    let Ok(resp) = ctx.http_call("GET", &session_url, &headers, "") else {
        ctx.log("warn", "approval_granted: failed to look up parent session");
        return;
    };
    if resp.status != 200 {
        return;
    }
    let sessions: Value =
        serde_json::from_str(&resp.body).unwrap_or_else(|_| json!({"value": []}));
    let Some(parent_session) = sessions
        .get("value")
        .and_then(Value::as_array)
        .and_then(|arr| arr.first())
    else {
        ctx.log("warn", "approval_granted: parent has no ChannelSession");
        return;
    };

    let channel_id = entity_field_str(parent_session, &["ChannelId", "channel_id"]).unwrap_or("");
    let thread_id = entity_field_str(parent_session, &["ThreadId", "thread_id"]).unwrap_or("");
    if channel_id.is_empty() || thread_id.is_empty() {
        return;
    }

    // Create a new ChannelSession for the child agent
    let create_url = format!("{temper_api_url}/tdata/ChannelSessions");
    let Ok(create_resp) = ctx.http_call("POST", &create_url, &headers, "{}") else {
        ctx.log("warn", "approval_granted: failed to create child session");
        return;
    };
    if create_resp.status < 200 || create_resp.status >= 300 {
        ctx.log("warn", &format!(
            "approval_granted: child session create failed (HTTP {})",
            create_resp.status
        ));
        return;
    }
    let created: Value =
        serde_json::from_str(&create_resp.body).unwrap_or_else(|_| json!({}));
    let session_id = created
        .get("entity_id")
        .or_else(|| created.get("Id"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if session_id.is_empty() {
        return;
    }

    // Configure the session with the same channel/thread but child's agent_id
    let config_body = json!({
        "channel_id": channel_id,
        "thread_id": thread_id,
        "author_id": "system",
        "agent_entity_id": child_agent_id,
        "last_message_at": "created",
    });
    let config_url = format!(
        "{temper_api_url}/tdata/ChannelSessions('{session_id}')/Paw.Channel.Create"
    );
    let _ = ctx.http_call("POST", &config_url, &headers, &config_body.to_string());

    ctx.log(
        "info",
        &format!(
            "approval_granted: created ChannelSession {session_id} for child agent {child_agent_id}"
        ),
    );
}

/// Send a notification to the user's Discord thread via the Channel entity.
fn send_discord_notification(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    agent_entity_id: &str,
    content: &str,
) {
    let fields = ctx
        .entity_state
        .get("fields")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let headers = runtime_headers(ctx, tenant, &fields, None, Some("application/json"));

    // Find the ChannelSession for the agent
    let escaped = agent_entity_id.replace('\'', "''");
    let session_url = format!(
        "{temper_api_url}/tdata/ChannelSessions?$filter=agent_entity_id eq '{escaped}'&$top=1"
    );
    let Ok(session_resp) = ctx.http_call("GET", &session_url, &headers, "") else {
        return;
    };
    if session_resp.status != 200 {
        return;
    }
    let sessions: Value =
        serde_json::from_str(&session_resp.body).unwrap_or_else(|_| json!({"value": []}));
    let Some(session) = sessions
        .get("value")
        .and_then(Value::as_array)
        .and_then(|arr| arr.first())
    else {
        return;
    };

    let channel_id = entity_field_str(session, &["ChannelId", "channel_id"]).unwrap_or("");
    let thread_id = entity_field_str(session, &["ThreadId", "thread_id"]).unwrap_or("");
    if channel_id.is_empty() || thread_id.is_empty() {
        return;
    }

    // Find the Channel entity
    let escaped_ch = channel_id.replace('\'', "''");
    let channel_url = format!(
        "{temper_api_url}/tdata/Channels?$filter=Status eq 'Connected' and channel_id eq '{escaped_ch}'&$top=1"
    );
    let Ok(ch_resp) = ctx.http_call("GET", &channel_url, &headers, "") else {
        return;
    };
    if ch_resp.status != 200 {
        return;
    }
    let channels: Value =
        serde_json::from_str(&ch_resp.body).unwrap_or_else(|_| json!({"value": []}));
    let Some(channel) = channels
        .get("value")
        .and_then(Value::as_array)
        .and_then(|arr| arr.first())
    else {
        return;
    };

    let webhook_url = entity_field_str(channel, &["webhook_url", "WebhookUrl"]).unwrap_or("");
    if webhook_url.is_empty() {
        return;
    }

    let body = json!({
        "thread_id": thread_id,
        "content": content,
        "agent_entity_id": agent_entity_id,
    });
    let wh_headers = vec![
        ("content-type".to_string(), "application/json".to_string()),
        ("x-tenant-id".to_string(), tenant.to_string()),
    ];
    let _ = ctx.http_call("POST", webhook_url, &wh_headers, &body.to_string());
}
