//! Session Recoverer — WASM module for recovering sessions after server restart.
//!
//! Triggered by the `RecoverFromRestart` action's `recover_session` integration.
//! Loads the session tree from TemperFS, repairs orphaned tool_use blocks,
//! injects a recovery steering message, and dispatches `RecoveryComplete`
//! to re-enter the LLM loop.
//!
//! ADR-0025: Session Recovery and Conversation Reset.

use session_tree_lib::SessionTree;
use temper_wasm_sdk::prelude::*;
use wasm_helpers::{
    append_session_entry_line_to_ref, is_session_entries_ref, read_session_from_temperfs,
    resolve_temper_api_url, write_session_to_temperfs,
};

/// Entry point.
#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        ctx.log("info", "session_recoverer: starting");

        let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));

        let session_file_id = fields
            .get("session_file_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if session_file_id.is_empty() {
            return Err(
                "session_file_id is empty — no conversation tree to recover from".to_string(),
            );
        }

        let session_leaf_id = fields
            .get("session_leaf_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let temper_api_url = resolve_temper_api_url(&ctx, &fields);
        let tenant = &ctx.tenant;

        // Load the session tree from TemperFS
        let session_jsonl =
            read_session_from_temperfs(&ctx, &temper_api_url, tenant, &fields, session_file_id)?;

        if session_jsonl.is_empty() {
            return Err("session tree is empty — nothing to recover".to_string());
        }

        let mut tree = SessionTree::from_jsonl(&session_jsonl);
        let entity_backed_session = is_session_entries_ref(session_file_id);
        let mut appended_lines: Vec<(i64, String)> = Vec::new();

        // Determine the current leaf
        let mut leaf_id = if !session_leaf_id.is_empty() {
            session_leaf_id.to_string()
        } else {
            tree.last_entry_id()
                .map(|s| s.to_string())
                .ok_or("session tree has no entries")?
        };

        // Repair orphaned tool_use blocks at the leaf (ADR-0025: authoritative repair location)
        if let Some(interrupted_results) = tree.interrupted_tool_results_for_leaf(&leaf_id) {
            let note = "Tool execution was interrupted by a server restart.";
            let tokens = estimate_tokens(note);
            let (tool_result_id, line) =
                tree.append_tool_results(&leaf_id, &interrupted_results, tokens);
            leaf_id = tool_result_id;
            appended_lines.push((tree.len().saturating_sub(1) as i64, line));
            ctx.log(
                "info",
                "session_recoverer: appended synthetic tool_results for interrupted execution",
            );
        }

        // Inject a recovery steering message
        let recovery_msg = "[System: Session recovered after server restart. Your conversation context is preserved. If you were executing tools when interrupted, the results were lost — check current state before retrying.]";
        let tokens = estimate_tokens(recovery_msg);
        let (steering_id, line) = tree.append_steering_message(&leaf_id, recovery_msg, tokens);
        leaf_id = steering_id;
        appended_lines.push((tree.len().saturating_sub(1) as i64, line));

        if entity_backed_session {
            for (sequence, line) in &appended_lines {
                append_session_entry_line_to_ref(
                    &ctx,
                    &temper_api_url,
                    tenant,
                    &fields,
                    session_file_id,
                    line,
                    *sequence,
                )?;
            }
        } else {
            // Legacy PawFS sessions still need the repaired tree written back.
            write_session_to_temperfs(
                &ctx,
                &temper_api_url,
                tenant,
                &fields,
                session_file_id,
                &tree.to_jsonl(),
            )?;
        }

        ctx.log(
            "info",
            &format!("session_recoverer: recovery complete, new leaf_id={leaf_id}"),
        );

        // If the session was inside an active tool-call checkpoint when the
        // server restarted, resume directly into tool execution instead of
        // re-entering the LLM loop. `pending_tool_context` is preserved across
        // the crash as an entity field; a non-empty `remaining_tool_calls`
        // signals that work was mid-batch.
        //
        // See TemperPaw Track 1 Phase 3 follow-up (session_recoverer mid-turn
        // resume). Keeps the Cedar pause-style checkpoint resume symmetry
        // across both failure modes — Cedar denial and server restart.
        let pending_ctx_str = fields
            .get("pending_tool_context")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty());

        let mid_turn_remaining = pending_ctx_str
            .and_then(|s| serde_json::from_str::<Value>(s).ok())
            .and_then(|v| {
                v.get("remaining_tool_calls")
                    .and_then(|r| r.as_array())
                    .map(|arr| arr.clone())
            })
            .filter(|arr| !arr.is_empty());

        if let Some(remaining) = mid_turn_remaining {
            let repl_file_id = fields
                .get("repl_file_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let pending_tool_calls =
                serde_json::to_string(&remaining).unwrap_or_else(|_| "[]".to_string());
            let pending_tool_context = pending_ctx_str.unwrap_or("{}").to_string();

            ctx.log(
                "info",
                &format!(
                    "session_recoverer: mid-turn checkpoint detected ({} tool calls remaining), resuming into Executing",
                    remaining.len()
                ),
            );

            set_success_result(
                "ResumeFromCheckpoint",
                &json!({
                    "pending_tool_calls": pending_tool_calls,
                    "pending_tool_context": pending_tool_context,
                    "repl_file_id": repl_file_id,
                    "session_leaf_id": leaf_id,
                }),
            );
        } else {
            // No checkpoint — dispatch RecoveryComplete to re-enter the LLM loop.
            set_success_result("RecoveryComplete", &json!({ "session_leaf_id": leaf_id }));
        }

        Ok(())
    })();

    if let Err(e) = result {
        set_error_result(&e);
    }
    0
}

fn estimate_tokens(text: &str) -> usize {
    text.len() / 4
}
