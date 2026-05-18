//! Provider Response Applier — staged Session-turn WASM for persistence and routing.
//!
//! Owns the `ApplyingProviderResponse` phase:
//! - read prepared/provider-response artifacts
//! - append assistant output back into session storage
//! - externalize oversized assistant content when needed
//! - derive the next Session action
//! - route to `ProcessToolCalls`, `CheckSteering`, `RecordResult`, or `RecordResultNoReply`
//!
//! Build: `cargo build --target wasm32-unknown-unknown --release`

use session_tree_lib::SessionTree;
use session_turn_artifacts::{
    PreparedContextArtifact, ProviderResponseArtifact, build_provider_response_applier_base_params,
    parse_prepared_context_artifact, parse_provider_response_artifact,
};
use temper_wasm_sdk::prelude::*;
use wasm_helpers::{
    append_session_entry_inline, create_content_file, is_session_entries_ref, read_content_file,
    read_session_from_temperfs, resolve_temper_api_url, runtime_headers, write_session_to_temperfs,
    write_temperfs_value_with_retry,
};

const SESSION_ENTRY_FILE_THRESHOLD_BYTES: usize = 4096;
const DEFAULT_PROVIDER_RESPONSE_APPLY_BUDGET_MS: i64 = 30_000;

#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    if let Err(err) = run_provider_response_applier() {
        set_error_result(&err);
    }
    0
}

pub fn run_provider_response_applier() -> Result<(), String> {
    let started_at = Context::get_time_millis();
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
    let prepared_context_inline_json =
        read_state_string_field(&ctx, &fields, "prepared_context_inline_json");
    let provider_response_inline_json =
        read_state_string_field(&ctx, &fields, "provider_response_inline_json");
    if (prepared_context_file_id.is_empty() && prepared_context_inline_json.is_empty())
        || (provider_response_file_id.is_empty() && provider_response_inline_json.is_empty())
    {
        return Err(
            "provider_response_applier: missing prepared/provider response inline JSON or file IDs"
                .to_string(),
        );
    }

    let temper_api_url = resolve_temper_api_url(&ctx, &fields);
    let tenant = &ctx.tenant;
    let apply_budget_ms = configured_budget_ms(
        &ctx,
        &fields,
        "provider_response_apply_budget_ms",
        DEFAULT_PROVIDER_RESPONSE_APPLY_BUDGET_MS,
    );
    let read_prepared_started_at = Context::get_time_millis();
    let prepared_result = read_prepared_context_artifact(
        &ctx,
        &temper_api_url,
        tenant,
        &fields,
        prepared_context_file_id,
        &prepared_context_inline_json,
    );
    emit_phase_step_duration(
        &ctx,
        "provider_response_applier",
        "read_prepared_artifact",
        read_prepared_started_at,
        if prepared_result.is_ok() {
            "ok"
        } else {
            "error"
        },
    );
    let prepared = prepared_result?;
    check_phase_budget(
        &ctx,
        "provider_response_applier",
        started_at,
        apply_budget_ms,
        "read_prepared_artifact",
    )?;

    let read_response_started_at = Context::get_time_millis();
    let response_result = read_provider_response_artifact(
        &ctx,
        &temper_api_url,
        tenant,
        &fields,
        provider_response_file_id,
        &provider_response_inline_json,
    );
    emit_phase_step_duration(
        &ctx,
        "provider_response_applier",
        "read_provider_response_artifact",
        read_response_started_at,
        if response_result.is_ok() {
            "ok"
        } else {
            "error"
        },
    );
    let response = response_result?;
    check_phase_budget(
        &ctx,
        "provider_response_applier",
        started_at,
        apply_budget_ms,
        "read_provider_response_artifact",
    )?;

    let legacy_conversation = legacy_updated_conversation_payload(&prepared, &response);
    if let Some(ref updated_conversation) = legacy_conversation
        && !prepared.conversation_file_id.is_empty()
    {
        write_conversation_to_temperfs(
            &ctx,
            &temper_api_url,
            tenant,
            &fields,
            &prepared.conversation_file_id,
            updated_conversation,
        )?;
    }
    let inline_conversation =
        if !prepared.use_session_tree && prepared.conversation_file_id.is_empty() {
            legacy_conversation
        } else {
            None
        };

    match response.stop_reason.as_str() {
        "tool_use" => {
            let tool_calls = extract_tool_calls(&response.content);
            let append_started_at = Context::get_time_millis();
            let append_result = append_assistant_response_to_session_tree(
                &ctx,
                &prepared,
                &temper_api_url,
                tenant,
                &fields,
                &response.content,
                response.output_tokens as usize,
            );
            emit_phase_step_duration(
                &ctx,
                "provider_response_applier",
                "append_session_tree",
                append_started_at,
                if append_result.is_ok() { "ok" } else { "error" },
            );
            let new_leaf = append_result?;
            note_phase_budget_overrun_after_committed_step(
                &ctx,
                "provider_response_applier",
                started_at,
                apply_budget_ms,
                "append_session_tree",
            );

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
            emit_phase_total_duration(
                &ctx,
                "provider_response_applier",
                started_at,
                "process_tool_calls",
            );
        }
        "end_turn" | "stop" => {
            let result_text = extract_text_response(&response.content);
            let append_started_at = Context::get_time_millis();
            let append_result = append_assistant_response_to_session_tree(
                &ctx,
                &prepared,
                &temper_api_url,
                tenant,
                &fields,
                &response.content,
                response.output_tokens as usize,
            );
            emit_phase_step_duration(
                &ctx,
                "provider_response_applier",
                "append_session_tree",
                append_started_at,
                if append_result.is_ok() { "ok" } else { "error" },
            );
            let new_leaf = append_result?;
            note_phase_budget_overrun_after_committed_step(
                &ctx,
                "provider_response_applier",
                started_at,
                apply_budget_ms,
                "append_session_tree",
            );

            let mut params = build_provider_response_applier_base_params(&prepared, &response);
            params["result"] = json!(result_text);
            params["session_leaf_id"] = json!(new_leaf);

            if should_check_steering(&fields) {
                set_success_result("CheckSteering", &params);
                emit_phase_total_duration(
                    &ctx,
                    "provider_response_applier",
                    started_at,
                    "check_steering",
                );
            } else {
                if let Some(conversation) = inline_conversation {
                    params["conversation"] = json!(conversation);
                }
                let terminal_action = if should_bypass_terminal_reply(&ctx.entity_id, &fields) {
                    "RecordResultNoReply"
                } else {
                    "RecordResult"
                };
                set_success_result(terminal_action, &params);
                emit_phase_total_duration(
                    &ctx,
                    "provider_response_applier",
                    started_at,
                    if terminal_action == "RecordResultNoReply" {
                        "record_result_no_reply"
                    } else {
                        "record_result"
                    },
                );
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
    inline_json: &str,
) -> Result<PreparedContextArtifact, String> {
    let raw = if inline_json.is_empty() {
        read_content_file(ctx, temper_api_url, tenant, fields, file_id)?
    } else {
        inline_json.to_string()
    };
    parse_prepared_context_artifact(&raw)
}

fn read_provider_response_artifact(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    fields: &Value,
    file_id: &str,
    inline_json: &str,
) -> Result<ProviderResponseArtifact, String> {
    let raw = if inline_json.is_empty() {
        read_content_file(ctx, temper_api_url, tenant, fields, file_id)?
    } else {
        inline_json.to_string()
    };
    parse_provider_response_artifact(&raw)
}

fn read_state_string_field(ctx: &Context, fields: &Value, field_name: &str) -> String {
    match ctx.read_field_string(field_name) {
        Ok(value) if !value.is_empty() => value,
        _ => fields
            .get(field_name)
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
    }
}

fn legacy_updated_conversation_payload(
    prepared: &PreparedContextArtifact,
    artifact: &ProviderResponseArtifact,
) -> Option<String> {
    if prepared.use_session_tree {
        return None;
    }

    let mut messages = prepared.messages.clone();
    messages.push(json!({
        "role": "assistant",
        "content": artifact.content.clone(),
    }));
    Some(serde_json::to_string(&messages).unwrap_or_default())
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

    if is_session_entries_ref(&prepared.session_file_id) {
        let created = append_session_entry_inline(
            ctx,
            temper_api_url,
            tenant,
            fields,
            &prepared.session_file_id,
            &prepared.session_leaf_id,
            "a",
            "assistant",
            content,
            output_tokens,
        )?;
        return Ok(Some(created.entry_id));
    }

    let session_jsonl = read_session_from_temperfs(
        ctx,
        temper_api_url,
        tenant,
        fields,
        &prepared.session_file_id,
    )?;
    let mut tree = SessionTree::from_jsonl(&session_jsonl);
    let content_str = serde_json::to_string(content).unwrap_or_default();
    let entity_backed_session = is_session_entries_ref(&prepared.session_file_id);
    let (new_leaf, externalized) = if !entity_backed_session
        && !prepared.workspace_id.is_empty()
        && should_store_entry_as_file(&content_str)
    {
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

fn should_check_steering(fields: &Value) -> bool {
    let max_follow_ups = field_i64(fields, "max_follow_ups").unwrap_or(5);
    if max_follow_ups <= 0 {
        return false;
    }

    let follow_up_count = field_i64(fields, "follow_up_count").unwrap_or(0);
    follow_up_count < max_follow_ups && has_queued_steering_messages(fields)
}

fn has_queued_steering_messages(fields: &Value) -> bool {
    let Some(raw) = fields.get("steering_messages").and_then(Value::as_str) else {
        return false;
    };
    serde_json::from_str::<Vec<Value>>(raw)
        .map(|messages| !messages.is_empty())
        .unwrap_or(false)
}

fn should_bypass_terminal_reply(session_id: &str, fields: &Value) -> bool {
    if string_field(fields, &["reply_channel_id", "ReplyChannelId"])
        .filter(|value| !value.trim().is_empty())
        .is_some()
        || string_field(fields, &["reply_thread_id", "ReplyThreadId"])
            .filter(|value| !value.trim().is_empty())
            .is_some()
        || string_field(fields, &["reply_channel_entity_id", "ReplyChannelEntityId"])
            .filter(|value| !value.trim().is_empty())
            .is_some()
        || string_field(fields, &["reply_channel_type", "ReplyChannelType"])
            .filter(|value| !value.trim().is_empty())
            .is_some()
    {
        return false;
    }

    let reply_route_source = string_field(fields, &["reply_route_source", "ReplyRouteSource"])
        .unwrap_or("")
        .trim();
    if !reply_route_source.is_empty() && reply_route_source != "direct_no_reply" {
        return false;
    }

    if string_field(fields, &["parent_session_id", "ParentSessionId"])
        .filter(|value| !value.trim().is_empty())
        .is_some()
    {
        return false;
    }

    let session_id = session_id.trim();
    if reply_route_source == "direct_no_reply" {
        return !session_id.is_empty();
    }

    let agent_id = string_field(fields, &["agent_id", "AgentId"])
        .unwrap_or("")
        .trim();
    !session_id.is_empty() && (agent_id.is_empty() || agent_id == session_id)
}

fn string_field<'a>(fields: &'a Value, names: &[&str]) -> Option<&'a str> {
    names.iter().find_map(|name| fields.get(*name)?.as_str())
}

fn field_i64(fields: &Value, field_name: &str) -> Option<i64> {
    fields
        .get(field_name)
        .and_then(|value| value.as_i64().or_else(|| value.as_str()?.parse().ok()))
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

fn elapsed_ms_since(started_at: i64) -> i64 {
    Context::get_time_millis().saturating_sub(started_at)
}

fn configured_budget_ms(ctx: &Context, fields: &Value, key: &str, default_value: i64) -> i64 {
    fields
        .get(key)
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<i64>().ok())
        .or_else(|| ctx.config.get(key).and_then(|s| s.parse::<i64>().ok()))
        .filter(|value| *value > 0)
        .unwrap_or(default_value)
}

fn emit_phase_step_duration(
    ctx: &Context,
    phase: &str,
    step: &str,
    started_at: i64,
    result: &str,
) -> i64 {
    let elapsed_ms = elapsed_ms_since(started_at);
    emit_metric_ignore(
        ctx,
        "temper_session_phase_step_duration_ms",
        elapsed_ms as f64,
        &json!({
            "phase": phase,
            "step": step,
            "result": result,
        }),
        Some("histogram"),
    );
    ctx.log(
        "info",
        &format!("session_phase phase={phase} step={step} result={result} elapsed_ms={elapsed_ms}"),
    );
    elapsed_ms
}

fn emit_phase_total_duration(ctx: &Context, phase: &str, started_at: i64, result: &str) -> i64 {
    let elapsed_ms = elapsed_ms_since(started_at);
    emit_metric_ignore(
        ctx,
        "temper_session_phase_duration_ms",
        elapsed_ms as f64,
        &json!({
            "phase": phase,
            "result": result,
        }),
        Some("histogram"),
    );
    elapsed_ms
}

fn check_phase_budget(
    ctx: &Context,
    phase: &str,
    started_at: i64,
    budget_ms: i64,
    last_step: &str,
) -> Result<(), String> {
    let elapsed_ms = elapsed_ms_since(started_at);
    if elapsed_ms <= budget_ms {
        return Ok(());
    }

    emit_metric_ignore(
        ctx,
        "temper_session_phase_budget_exceeded_total",
        1.0,
        &json!({
            "phase": phase,
            "last_step": last_step,
        }),
        Some("count"),
    );
    Err(format!(
        "{phase}: exceeded local budget after {last_step} (elapsed_ms={elapsed_ms}, budget_ms={budget_ms})"
    ))
}

fn note_phase_budget_overrun_after_committed_step(
    ctx: &Context,
    phase: &str,
    started_at: i64,
    budget_ms: i64,
    committed_step: &str,
) {
    let elapsed_ms = elapsed_ms_since(started_at);
    if elapsed_ms <= budget_ms {
        return;
    }

    emit_metric_ignore(
        ctx,
        "temper_session_phase_budget_exceeded_after_commit_total",
        1.0,
        &json!({
            "phase": phase,
            "committed_step": committed_step,
        }),
        Some("count"),
    );
    ctx.log(
        "warn",
        &format!(
            "{phase}: exceeded local budget after committed {committed_step}; continuing because the session append already succeeded (elapsed_ms={elapsed_ms}, budget_ms={budget_ms})"
        ),
    );
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
        assert!(!should_store_entry_as_file(
            &"a".repeat(SESSION_ENTRY_FILE_THRESHOLD_BYTES)
        ));
        assert!(should_store_entry_as_file(
            &"a".repeat(SESSION_ENTRY_FILE_THRESHOLD_BYTES + 1)
        ));
    }

    #[test]
    fn empty_steering_queue_fast_finalizes() {
        let fields = json!({
            "max_follow_ups": "5",
            "follow_up_count": 0,
            "steering_messages": "[]"
        });

        assert!(!should_check_steering(&fields));
    }

    #[test]
    fn missing_or_invalid_steering_queue_fast_finalizes() {
        assert!(!should_check_steering(&json!({"max_follow_ups": "5"})));
        assert!(!should_check_steering(&json!({
            "max_follow_ups": "5",
            "steering_messages": "not-json"
        })));
    }

    #[test]
    fn queued_steering_checks_when_budget_remains() {
        let fields = json!({
            "max_follow_ups": "5",
            "follow_up_count": 1,
            "steering_messages": r#"[{"content":"please revise"}]"#
        });

        assert!(should_check_steering(&fields));
    }

    #[test]
    fn exhausted_steering_budget_fast_finalizes() {
        let fields = json!({
            "max_follow_ups": "2",
            "follow_up_count": 2,
            "steering_messages": r#"[{"content":"too late"}]"#
        });

        assert!(!should_check_steering(&fields));
    }

    #[test]
    fn zero_max_follow_ups_fast_finalizes_even_with_queue() {
        let fields = json!({
            "max_follow_ups": "0",
            "follow_up_count": 0,
            "steering_messages": r#"[{"content":"queued"}]"#
        });

        assert!(!should_check_steering(&fields));
    }

    #[test]
    fn terminal_reply_bypass_only_matches_unrouted_direct_sessions() {
        assert!(should_bypass_terminal_reply("ss-direct", &json!({})));
        assert!(should_bypass_terminal_reply(
            "ss-direct",
            &json!({"agent_id": "ss-direct"})
        ));
        assert!(should_bypass_terminal_reply(
            "ss-direct",
            &json!({
                "agent_id": "aj-direct",
                "reply_route_source": "direct_no_reply"
            })
        ));

        for fields in [
            json!({"reply_channel_id": "discord-channel", "reply_thread_id": "thread"}),
            json!({"reply_thread_id": "thread"}),
            json!({"reply_route_source": "channel_message"}),
            json!({
                "reply_route_source": "direct_no_reply",
                "reply_channel_id": "discord-channel",
                "reply_thread_id": "thread"
            }),
            json!({"parent_session_id": "ss-parent"}),
            json!({"agent_id": "aj-agent"}),
        ] {
            assert!(
                !should_bypass_terminal_reply("ss-direct", &fields),
                "ambiguous or channel-bound fields must keep RecordResult delivery path: {fields}"
            );
        }
    }

    #[test]
    fn fresh_session_tree_response_apply_does_not_build_legacy_conversation_payload() {
        let prepared = PreparedContextArtifact {
            version: 1,
            messages: vec![json!({"role": "user", "content": "hello"})],
            tools: vec![],
            system_prompt: "You are concise.".to_string(),
            system_prompt_hash: "hash-123".to_string(),
            system_prompt_file_id: "file-system".to_string(),
            conversation_file_id: String::new(),
            session_file_id: "session-file".to_string(),
            session_leaf_id: "leaf-1".to_string(),
            workspace_id: "workspace-1".to_string(),
            use_session_tree: true,
            context_tokens: 12,
            context_bytes: 128,
            entries_loaded: 1,
            content_files_loaded: 0,
            prune_tool_results_after_turns: 4,
        };
        let artifact = ProviderResponseArtifact {
            version: 1,
            provider: "openai_codex".to_string(),
            model: "gpt-5.4-codex".to_string(),
            content: json!([{"type": "tool_use", "id": "call_1", "name": "temper.read", "input": {"path": "/x"}}]),
            stop_reason: "tool_use".to_string(),
            input_tokens: 10,
            output_tokens: 20,
            cache_read_input_tokens: 0,
            cache_creation_input_tokens: 0,
            request_bytes: 256,
            response_bytes: 512,
        };

        assert!(legacy_updated_conversation_payload(&prepared, &artifact).is_none());
    }

    #[test]
    fn legacy_inline_response_apply_keeps_conversation_payload() {
        let prepared = PreparedContextArtifact {
            version: 1,
            messages: vec![json!({"role": "user", "content": "hello"})],
            tools: vec![],
            system_prompt: "You are concise.".to_string(),
            system_prompt_hash: "hash-123".to_string(),
            system_prompt_file_id: "file-system".to_string(),
            conversation_file_id: String::new(),
            session_file_id: String::new(),
            session_leaf_id: String::new(),
            workspace_id: String::new(),
            use_session_tree: false,
            context_tokens: 12,
            context_bytes: 128,
            entries_loaded: 1,
            content_files_loaded: 0,
            prune_tool_results_after_turns: 4,
        };
        let artifact = ProviderResponseArtifact {
            version: 1,
            provider: "anthropic".to_string(),
            model: "claude-sonnet-4-6".to_string(),
            content: json!([{"type": "text", "text": "hi"}]),
            stop_reason: "end_turn".to_string(),
            input_tokens: 10,
            output_tokens: 20,
            cache_read_input_tokens: 0,
            cache_creation_input_tokens: 0,
            request_bytes: 256,
            response_bytes: 512,
        };

        let payload = legacy_updated_conversation_payload(&prepared, &artifact)
            .expect("legacy inline mode still needs a conversation param");
        let messages: Vec<Value> = serde_json::from_str(&payload).unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[1]["role"], "assistant");
    }
}
