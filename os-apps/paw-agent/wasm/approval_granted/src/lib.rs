//! approval_granted WASM — executes the denied action after human approval.
//!
//! Triggered by PendingApproval.Approve. Reads the stored action context
//! (target_url, target_body), replays the action with system identity,
//! and sends the result to the user's Discord thread.

use temper_wasm_sdk::prelude::*;
use wasm_helpers::{entity_field_str, resolve_temper_api_url, runtime_headers_as};

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

        let target_url = entity_field_str(&fields, &["target_url", "TargetUrl"]).unwrap_or("");
        let target_body = entity_field_str(&fields, &["target_body", "TargetBody"]).unwrap_or("{}");
        let action_desc =
            entity_field_str(&fields, &["action_description", "ActionDescription"]).unwrap_or("");
        let agent_entity_id =
            entity_field_str(&fields, &["agent_entity_id", "AgentEntityId"]).unwrap_or("");
        let reviewer_id =
            entity_field_str(&fields, &["reviewer_id", "ReviewerId"]).unwrap_or("unknown");

        if target_url.is_empty() {
            ctx.log("warn", "approval_granted: no target_url, nothing to retry");
            return Ok(());
        }

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
        let retry_resp = ctx.http_call("POST", target_url, &system_headers, target_body)?;

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

        ctx.log(
            "info",
            &format!("approval_granted: {result_summary}"),
        );

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

/// Send a notification to the user's Discord thread via the Channel entity.
/// Best-effort — if the session or channel can't be found, we skip silently.
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

    // Find the ChannelSession for the agent
    let escaped = agent_entity_id.replace('\'', "''");
    let session_url = format!(
        "{temper_api_url}/tdata/ChannelSessions?$filter=agent_entity_id eq '{escaped}'&$top=1"
    );
    let headers = wasm_helpers::runtime_headers(
        ctx,
        tenant,
        &fields,
        None,
        Some("application/json"),
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

    // POST to the webhook (same format as send_reply)
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
