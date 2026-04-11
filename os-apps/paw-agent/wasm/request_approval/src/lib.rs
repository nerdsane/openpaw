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
        // Use the persistent Agent entity ID from Session fields, not the Session's own ID.
        // ChannelSessions store agent_entity_id = aj-..., not ss-...
        let session_id = ctx.entity_id.as_str();
        let agent_id = entity_field_str(&fields, &["agent_id", "AgentId"])
            .unwrap_or(session_id);
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

        // The pending_tool_context has shape:
        // { "tool_context": { "method": "Session.SwitchMode", "args": {...} }, ... }
        let inner_ctx = tool_context.get("tool_context").unwrap_or(&tool_context);
        let action_method = inner_ctx
            .get("method")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown action");
        let action_desc = action_method;

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

        // Register callback on the linked GovernanceDecision entity so that
        // when a human approves/denies, the GD dispatches back to this Session.
        if let Err(e) = register_gd_callback(
            &ctx, &temper_api_url, tenant, session_id, decision_id,
        ) {
            ctx.log("warn", &format!("notify_approval: GD callback registration failed: {e}"));
        }

        set_success_result("", &json!({"status": "notified", "decision_id": decision_id}));
        Ok(())
    })();

    if let Err(error) = result {
        set_error_result(&error);
    }
    0
}

/// Query the GovernanceDecision entity by pending_decision_id in temper-system
/// and dispatch RegisterCallback so approval/denial routes back to this Session.
fn register_gd_callback(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    session_id: &str,
    decision_id: &str,
) -> Result<(), String> {
    // Query GD by pending_decision_id in temper-system tenant.
    let escaped = decision_id.replace('\'', "''");
    let gd_filter = format!("$filter=pending_decision_id eq '{escaped}'&$top=1");
    let gd_url = format!("{temper_api_url}/tdata/GovernanceDecisions?{gd_filter}");
    // Use "admin" principal — "system" is blocked from HTTP headers to prevent
    // privilege escalation. The temper-system tenant has a Cedar policy permitting
    // Admin principals to manage GovernanceDecision entities.
    let system_headers = vec![
        ("accept".to_string(), "application/json".to_string()),
        ("x-tenant-id".to_string(), "temper-system".to_string()),
        ("x-temper-principal-kind".to_string(), "admin".to_string()),
    ];

    let resp = ctx.http_call("GET", &gd_url, &system_headers, "")?;
    if resp.status != 200 {
        return Err(format!("GD query failed (HTTP {})", resp.status));
    }
    let parsed: Value =
        serde_json::from_str(&resp.body).unwrap_or_else(|_| json!({"value": []}));
    let gd = parsed
        .get("value")
        .and_then(Value::as_array)
        .and_then(|arr| arr.first());
    let Some(gd) = gd else {
        ctx.log("info", &format!(
            "notify_approval: no GovernanceDecision found for {decision_id}, callback skipped"
        ));
        return Ok(());
    };

    let gd_id = gd
        .get("entity_id")
        .or_else(|| gd.get("fields").and_then(|f| f.get("Id")))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if gd_id.is_empty() {
        return Err("GovernanceDecision has no entity_id".to_string());
    }

    // Dispatch RegisterCallback on the GovernanceDecision.
    let callback_url = format!(
        "{temper_api_url}/tdata/GovernanceDecisions('{gd_id}')/temper-system.RegisterCallback"
    );
    let callback_body = json!({
        "callback_tenant": tenant,
        "callback_entity_set": "Sessions",
        "callback_entity_id": session_id,
        "callback_on_approve": "ResumeAfterApproval",
        "callback_on_deny": "Fail"
    });
    let post_headers = vec![
        ("content-type".to_string(), "application/json".to_string()),
        ("x-tenant-id".to_string(), "temper-system".to_string()),
        ("x-temper-principal-kind".to_string(), "admin".to_string()),
    ];

    let resp = ctx.http_call("POST", &callback_url, &post_headers, &callback_body.to_string())?;
    if !(200..300).contains(&resp.status) {
        return Err(format!(
            "RegisterCallback failed (HTTP {}): {}",
            resp.status,
            &resp.body[..resp.body.len().min(200)]
        ));
    }

    ctx.log("info", &format!(
        "notify_approval: registered callback on GD {gd_id} → Sessions('{session_id}')"
    ));
    Ok(())
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
