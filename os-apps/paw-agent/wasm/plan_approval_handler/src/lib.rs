//! Plan Approval Handler — WASM integration triggered by Plan.Approve.
//!
//! When a Plan entity is approved (UnderReview/Escalated → Active), this
//! module checks if the Plan has an associated session_id. If the session
//! is in WaitingForApproval state, it dispatches ResumeWithPlanApproval
//! to switch the session back to execute mode with restored tools.
//!
//! Follows the agent_reply cross-entity dispatch pattern.

use temper_wasm_sdk::prelude::*;
use wasm_helpers::{entity_field_str, resolve_temper_api_url, runtime_headers};

/// Default tools — must match dispatch.rs and entity_ops.rs.
const DEFAULT_TOOLS_ENABLED: &str = "temper_create,temper_get,temper_list,temper_action,temper_patch,temper_submit_specs,temper_show_spec,temper_specs,temper_upload_wasm,temper_get_trajectories,temper_get_insights,temper_get_decisions,temper_poll_decision,temper_approve_decision,temper_deny_decision,temper_submit_policy,temper_list_policies,temper_get_policy,temper_update_policy,temper_delete_policy,temper_search_apps,temper_install_app,temper_publish_app,temper_update_app,temper_list_apps,temper_spawn_session,temper_list_sessions,temper_abort_session,temper_steer_session,temper_save_memory,temper_recall_memory,temper_write,temper_write_many,temper_read,temper_run_coding_agent,temper_get_secret,temper_datadog_query,temper_railway,temper_vercel,temper_web_search,temper_web_fetch,read,write,edit,bash";

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

        // Read session_id from the Plan entity
        let session_id =
            entity_field_str(&fields, &["session_id", "SessionId"]).unwrap_or("");
        if session_id.is_empty() {
            ctx.log(
                "info",
                "plan_approval_handler: no session_id on Plan; skipping",
            );
            set_success_result(
                "",
                &json!({
                    "status": "skipped",
                    "reason": "no_session_id",
                }),
            );
            return Ok(());
        }

        // GET the Session to check its state
        let headers = runtime_headers(
            &ctx,
            tenant,
            &fields,
            Some("application/json"),
            Some("application/json"),
        );
        let session_url = format!("{temper_api_url}/tdata/Sessions('{session_id}')");
        let resp = ctx.http_call("GET", &session_url, &headers, "")?;
        if resp.status != 200 {
            return Err(format!(
                "plan_approval_handler: GET session {session_id} failed (HTTP {})",
                resp.status,
            ));
        }
        let session: Value =
            serde_json::from_str(&resp.body).map_err(|e| format!("parse session: {e}"))?;
        let session_status =
            entity_field_str(&session, &["Status", "status"]).unwrap_or("");
        if session_status != "WaitingForApproval" {
            ctx.log(
                "info",
                &format!(
                    "plan_approval_handler: session {session_id} is {session_status}, not WaitingForApproval; skipping"
                ),
            );
            set_success_result(
                "",
                &json!({
                    "status": "skipped",
                    "reason": "session_not_waiting",
                    "session_status": session_status,
                }),
            );
            return Ok(());
        }

        // Read pre_plan_tools_enabled from the session to restore
        let session_fields = session
            .get("fields")
            .or_else(|| session.get("Fields"))
            .cloned()
            .unwrap_or_else(|| json!({}));
        let stashed_tools = entity_field_str(
            &session_fields,
            &["pre_plan_tools_enabled", "PrePlanToolsEnabled"],
        )
        .filter(|s| !s.is_empty())
        .unwrap_or(DEFAULT_TOOLS_ENABLED);

        // Dispatch ResumeWithPlanApproval on the session
        let resume_body = json!({
            "session_mode": "execute",
            "tools_enabled": stashed_tools,
            "pre_plan_tools_enabled": "",
        });
        let resume_url = format!(
            "{temper_api_url}/tdata/Sessions('{session_id}')/TemperPaw.ResumeWithPlanApproval"
        );
        let resp = ctx.http_call("POST", &resume_url, &headers, &resume_body.to_string())?;
        if !(200..300).contains(&resp.status) {
            return Err(format!(
                "plan_approval_handler: ResumeWithPlanApproval failed (HTTP {}): {}",
                resp.status,
                &resp.body[..resp.body.len().min(200)],
            ));
        }

        ctx.log(
            "info",
            &format!(
                "plan_approval_handler: resumed session {session_id} in execute mode"
            ),
        );
        set_success_result(
            "",
            &json!({
                "status": "resumed",
                "session_id": session_id,
            }),
        );
        Ok(())
    })();

    if let Err(error) = result {
        set_error_result(&error);
    }
    0
}
