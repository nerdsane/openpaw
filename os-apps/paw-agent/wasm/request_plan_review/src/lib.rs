//! notify_plan_approval_needed WASM — sends plan review controls when a session
//! pauses behind `PauseForPlanApproval`.
//!
//! This is intentionally separate from governance approvals:
//! - plan review is keyed by `active_plan_id`
//! - governance approval is keyed by `pending_decision_id`
//!
//! Keeping them separate makes the session state machine readable and avoids
//! overloading governance-only callback wiring onto plan review.
//!
//! Notification is best-effort. A paused session without a channel binding
//! must still remain reviewable via dashboard/API flows.

use temper_wasm_sdk::prelude::*;
use wasm_helpers::{
    entity_field_str, find_channel_session_by_agent, find_connected_channel_by_external_id,
    resolve_temper_api_url, runtime_headers,
};

const PLAN_SNIPPET_LIMIT: usize = 600;

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
        let session_id = ctx.entity_id.as_str();
        let agent_id = entity_field_str(&fields, &["agent_id", "AgentId"]).unwrap_or(session_id);
        let parent_session_id =
            entity_field_str(&fields, &["parent_session_id", "ParentSessionId"]).unwrap_or("");
        let plan_id = entity_field_str(&fields, &["active_plan_id", "ActivePlanId"]).unwrap_or("");

        if plan_id.is_empty() {
            return Err("notify_plan_review: missing active_plan_id".to_string());
        }

        let session =
            find_session_by_agent(&ctx, &temper_api_url, tenant, agent_id, parent_session_id)?;
        let Some((session, bound_agent_id)) = session else {
            ctx.log(
                "warn",
                &format!(
                    "notify_plan_review: no channel session for agent {agent_id}; plan {plan_id} awaiting out-of-band review"
                ),
            );
            set_success_result(
                "",
                &json!({
                    "status": "waiting_for_out_of_band_review",
                    "plan_id": plan_id,
                    "delivery": "skipped",
                    "reason": "no_channel_session",
                }),
            );
            return Ok(());
        };
        if bound_agent_id != agent_id {
            ctx.log(
                "info",
                &format!(
                    "notify_plan_review: using parent channel session {bound_agent_id} for agent {agent_id}"
                ),
            );
        }

        let channel_id = entity_field_str(&session, &["ChannelId", "channel_id"]).unwrap_or("");
        let thread_id = entity_field_str(&session, &["ThreadId", "thread_id"]).unwrap_or("");
        if channel_id.is_empty() || thread_id.is_empty() {
            ctx.log(
                "warn",
                &format!(
                    "notify_plan_review: session missing channel_id or thread_id for agent {agent_id}; plan {plan_id} awaiting out-of-band review"
                ),
            );
            set_success_result(
                "",
                &json!({
                    "status": "waiting_for_out_of_band_review",
                    "plan_id": plan_id,
                    "delivery": "skipped",
                    "reason": "missing_channel_binding",
                }),
            );
            return Ok(());
        }

        let Some(channel) =
            find_connected_channel_by_external_id(&ctx, &temper_api_url, tenant, channel_id)?
        else {
            ctx.log(
                "warn",
                &format!(
                    "notify_plan_review: no connected Channel for channel_id={channel_id}; plan {plan_id} awaiting out-of-band review"
                ),
            );
            set_success_result(
                "",
                &json!({
                    "status": "waiting_for_out_of_band_review",
                    "plan_id": plan_id,
                    "delivery": "skipped",
                    "reason": "channel_not_connected",
                }),
            );
            return Ok(());
        };
        let webhook_url = entity_field_str(&channel, &["webhook_url", "WebhookUrl"]).unwrap_or("");
        if webhook_url.is_empty() {
            ctx.log(
                "warn",
                &format!(
                    "notify_plan_review: Channel has no webhook_url for channel_id={channel_id}; plan {plan_id} awaiting out-of-band review"
                ),
            );
            set_success_result(
                "",
                &json!({
                    "status": "waiting_for_out_of_band_review",
                    "plan_id": plan_id,
                    "delivery": "skipped",
                    "reason": "missing_webhook_url",
                }),
            );
            return Ok(());
        }

        let (plan_description, plan_snippet) =
            fetch_plan_preview(&ctx, &temper_api_url, tenant, &fields, plan_id)?;

        let mut content_lines = vec![
            "**Plan Review Required**".to_string(),
            format!("Plan: `{plan_id}`"),
        ];
        if !plan_description.is_empty() {
            content_lines.push(format!("Summary: {plan_description}"));
        }
        if !plan_snippet.is_empty() {
            content_lines.push(String::new());
            content_lines.push("Plan excerpt:".to_string());
            content_lines.push(format!("```text\n{plan_snippet}\n```"));
        }
        content_lines.push(String::new());
        content_lines.push(
            "Approve to restore execute mode. Request changes to send the plan back for revision."
                .to_string(),
        );
        let content = content_lines.join("\n");

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
                        "label": "Approve Plan",
                        "custom_id": format!("plan_approve:{plan_id}")
                    },
                    {
                        "type": 2,
                        "style": 4,
                        "label": "Request Changes",
                        "custom_id": format!("plan_request_changes:{plan_id}")
                    }
                ]
            }],
            "blocks": [
                {
                    "type": "section",
                    "text": {
                        "type": "mrkdwn",
                        "text": content,
                    }
                },
                {
                    "type": "actions",
                    "block_id": format!("plan_review_{plan_id}"),
                    "elements": [
                        {
                            "type": "button",
                            "text": {
                                "type": "plain_text",
                                "text": "Approve Plan"
                            },
                            "action_id": format!("plan_approve:{plan_id}"),
                            "style": "primary"
                        },
                        {
                            "type": "button",
                            "text": {
                                "type": "plain_text",
                                "text": "Request Changes"
                            },
                            "action_id": format!("plan_request_changes:{plan_id}"),
                            "style": "danger"
                        }
                    ]
                }
            ]
        });

        let headers = vec![
            ("content-type".to_string(), "application/json".to_string()),
            ("x-tenant-id".to_string(), tenant.to_string()),
        ];
        let resp = ctx.http_call("POST", webhook_url, &headers, &body.to_string())?;
        if !(200..300).contains(&resp.status) {
            let failure = format!(
                "notify_plan_review: webhook POST failed (HTTP {}): {}",
                resp.status,
                &resp.body[..resp.body.len().min(200)]
            );
            ctx.log("warn", &failure);
            set_success_result(
                "",
                &json!({
                    "status": "waiting_for_out_of_band_review",
                    "plan_id": plan_id,
                    "delivery": "failed",
                    "error": failure,
                }),
            );
            return Ok(());
        }

        ctx.log(
            "info",
            &format!(
                "notify_plan_review: sent plan review buttons for {plan_id} (agent {agent_id}) to thread {thread_id}"
            ),
        );
        set_success_result(
            "",
            &json!({
                "status": "notified",
                "plan_id": plan_id,
            }),
        );
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
    if let Some(session) = find_channel_session_by_agent(ctx, temper_api_url, tenant, agent_id)? {
        return Ok(Some((session, agent_id.to_string())));
    }

    let parent_session_id = parent_session_id.trim();
    if !parent_session_id.is_empty() && parent_session_id != agent_id {
        if let Some(session) =
            find_channel_session_by_agent(ctx, temper_api_url, tenant, parent_session_id)?
        {
            return Ok(Some((session, parent_session_id.to_string())));
        }
    }

    Ok(None)
}

fn fetch_plan_preview(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    fields: &Value,
    plan_id: &str,
) -> Result<(String, String), String> {
    let headers = runtime_headers(
        ctx,
        tenant,
        fields,
        Some("application/json"),
        Some("application/json"),
    );
    let plan_url = format!("{temper_api_url}/tdata/Plans('{plan_id}')");
    let resp = ctx.http_call("GET", &plan_url, &headers, "")?;
    if resp.status != 200 {
        return Err(format!(
            "notify_plan_review: GET plan {plan_id} failed (HTTP {})",
            resp.status,
        ));
    }
    let plan: Value = serde_json::from_str(&resp.body).map_err(|e| format!("parse plan: {e}"))?;
    let description = entity_field_str(&plan, &["Description", "description"])
        .unwrap_or("")
        .trim()
        .to_string();
    let plan_text = entity_field_str(&plan, &["PlanText", "plan_text"])
        .unwrap_or("")
        .trim();
    Ok((
        description,
        truncate_for_preview(plan_text, PLAN_SNIPPET_LIMIT),
    ))
}

fn truncate_for_preview(value: &str, limit: usize) -> String {
    if value.len() <= limit {
        return value.to_string();
    }

    let boundary = value.floor_char_boundary(limit);
    format!("{}...", value[..boundary].trim_end())
}
