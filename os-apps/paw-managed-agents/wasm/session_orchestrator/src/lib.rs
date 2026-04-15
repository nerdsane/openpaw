#[path = "../../common.rs"]
mod common;

use common::{
    create_entity, create_session_event, entity_id, escape_odata_string, field_i64, field_string,
    get_entity, infer_provider, is_terminal_status, managed_tools_enabled, next_session_event_sequence,
    pending_user_prompt, post_absolute_action, post_action, status_of, system_json_headers,
};
use temper_wasm_sdk::prelude::*;
use wasm_helpers::resolve_temper_api_url;

#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        let fields = ctx.entity_state.get("fields").cloned().unwrap_or_else(|| json!({}));
        let base_url = resolve_temper_api_url(&ctx, &fields);
        let headers = system_json_headers(&ctx, &ctx.tenant, &fields);

        match ctx.config.get("mode").map(String::as_str).unwrap_or("start") {
            "check" => check_inner_session(&ctx, &fields, &base_url, &headers),
            "resume" => start_or_resume(&ctx, &fields, &base_url, &headers, true),
            _ => start_or_resume(&ctx, &fields, &base_url, &headers, false),
        }
    })();

    if let Err(error) = result {
        temper_wasm_sdk::set_error_result(&error);
    }
    0
}

fn start_or_resume(
    ctx: &Context,
    fields: &Value,
    base_url: &str,
    headers: &[(String, String)],
    is_resume: bool,
) -> Result<(), String> {
    let session_id = ctx.entity_id.as_str();
    let managed_agent_id = field_string(fields, &["AgentId", "agent_id"]);
    let environment_id = field_string(fields, &["EnvironmentId", "environment_id"]);
    if managed_agent_id.is_empty() || environment_id.is_empty() {
        temper_wasm_sdk::set_success_result(
            "InnerSessionFailed",
            &json!({
                "error_message": "ManagedSession requires AgentId and EnvironmentId.",
                "stop_reason": "error",
            }),
        );
        return Ok(());
    }

    let managed_agent = get_entity(ctx, base_url, headers, "ManagedAgents", &managed_agent_id)?;
    let managed_environment =
        get_entity(ctx, base_url, headers, "ManagedEnvironments", &environment_id)?;

    let tool_rows = common::list_entities(
        ctx,
        base_url,
        headers,
        &format!(
            "/tdata/AgentTools?$filter=AgentId%20eq%20'{}'&$orderby=Name%20asc",
            escape_odata_string(&managed_agent_id)
        ),
    )?;

    let inner_agent_id =
        ensure_inner_agent(ctx, base_url, headers, &managed_agent, &managed_agent_id, &tool_rows)?;
    let computer_id =
        ensure_computer(ctx, base_url, headers, &managed_environment, &environment_id)?;

    let last_consumed = field_i64(fields, &["LastConsumedUserSequence", "last_consumed_user_sequence"]);
    let pending_events = common::list_entities(
        ctx,
        base_url,
        headers,
        &format!(
            "/tdata/SessionEvents?$filter=SessionId%20eq%20'{}'%20and%20Sequence%20gt%20{}&$orderby=Sequence%20asc",
            escape_odata_string(session_id),
            last_consumed
        ),
    )?;
    let (prompt, last_sequence) = pending_user_prompt(&pending_events, last_consumed);
    if prompt.trim().is_empty() {
        temper_wasm_sdk::set_success_result(
            "InnerSessionFailed",
            &json!({
                "error_message": "No pending user input was available to start the managed session.",
                "stop_reason": "error",
            }),
        );
        return Ok(());
    }

    let existing_inner_session_id = field_string(fields, &["InnerSessionId", "inner_session_id"]);
    if is_resume && !existing_inner_session_id.is_empty() {
        if let Ok(existing_inner) = get_entity(ctx, base_url, headers, "Sessions", &existing_inner_session_id) {
            if !is_terminal_status(&status_of(&existing_inner)) {
                let _ = post_absolute_action(
                    ctx,
                    headers,
                    &format!("{base_url}/tdata/Sessions('{existing_inner_session_id}')/OpenPaw.Steer"),
                    &json!({
                        "steering_messages": serde_json::to_string(&vec![json!({ "content": prompt })])
                            .unwrap_or_else(|_| "[]".to_string())
                    }),
                    "steer inner session",
                )?;
                record_running_event(ctx, base_url, headers, session_id)?;
                temper_wasm_sdk::set_success_result(
                    "InnerSessionReady",
                    &json!({
                        "inner_session_id": existing_inner_session_id,
                        "inner_agent_id": inner_agent_id,
                        "computer_id": computer_id,
                        "last_consumed_user_sequence": last_sequence,
                        "inner_session_check_count": 0,
                    }),
                );
                return Ok(());
            }
        }
    }

    let workspace_id = existing_inner_session_id
        .as_str()
        .pipe(|inner_id| {
            if inner_id.is_empty() {
                Ok(String::new())
            } else {
                get_entity(ctx, base_url, headers, "Sessions", inner_id)
                    .map(|entity| field_string(&entity, &["WorkspaceId", "workspace_id"]))
            }
        })
        .unwrap_or_default();

    let created = create_entity(ctx, base_url, headers, "Sessions", &json!({}))?;
    let inner_session_id =
        entity_id(&created).ok_or("create Sessions did not return an entity id")?;

    let model_id = {
        let value = field_string(&managed_agent, &["ModelId", "model_id"]);
        if value.is_empty() {
            "claude-sonnet-4-6".to_string()
        } else {
            value
        }
    };
    let provider = infer_provider(&model_id);
    let system_prompt = {
        let value = field_string(&managed_agent, &["System", "system"]);
        if value.is_empty() {
            "You are a helpful managed agent.".to_string()
        } else {
            value
        }
    };
    let tools_enabled = managed_tools_enabled(&tool_rows);
    let max_turns = "60";

    let mut configure_body = json!({
        "system_prompt": system_prompt,
        "user_message": prompt,
        "model": model_id,
        "provider": provider,
        "tools_enabled": tools_enabled,
        "max_turns": max_turns,
        "temper_api_url": base_url,
        "agent_id": inner_agent_id,
    });
    if !workspace_id.is_empty() {
        configure_body["workspace_id"] = json!(workspace_id);
    }

    let _ = post_absolute_action(
        ctx,
        headers,
        &format!("{base_url}/tdata/Sessions('{inner_session_id}')/OpenPaw.Configure"),
        &configure_body,
        "configure inner session",
    )?;

    record_running_event(ctx, base_url, headers, session_id)?;
    temper_wasm_sdk::set_success_result(
        "InnerSessionReady",
        &json!({
            "inner_session_id": inner_session_id,
            "inner_agent_id": inner_agent_id,
            "computer_id": computer_id,
            "last_consumed_user_sequence": last_sequence,
            "inner_session_check_count": 0,
        }),
    );
    Ok(())
}

fn check_inner_session(
    ctx: &Context,
    fields: &Value,
    base_url: &str,
    headers: &[(String, String)],
) -> Result<(), String> {
    let inner_session_id = field_string(fields, &["InnerSessionId", "inner_session_id"]);
    if inner_session_id.is_empty() {
        temper_wasm_sdk::set_success_result(
            "InnerSessionFailed",
            &json!({
                "error_message": "ManagedSession has no inner session to monitor.",
                "stop_reason": "error",
            }),
        );
        return Ok(());
    }

    let check_count = field_i64(fields, &["InnerSessionCheckCount", "inner_session_check_count"]);
    let max_checks = field_string(fields, &["MaxInnerSessionChecks", "max_inner_session_checks"])
        .parse::<i64>()
        .unwrap_or(180);
    if check_count >= max_checks {
        temper_wasm_sdk::set_success_result(
            "InnerSessionFailed",
            &json!({
                "error_message": format!(
                    "ManagedSession exceeded {} inner-session checks without reaching a terminal state.",
                    max_checks
                ),
                "stop_reason": "error",
            }),
        );
        return Ok(());
    }

    let inner_session = get_entity(ctx, base_url, headers, "Sessions", &inner_session_id)?;
    let status = status_of(&inner_session);
    match status.as_str() {
        "Completed" => temper_wasm_sdk::set_success_result(
            "IdleSession",
            &json!({
                "stop_reason": "user_input_required",
            }),
        ),
        "Failed" | "Cancelled" => {
            let error_message = {
                let error = field_string(&inner_session, &["ErrorMessage", "error_message"]);
                if error.is_empty() {
                    let result = field_string(&inner_session, &["Result", "result"]);
                    if result.is_empty() {
                        format!("Inner session ended with status {status}.")
                    } else {
                        result
                    }
                } else {
                    error
                }
            };
            temper_wasm_sdk::set_success_result(
                "InnerSessionFailed",
                &json!({
                    "error_message": error_message,
                    "stop_reason": "error",
                }),
            );
        }
        _ => temper_wasm_sdk::set_success_result("InnerSessionPending", &json!({})),
    }
    Ok(())
}

fn ensure_inner_agent(
    ctx: &Context,
    base_url: &str,
    headers: &[(String, String)],
    managed_agent: &Value,
    managed_agent_id: &str,
    tool_rows: &[Value],
) -> Result<String, String> {
    let existing = field_string(managed_agent, &["InnerAgentId", "inner_agent_id"]);
    let name = {
        let value = field_string(managed_agent, &["Name", "name"]);
        if value.is_empty() {
            format!("managed-agent-{managed_agent_id}")
        } else {
            value
        }
    };
    let description = field_string(managed_agent, &["Description", "description"]);
    let model_id = {
        let value = field_string(managed_agent, &["ModelId", "model_id"]);
        if value.is_empty() {
            "claude-sonnet-4-6".to_string()
        } else {
            value
        }
    };
    let tools_enabled = managed_tools_enabled(tool_rows);
    let provider = infer_provider(&model_id);

    if existing.is_empty() {
        let created = create_entity(ctx, base_url, headers, "Agents", &json!({}))?;
        let inner_agent_id =
            entity_id(&created).ok_or("create Agents did not return an entity id")?;
        let _ = post_absolute_action(
            ctx,
            headers,
            &format!("{base_url}/tdata/Agents('{inner_agent_id}')/OpenPaw.Configure"),
            &json!({
                "name": name,
                "role": "managed-agent",
                "description": description,
                "source_app_id": "paw-managed-agents",
                "model": model_id,
                "provider": provider,
                "tools_enabled": tools_enabled,
                "max_turns": "60",
            }),
            "configure inner agent",
        )?;
        let _ = post_action(
            ctx,
            base_url,
            headers,
            "ManagedAgents",
            managed_agent_id,
            "BindInnerAgent",
            &json!({ "inner_agent_id": inner_agent_id }),
            false,
        )?;
        Ok(inner_agent_id)
    } else {
        let _ = post_absolute_action(
            ctx,
            headers,
            &format!("{base_url}/tdata/Agents('{existing}')/OpenPaw.Update"),
            &json!({
                "description": description,
                "model": model_id,
                "tools_enabled": tools_enabled,
                "max_turns": "60",
            }),
            "update inner agent",
        )?;
        Ok(existing)
    }
}

fn ensure_computer(
    ctx: &Context,
    base_url: &str,
    headers: &[(String, String)],
    managed_environment: &Value,
    environment_id: &str,
) -> Result<String, String> {
    let existing = field_string(managed_environment, &["ComputerId", "computer_id"]);
    if !existing.is_empty() {
        return Ok(existing);
    }

    let provisioned = post_action(
        ctx,
        base_url,
        headers,
        "ManagedEnvironments",
        environment_id,
        "ProvisionComputer",
        &json!({}),
        true,
    )?;

    let computer_id = field_string(&provisioned, &["ComputerId", "computer_id"]);
    if !computer_id.is_empty() {
        return Ok(computer_id);
    }

    let refreshed = get_entity(ctx, base_url, headers, "ManagedEnvironments", environment_id)?;
    let computer_id = field_string(&refreshed, &["ComputerId", "computer_id"]);
    if computer_id.is_empty() {
        Err("ProvisionComputer completed without binding a ComputerId.".to_string())
    } else {
        Ok(computer_id)
    }
}

fn record_running_event(
    ctx: &Context,
    base_url: &str,
    headers: &[(String, String)],
    session_id: &str,
) -> Result<(), String> {
    let sequence = next_session_event_sequence(ctx, base_url, headers, session_id)?;
    let _ = create_session_event(
        ctx,
        base_url,
        headers,
        session_id,
        sequence,
        "session.status_running",
        json!({}),
    )?;
    Ok(())
}

trait Pipe: Sized {
    fn pipe<T, F: FnOnce(Self) -> T>(self, func: F) -> T {
        func(self)
    }
}

impl<T> Pipe for T {}
