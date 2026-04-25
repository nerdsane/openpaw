//! Provider Response Applier — staged Session-turn WASM for persistence and routing.
//!
//! Owns the `ApplyingProviderResponse` phase:
//! - read prepared/provider-response artifacts
//! - append assistant output back into session storage
//! - externalize oversized assistant content when needed
//! - derive the next Session action
//! - route to `ProcessToolCalls`, `CheckSteering`, or `RecordResult`
//!
//! Build: `cargo build --target wasm32-unknown-unknown --release`

use session_turn_artifacts::{
    PreparedContextArtifact, ProviderResponseArtifact, build_provider_response_applier_base_params,
};
use session_tree_lib::SessionTree;
use temper_wasm_sdk::prelude::*;
use wasm_helpers::{
    create_content_file, read_content_file, read_session_from_temperfs, resolve_temper_api_url,
    runtime_headers, write_session_to_temperfs, write_temperfs_value_with_retry,
};

const SESSION_ENTRY_FILE_THRESHOLD_BYTES: usize = 4096;

#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    if let Err(err) = run_provider_response_applier() {
        set_error_result(&err);
    }
    0
}

pub fn run_provider_response_applier() -> Result<(), String> {
    let ctx = Context::from_host()?;
    ctx.log("info", "provider_response_applier: starting");

    let fields = ctx
        .entity_state
        .get("fields")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let prepared_context_file_id = fields
        .get("prepared_context_file_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let provider_response_file_id = fields
        .get("provider_response_file_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if prepared_context_file_id.is_empty() || provider_response_file_id.is_empty() {
        return Err(
            "provider_response_applier: missing prepared_context_file_id or provider_response_file_id"
                .to_string(),
        );
    }

    let temper_api_url = resolve_temper_api_url(&ctx, &fields);
    let tenant = &ctx.tenant;
    let prepared = read_prepared_context_artifact(
        &ctx,
        &temper_api_url,
        tenant,
        &fields,
        prepared_context_file_id,
    )?;
    let response = read_provider_response_artifact(
        &ctx,
        &temper_api_url,
        tenant,
        &fields,
        provider_response_file_id,
    )?;

    let mut messages = prepared.messages.clone();
    messages.push(json!({
        "role": "assistant",
        "content": response.content.clone(),
    }));

    let updated_conversation = serde_json::to_string(&messages).unwrap_or_default();
    if !prepared.conversation_file_id.is_empty() && !prepared.use_session_tree {
        write_conversation_to_temperfs(
            &ctx,
            &temper_api_url,
            tenant,
            &fields,
            &prepared.conversation_file_id,
            &updated_conversation,
        )?;
    }
    let inline_conversation = if prepared.conversation_file_id.is_empty() {
        Some(updated_conversation)
    } else {
        None
    };

    match response.stop_reason.as_str() {
        "tool_use" => {
            let tool_calls = extract_tool_calls(&response.content);
            let new_leaf = append_assistant_response_to_session_tree(
                &ctx,
                &prepared,
                &temper_api_url,
                tenant,
                &fields,
                &response.content,
                response.output_tokens as usize,
            )?;

            let mut params = build_provider_response_applier_base_params(&prepared, &response);
            params["pending_tool_calls"] =
                json!(serde_json::to_string(&tool_calls).unwrap_or_default());
            if let Some(leaf) = new_leaf {
                params["session_leaf_id"] = json!(leaf);
            }
            if let Some(conversation) = inline_conversation {
                params["conversation"] = json!(conversation);
            }
            set_success_result("ProcessToolCalls", &params);
        }
        "end_turn" | "stop" => {
            let result_text = extract_text_response(&response.content);
            let new_leaf = append_assistant_response_to_session_tree(
                &ctx,
                &prepared,
                &temper_api_url,
                tenant,
                &fields,
                &response.content,
                response.output_tokens as usize,
            )?;

            let mut params = build_provider_response_applier_base_params(&prepared, &response);
            params["result"] = json!(result_text);
            params["session_leaf_id"] = json!(new_leaf);

            let max_follow_ups = fields
                .get("max_follow_ups")
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse::<i64>().ok())
                .unwrap_or(5);
            if max_follow_ups > 0 {
                set_success_result("CheckSteering", &params);
            } else {
                if let Some(conversation) = inline_conversation {
                    params["conversation"] = json!(conversation);
                }
                set_success_result("RecordResult", &params);
            }
        }
        other => return Err(format!("unsupported stop_reason: {other}")),
    }

    Ok(())
}

fn read_prepared_context_artifact(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    fields: &Value,
    file_id: &str,
) -> Result<PreparedContextArtifact, String> {
    let raw = read_content_file(ctx, temper_api_url, tenant, fields, file_id)?;
    serde_json::from_str(&raw).map_err(|e| format!("parse prepared context artifact: {e}"))
}

fn read_provider_response_artifact(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    fields: &Value,
    file_id: &str,
) -> Result<ProviderResponseArtifact, String> {
    let raw = read_content_file(ctx, temper_api_url, tenant, fields, file_id)?;
    serde_json::from_str(&raw).map_err(|e| format!("parse provider response artifact: {e}"))
}

fn append_assistant_response_to_session_tree(
    ctx: &Context,
    prepared: &PreparedContextArtifact,
    temper_api_url: &str,
    tenant: &str,
    fields: &Value,
    content: &Value,
    output_tokens: usize,
) -> Result<Option<String>, String> {
    if !prepared.use_session_tree {
        return Ok(None);
    }

    let session_jsonl =
        read_session_from_temperfs(ctx, temper_api_url, tenant, fields, &prepared.session_file_id)?;
    let mut tree = SessionTree::from_jsonl(&session_jsonl);
    let content_str = serde_json::to_string(content).unwrap_or_default();
    let (new_leaf, externalized) =
        if !prepared.workspace_id.is_empty() && should_store_entry_as_file(&content_str) {
            match create_content_file_for_entry(
                ctx,
                temper_api_url,
                tenant,
                &prepared.workspace_id,
                &format!("a-{}", tree.len()),
                &content_str,
            ) {
                Ok(content_file_id) => {
                    let (leaf, _) = tree.append_assistant_message_file(
                        &prepared.session_leaf_id,
                        &content_file_id,
                        None,
                        output_tokens,
                    );
                    (leaf, true)
                }
                Err(_) => {
                    let (leaf, _) = tree.append_assistant_message(
                        &prepared.session_leaf_id,
                        content,
                        output_tokens,
                    );
                    (leaf, false)
                }
            }
        } else {
            let (leaf, _) =
                tree.append_assistant_message(&prepared.session_leaf_id, content, output_tokens);
            (leaf, false)
        };

    if externalized {
        emit_metric_ignore(
            ctx,
            "temper_session_large_content_externalized_total",
            1.0,
            &session_metric_tags("", ""),
            Some("count"),
        );
    }

    write_session_to_temperfs(
        ctx,
        temper_api_url,
        tenant,
        fields,
        &prepared.session_file_id,
        &tree.to_jsonl(),
    )?;
    Ok(Some(new_leaf))
}

fn extract_tool_calls(content: &Value) -> Vec<Value> {
    content
        .as_array()
        .into_iter()
        .flatten()
        .filter(|block| block.get("type").and_then(|v| v.as_str()) == Some("tool_use"))
        .cloned()
        .collect()
}

fn extract_text_response(content: &Value) -> String {
    content
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|block| {
            (block.get("type").and_then(|v| v.as_str()) == Some("text"))
                .then(|| block.get("text").and_then(|v| v.as_str()))
                .flatten()
                .map(ToOwned::to_owned)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn write_conversation_to_temperfs(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    fields: &Value,
    file_id: &str,
    conversation_json: &str,
) -> Result<(), String> {
    let url = format!("{temper_api_url}/tdata/Files('{file_id}')/$value");
    let headers = runtime_headers(ctx, tenant, fields, Some("application/json"), None);
    let body = format!("{{\"messages\":{conversation_json}}}");
    write_temperfs_value_with_retry(ctx, &url, &headers, &body, "TemperFS conversation write")?;
    Ok(())
}

fn create_content_file_for_entry(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    workspace_id: &str,
    entry_id: &str,
    content: &str,
) -> Result<String, String> {
    create_content_file(
        ctx,
        temper_api_url,
        tenant,
        workspace_id,
        &format!("msg-{entry_id}.txt"),
        content,
    )
}

fn should_store_entry_as_file(content: &str) -> bool {
    content.len() > SESSION_ENTRY_FILE_THRESHOLD_BYTES
}

fn session_metric_tags(provider: &str, model: &str) -> Value {
    json!({
        "provider": provider,
        "model": model,
    })
}

fn emit_metric_ignore(ctx: &Context, name: &str, value: f64, tags: &Value, kind: Option<&str>) {
    let _ = ctx.emit_metric(name, value, tags, kind);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_tool_calls_only() {
        let tool_calls = extract_tool_calls(&json!([
            {"type": "text", "text": "hello"},
            {"type": "tool_use", "id": "tool-1", "name": "temper.get", "input": {"id": "x"}},
            {"type": "tool_use", "id": "tool-2", "name": "temper.list", "input": {}}
        ]));

        assert_eq!(tool_calls.len(), 2);
        assert_eq!(tool_calls[0]["id"], "tool-1");
        assert_eq!(tool_calls[1]["id"], "tool-2");
    }

    #[test]
    fn extracts_text_response_blocks() {
        let text = extract_text_response(&json!([
            {"type": "text", "text": "first"},
            {"type": "tool_use", "id": "tool-1"},
            {"type": "text", "text": "second"}
        ]));

        assert_eq!(text, "first\nsecond");
    }

    #[test]
    fn stores_large_entries_as_files_only_after_threshold() {
        assert!(!should_store_entry_as_file(&"a".repeat(SESSION_ENTRY_FILE_THRESHOLD_BYTES)));
        assert!(should_store_entry_as_file(
            &"a".repeat(SESSION_ENTRY_FILE_THRESHOLD_BYTES + 1)
        ));
    }
}
