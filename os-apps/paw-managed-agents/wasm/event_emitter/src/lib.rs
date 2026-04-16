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
                    "InputTokens": 0,
                    "OutputTokens": 0,
                    "StopReasonEventIds": "[]",
                    "LastEmittedResultHash": "missing-inner-session",
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
        if inner_status != "Completed" {
            return Err(format!(
                "event_emitter only supports completed inner sessions, got status {inner_status}"
            ));
        }
        let result_text = resolve_result_text(&ctx, &base_url, &inner_fields)?;
        let stop_reason = {
            let existing = field_string(&fields, &["StopReason", "stop_reason"]);
            if existing.is_empty() {
                "user_input_required".to_string()
            } else {
                existing
            }
        };
        let result_hash = format!("{inner_session_id}:{inner_status}:{result_text}");
        let previous_hash =
            field_string(&fields, &["LastEmittedResultHash", "last_emitted_result_hash"]);
        let previous_inner_session_id = field_string(
            &fields,
            &["LastEmittedInnerSessionId", "last_emitted_inner_session_id"],
        );
        let previous_tree_index =
            field_i64(&fields, &["LastEmittedTreeIndex", "last_emitted_tree_index"]).max(0)
                as usize;
        let tree_index = current_tree_index(&ctx, &base_url, &inner_fields)?;
        let emit_from_index =
            emission_start_index(&inner_session_id, &previous_inner_session_id, previous_tree_index);
        let has_new_tree_entries = (tree_index as usize) > emit_from_index;

        if previous_hash != result_hash || has_new_tree_entries {
            let mut sequence = next_session_event_sequence(&ctx, &base_url, &headers, &ctx.entity_id)?;
            if has_new_tree_entries {
                emit_tree_events(
                    &ctx,
                    &base_url,
                    &headers,
                    &inner_fields,
                    &ctx.entity_id,
                    emit_from_index,
                    &mut sequence,
                )?;
            }

            if previous_hash != result_hash && !result_text.trim().is_empty() {
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

            if previous_hash != result_hash {
                let _ = create_session_event(
                    &ctx,
                    &base_url,
                    &headers,
                    &ctx.entity_id,
                    sequence,
                    "session.status_idle",
                    json!({
                        "StopReason": stop_reason,
                        "StopReasonEventIds": "[]",
                    }),
                )?;
            }
        }

        temper_wasm_sdk::set_success_result(
            "UpdateUsage",
            &json!({
                "InputTokens": field_i64(&inner_fields, &["InputTokens", "input_tokens"]),
                "OutputTokens": field_i64(&inner_fields, &["OutputTokens", "output_tokens"]),
                "StopReason": stop_reason,
                "StopReasonEventIds": "[]",
                "LastEmittedResultHash": result_hash,
                "LastEmittedInnerSessionId": inner_session_id,
                "LastEmittedTreeIndex": tree_index,
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
    start_index: usize,
    sequence: &mut i64,
) -> Result<(), String> {
    let session_file_id = field_string(inner_fields, &["SessionFileId", "session_file_id"]);
    if session_file_id.is_empty() {
        return Ok(());
    }

    let jsonl = read_session_from_temperfs(ctx, base_url, &ctx.tenant, inner_fields, &session_file_id)?;
    let tree = SessionTree::from_jsonl(&jsonl);
    for entry_id in tree.entry_ids().iter().skip(start_index) {
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
                                    "Input": block.get("input").map(|value| value.to_string()).unwrap_or_default(),
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

fn current_tree_index(ctx: &Context, base_url: &str, inner_fields: &Value) -> Result<i64, String> {
    let session_file_id = field_string(inner_fields, &["SessionFileId", "session_file_id"]);
    if session_file_id.is_empty() {
        return Ok(0);
    }

    let jsonl = read_session_from_temperfs(ctx, base_url, &ctx.tenant, inner_fields, &session_file_id)?;
    let tree = SessionTree::from_jsonl(&jsonl);
    Ok(tree.entry_ids().len() as i64)
}

fn emission_start_index(
    current_inner_session_id: &str,
    previous_inner_session_id: &str,
    previous_tree_index: usize,
) -> usize {
    if current_inner_session_id == previous_inner_session_id {
        previous_tree_index
    } else {
        0
    }
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

#[cfg(test)]
mod tests {
    use super::emission_start_index;

    #[test]
    fn emission_start_index_resumes_from_last_emitted_count_for_same_inner_session() {
        assert_eq!(emission_start_index("inner-1", "inner-1", 3), 3);
    }

    #[test]
    fn emission_start_index_resets_when_inner_session_changes() {
        assert_eq!(emission_start_index("inner-2", "inner-1", 3), 0);
    }
}
