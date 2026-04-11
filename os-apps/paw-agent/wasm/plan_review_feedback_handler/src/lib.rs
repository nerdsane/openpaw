//! Plan Review Feedback Handler — resumes a paused session when a plan
//! reviewer sends it back with changes requested.

use session_tree_lib::SessionTree;
use temper_wasm_sdk::prelude::*;
use wasm_helpers::{
    create_content_file, entity_field_str, read_session_from_temperfs, resolve_temper_api_url,
    runtime_headers, write_session_to_temperfs,
};

const DEFAULT_REVIEW_NOTES: &str =
    "Plan changes were requested. Revise the plan and resubmit it for review.";

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
        let plan_id = ctx.entity_id.as_str();
        let session_id = entity_field_str(&fields, &["session_id", "SessionId"]).unwrap_or("");
        let review_notes = entity_field_str(&fields, &["review_notes", "ReviewNotes"])
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(DEFAULT_REVIEW_NOTES);

        if session_id.is_empty() {
            ctx.log(
                "info",
                &format!(
                    "plan_review_feedback_handler: plan {plan_id} has no session_id; skipping"
                ),
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

        let session = fetch_session(&ctx, &temper_api_url, tenant, &fields, session_id)?;
        let session_status = entity_field_str(&session, &["Status", "status"]).unwrap_or("");
        if session_status != "WaitingForApproval" {
            ctx.log(
                "info",
                &format!(
                    "plan_review_feedback_handler: session {session_id} is {session_status}, not WaitingForApproval; skipping"
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

        let session_fields = session
            .get("fields")
            .or_else(|| session.get("Fields"))
            .cloned()
            .unwrap_or_else(|| json!({}));
        let review_message = format!(
            "Plan review feedback:\n{}\n\nRevise the plan and resubmit it for review.",
            review_notes
        );

        let resume_action = if let Some(session_leaf_id) = inject_review_message(
            &ctx,
            &temper_api_url,
            tenant,
            &session_fields,
            &review_message,
        )? {
            json!({
                "session_leaf_id": session_leaf_id,
            })
        } else {
            let updated_conversation =
                inline_conversation_with_review(&session_fields, &review_message)?;
            json!({
                "conversation": updated_conversation,
            })
        };

        let resume_url = format!(
            "{temper_api_url}/tdata/Sessions('{session_id}')/OpenPaw.ResumeWithPlanChanges"
        );
        let headers = runtime_headers(
            &ctx,
            tenant,
            &session_fields,
            Some("application/json"),
            Some("application/json"),
        );
        let resp = ctx.http_call("POST", &resume_url, &headers, &resume_action.to_string())?;
        if !(200..300).contains(&resp.status) {
            return Err(format!(
                "plan_review_feedback_handler: ResumeWithPlanChanges failed (HTTP {}): {}",
                resp.status,
                &resp.body[..resp.body.len().min(200)],
            ));
        }

        ctx.log(
            "info",
            &format!(
                "plan_review_feedback_handler: resumed session {session_id} with plan feedback for {plan_id}"
            ),
        );
        set_success_result(
            "",
            &json!({
                "status": "resumed",
                "session_id": session_id,
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

fn fetch_session(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    fields: &Value,
    session_id: &str,
) -> Result<Value, String> {
    let headers = runtime_headers(
        ctx,
        tenant,
        fields,
        Some("application/json"),
        Some("application/json"),
    );
    let session_url = format!("{temper_api_url}/tdata/Sessions('{session_id}')");
    let resp = ctx.http_call("GET", &session_url, &headers, "")?;
    if resp.status != 200 {
        return Err(format!(
            "plan_review_feedback_handler: GET session {session_id} failed (HTTP {})",
            resp.status,
        ));
    }
    serde_json::from_str(&resp.body).map_err(|e| format!("parse session: {e}"))
}

fn inject_review_message(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    session_fields: &Value,
    review_message: &str,
) -> Result<Option<String>, String> {
    let session_file_id =
        entity_field_str(session_fields, &["session_file_id", "SessionFileId"]).unwrap_or("");
    let session_leaf_id =
        entity_field_str(session_fields, &["session_leaf_id", "SessionLeafId"]).unwrap_or("");

    if session_file_id.is_empty() || session_leaf_id.is_empty() {
        return Ok(None);
    }

    let session_jsonl =
        read_session_from_temperfs(ctx, temper_api_url, tenant, session_fields, session_file_id)?;
    let mut tree = SessionTree::from_jsonl(&session_jsonl);
    let workspace_id =
        entity_field_str(session_fields, &["workspace_id", "WorkspaceId"]).unwrap_or("");

    let new_leaf_id = if !workspace_id.is_empty() {
        let file_name = format!("review-feedback-{}.txt", tree.len());
        let content_file_id = create_content_file(
            ctx,
            temper_api_url,
            tenant,
            workspace_id,
            &file_name,
            review_message,
        )?;
        let (leaf_id, _) = tree.append_user_message_file(
            session_leaf_id,
            &content_file_id,
            estimate_tokens(review_message),
        );
        leaf_id
    } else {
        let (leaf_id, _) = tree.append_user_message(
            session_leaf_id,
            review_message,
            estimate_tokens(review_message),
        );
        leaf_id
    };

    write_session_to_temperfs(
        ctx,
        temper_api_url,
        tenant,
        session_fields,
        session_file_id,
        &tree.to_jsonl(),
    )?;

    Ok(Some(new_leaf_id))
}

fn inline_conversation_with_review(
    session_fields: &Value,
    review_message: &str,
) -> Result<String, String> {
    let conversation_json =
        entity_field_str(session_fields, &["conversation", "Conversation"]).unwrap_or("[]");
    let mut messages: Vec<Value> = serde_json::from_str(conversation_json).unwrap_or_default();
    messages.push(json!({
        "role": "user",
        "content": review_message,
    }));
    serde_json::to_string(&messages).map_err(|e| format!("serialize conversation: {e}"))
}

fn estimate_tokens(text: &str) -> usize {
    (text.len() / 4).max(1)
}
