#[path = "../../common.rs"]
mod common;

use common::{
    content_string_to_text, create_session_event, extract_text_from_value, field_i64, field_string,
    get_entity, message_blocks_json, next_session_event_sequence, status_of, system_json_headers,
};
use session_tree_lib::{EntryType, SessionEntry, SessionTree};
use temper_wasm_sdk::prelude::*;
use wasm_helpers::{read_content_file, read_session_from_temperfs, resolve_temper_api_url};

#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        let fields = ctx.entity_state.get("fields").cloned().unwrap_or_else(|| json!({}));
        let base_url = resolve_temper_api_url(&ctx, &fields);
        let headers = system_json_headers(&ctx, &ctx.tenant, &fields);

        let inner_session_id = field_string(&fields, &["InnerSessionId", "inner_session_id"]);
        if inner_session_id.is_empty() {
            temper_wasm_sdk::set_success_result(
                "UpdateUsage",
                &json!({
                    "input_tokens": 0,
                    "output_tokens": 0,
                    "stop_reason": "error",
                    "stop_reason_event_ids": "[]",
                    "last_emitted_result_hash": "missing-inner-session",
                }),
            );
            return Ok(());
        }

        let inner_session = get_entity(&ctx, &base_url, &headers, "Sessions", &inner_session_id)?;
        let inner_fields = inner_session
            .get("fields")
            .cloned()
            .unwrap_or_else(|| inner_session.clone());
        let inner_status = status_of(&inner_session);
        let result_text = resolve_result_text(&ctx, &base_url, &inner_fields)?;
        let stop_reason = if inner_status == "Completed" {
            "user_input_required"
        } else {
            "error"
        };
        let result_hash = format!("{inner_session_id}:{inner_status}:{result_text}");
        let previous_hash =
            field_string(&fields, &["LastEmittedResultHash", "last_emitted_result_hash"]);

        if previous_hash != result_hash {
            let mut sequence = next_session_event_sequence(&ctx, &base_url, &headers, &ctx.entity_id)?;
            emit_tree_events(&ctx, &base_url, &headers, &inner_fields, &ctx.entity_id, &mut sequence)?;

            if !result_text.trim().is_empty() {
                let _ = create_session_event(
                    &ctx,
                    &base_url,
                    &headers,
                    &ctx.entity_id,
                    sequence,
                    "agent.message",
                    json!({ "Content": message_blocks_json(&result_text) }),
                )?;
                sequence += 1;
            }

            let mut idle_payload = json!({
                "StopReason": stop_reason,
                "StopReasonEventIds": "[]",
            });
            if inner_status != "Completed" {
                let error_message = field_string(&inner_fields, &["ErrorMessage", "error_message"]);
                if !error_message.is_empty() {
                    idle_payload["ErrorMessage"] = json!(error_message);
                }
                idle_payload["ErrorKind"] = json!(inner_status.to_lowercase());
            }
            let _ = create_session_event(
                &ctx,
                &base_url,
                &headers,
                &ctx.entity_id,
                sequence,
                "session.status_idle",
                idle_payload,
            )?;
        }

        temper_wasm_sdk::set_success_result(
            "UpdateUsage",
            &json!({
                "input_tokens": field_i64(&inner_fields, &["InputTokens", "input_tokens"]),
                "output_tokens": field_i64(&inner_fields, &["OutputTokens", "output_tokens"]),
                "stop_reason": stop_reason,
                "stop_reason_event_ids": "[]",
                "last_emitted_result_hash": result_hash,
            }),
        );
        Ok(())
    })();

    if let Err(error) = result {
        temper_wasm_sdk::set_error_result(&error);
    }
    0
}

fn resolve_result_text(ctx: &Context, base_url: &str, inner_fields: &Value) -> Result<String, String> {
    let direct = field_string(inner_fields, &["Result", "result"]);
    if !direct.is_empty() {
        return Ok(direct);
    }

    let session_file_id = field_string(inner_fields, &["SessionFileId", "session_file_id"]);
    let session_leaf_id = field_string(inner_fields, &["SessionLeafId", "session_leaf_id"]);
    if session_file_id.is_empty() || session_leaf_id.is_empty() {
        return Ok(String::new());
    }

    let jsonl = read_session_from_temperfs(ctx, base_url, &ctx.tenant, inner_fields, &session_file_id)?;
    let tree = SessionTree::from_jsonl(&jsonl);
    let refs = tree.build_context_refs(&session_leaf_id);
    for item in refs.iter().rev() {
        if item.role != "assistant" {
            continue;
        }
        if let Some(file_id) = &item.content_file_id {
            let raw = read_content_file(ctx, base_url, &ctx.tenant, inner_fields, file_id)?;
            let text = content_string_to_text(&raw);
            if !text.trim().is_empty() {
                return Ok(text);
            }
        }
        if let Some(inline) = &item.inline_content {
            let text = extract_text_from_value(inline);
            if !text.trim().is_empty() {
                return Ok(text);
            }
        }
    }

    Ok(String::new())
}

fn emit_tree_events(
    ctx: &Context,
    base_url: &str,
    headers: &[(String, String)],
    inner_fields: &Value,
    session_id: &str,
    sequence: &mut i64,
) -> Result<(), String> {
    let session_file_id = field_string(inner_fields, &["SessionFileId", "session_file_id"]);
    if session_file_id.is_empty() {
        return Ok(());
    }

    let jsonl = read_session_from_temperfs(ctx, base_url, &ctx.tenant, inner_fields, &session_file_id)?;
    let tree = SessionTree::from_jsonl(&jsonl);
    for entry_id in tree.entry_ids() {
        let Some(entry) = tree.get(entry_id) else {
            continue;
        };
        if entry.entry_type != EntryType::Message {
            continue;
        }

        let role = entry
            .data
            .get("role")
            .and_then(Value::as_str)
            .unwrap_or("");
        let content = load_entry_content(ctx, base_url, inner_fields, entry)?;
        let Some(blocks) = content.as_array() else {
            continue;
        };

        match role {
            "assistant" => {
                for block in blocks {
                    match block.get("type").and_then(Value::as_str).unwrap_or("") {
                        "thinking" => {
                            let _ = create_session_event(
                                ctx,
                                base_url,
                                headers,
                                session_id,
                                *sequence,
                                "agent.thinking",
                                json!({
                                    "Content": serde_json::to_string(&vec![block.clone()])
                                        .unwrap_or_else(|_| "[]".to_string()),
                                }),
                            )?;
                            *sequence += 1;
                        }
                        "tool_use" => {
                            let _ = create_session_event(
                                ctx,
                                base_url,
                                headers,
                                session_id,
                                *sequence,
                                "agent.tool_use",
                                json!({
                                    "ToolUseId": block.get("id").and_then(Value::as_str).unwrap_or(""),
                                    "ToolName": block.get("name").and_then(Value::as_str).unwrap_or(""),
                                    "EvaluatedPermission": "allow",
                                }),
                            )?;
                            *sequence += 1;
                        }
                        _ => {}
                    }
                }
            }
            "user" => {
                for block in blocks {
                    if block.get("type").and_then(Value::as_str) != Some("tool_result") {
                        continue;
                    }
                    let _ = create_session_event(
                        ctx,
                        base_url,
                        headers,
                        session_id,
                        *sequence,
                        "agent.tool_result",
                        json!({
                            "ToolUseId": block.get("tool_use_id").and_then(Value::as_str).unwrap_or(""),
                            "Content": message_blocks_json(
                                &block
                                    .get("content")
                                    .map(extract_text_from_value)
                                    .unwrap_or_default()
                            ),
                        }),
                    )?;
                    *sequence += 1;
                }
            }
            _ => {}
        }
    }

    Ok(())
}

fn load_entry_content(
    ctx: &Context,
    base_url: &str,
    inner_fields: &Value,
    entry: &SessionEntry,
) -> Result<Value, String> {
    if let Some(file_id) = &entry.content_file_id {
        let raw = read_content_file(ctx, base_url, &ctx.tenant, inner_fields, file_id)?;
        return serde_json::from_str(&raw).or_else(|_| Ok(json!(raw)));
    }

    Ok(entry.data.get("content").cloned().unwrap_or_else(|| json!(null)))
}
