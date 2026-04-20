use session_tree_lib::SessionTree;
use temper_wasm_sdk::prelude::*;
use wasm_helpers::{create_content_file, runtime_headers, runtime_headers_for_workspace};

const DEFAULT_TOOLS_ENABLED: &str = "temper_create,temper_get,temper_list,temper_action,temper_patch,temper_submit_specs,temper_show_spec,temper_specs,temper_upload_wasm,temper_get_trajectories,temper_get_insights,temper_get_decisions,temper_poll_decision,temper_approve_decision,temper_deny_decision,temper_submit_policy,temper_list_policies,temper_get_policy,temper_update_policy,temper_delete_policy,temper_install_app,temper_list_apps,temper_spawn_session,temper_list_sessions,temper_abort_session,temper_steer_session,temper_save_memory,temper_recall_memory,temper_write,temper_read,temper_run_coding_agent,temper_get_secret,temper_datadog_query,temper_railway,temper_vercel,temper_web_search,temper_web_fetch,read,write,edit,bash";
const PLAN_MODE_TOOLS: &str = "temper_create,temper_get,temper_list,temper_action,temper_specs,temper_show_spec,temper_save_memory,temper_recall_memory,temper_read,temper_write,temper_web_search,temper_web_fetch,temper_get_trajectories,temper_get_insights,read,bash";
const DEFAULT_WORKDIR: &str = "/workspace";

#[derive(Debug, Clone, PartialEq, Eq)]
struct TraceContextFields {
    trace_id: String,
    span_id: String,
}

impl AsRef<TraceContextFields> for TraceContextFields {
    fn as_ref(&self) -> &TraceContextFields {
        self
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[unsafe(no_mangle)]
extern "C" fn host_get_context(_buf_ptr: i32, _buf_len: i32) -> i32 {
    -1
}

#[cfg(not(target_arch = "wasm32"))]
#[unsafe(no_mangle)]
extern "C" fn host_http_call(
    _method_ptr: i32,
    _method_len: i32,
    _url_ptr: i32,
    _url_len: i32,
    _headers_ptr: i32,
    _headers_len: i32,
    _body_ptr: i32,
    _body_len: i32,
    _result_buf_ptr: i32,
    _result_buf_len: i32,
) -> i32 {
    -1
}

#[cfg(not(target_arch = "wasm32"))]
#[unsafe(no_mangle)]
extern "C" fn host_log(_level_ptr: i32, _level_len: i32, _msg_ptr: i32, _msg_len: i32) {}

#[cfg(not(target_arch = "wasm32"))]
#[unsafe(no_mangle)]
extern "C" fn host_set_result(_ptr: i32, _len: i32) {}

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
        let channel_id = str_field(&fields, &["channel_id", "ChannelId"]).unwrap_or("");
        let default_agent_config =
            str_field(&fields, &["default_agent_config", "DefaultAgentConfig"]).unwrap_or("{}");
        let thread_id = str_field(&fields, &["thread_id", "ThreadId"]).unwrap_or("");
        let author_id = str_field(&fields, &["author_id", "AuthorId"]).unwrap_or("");
        let content = str_field(&fields, &["content", "Content"]).unwrap_or("");
        // Read command from trigger_params (the current ReceiveMessage call), NOT
        // from entity_state.fields which persists the value from prior calls.
        // Regular messages don't include "command", so it must default to "" —
        // reading from entity state would re-use a stale "/plan" command forever.
        let command = ctx
            .trigger_params
            .get("command")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let trace_context = trace_context_from_trigger_params(&ctx.trigger_params);
        if channel_id.is_empty() || author_id.is_empty() {
            return Err("route_message: missing channel_id/author_id".to_string());
        }
        // For DMs without threads, default thread_id to channel_id
        let thread_id = if thread_id.is_empty() {
            channel_id
        } else {
            thread_id
        };

        let existing_cs = find_active_session(
            &ctx,
            &temper_api_url,
            &ctx.tenant,
            channel_id,
            thread_id,
            author_id,
        )?;

        // /reset command: cancel active session, expire ChannelSession, start fresh (ADR-0025)
        if command == "reset" {
            let route = find_route(&ctx, &temper_api_url, &ctx.tenant, channel_id)?;
            let route_config = route
                .as_ref()
                .and_then(|value| nested_str_field(value, &["AgentConfig", "agent_config"]))
                .filter(|value| !value.trim().is_empty())
                .unwrap_or(default_agent_config);
            let route_agent_id = route
                .as_ref()
                .and_then(|value| nested_str_field(value, &["AgentId", "agent_id"]))
                .unwrap_or("");

            // Clean up existing session/channel-session
            if let Some(ref cs) = existing_cs {
                let cs_id = cs
                    .get("entity_id")
                    .and_then(|v| v.as_str())
                    .or_else(|| nested_str_field(cs, &["Id", "entity_id"]))
                    .unwrap_or_default();
                let session_entity_id =
                    nested_str_field(cs, &["SessionEntityId", "session_entity_id"])
                        .unwrap_or_default();
                if !session_entity_id.is_empty() {
                    cancel_session(&ctx, &temper_api_url, &ctx.tenant, session_entity_id).ok();
                }
                expire_session(&ctx, &temper_api_url, &ctx.tenant, cs_id).ok();
            }

            // Create fresh session with no inherited conversation tree
            let user_msg = if content.is_empty() {
                "[System: The user has reset the conversation. Start fresh.]"
            } else {
                content
            };
            let (new_agent_id, new_session_id) = create_session_for_agent(
                &ctx,
                &temper_api_url,
                &ctx.tenant,
                route_config,
                route_agent_id,
                user_msg,
                "",
                trace_context.as_ref(),
            )?;
            create_channel_session(
                &ctx,
                &temper_api_url,
                &ctx.tenant,
                channel_id,
                thread_id,
                author_id,
                &new_agent_id,
                &new_session_id,
            )?;
            set_success_result("MessageRouted", &json!({ "session_id": new_session_id }));
            return Ok(());
        }

        let session_id = if let Some(cs) = existing_cs {
            let cs_id = cs
                .get("entity_id")
                .and_then(|v| v.as_str())
                .or_else(|| nested_str_field(&cs, &["Id", "entity_id"]))
                .unwrap_or_default()
                .to_string();
            let agent_entity_id =
                nested_str_field(&cs, &["AgentEntityId", "agent_entity_id"])
                    .unwrap_or_default()
                    .to_string();
            let session_entity_id =
                nested_str_field(&cs, &["SessionEntityId", "session_entity_id"])
                    .unwrap_or_default()
                    .to_string();

            if !session_entity_id.is_empty() {
                // Fetch the current Session entity to check its status
                let session = fetch_entity(
                    &ctx,
                    &session_entity_url(&temper_api_url, &session_entity_id),
                    &ctx.tenant,
                )?;
                let session_status =
                    nested_str_field(&session, &["Status", "status"]).unwrap_or("");

                if is_steerable_status(session_status) {
                    resume_session(&ctx, &temper_api_url, &ctx.tenant, &cs_id).ok();
                    if !command.is_empty() {
                        // Slash command: switch mode first, then steer
                        switch_mode_and_steer(
                            &ctx,
                            &temper_api_url,
                            &ctx.tenant,
                            &session_entity_id,
                            command,
                            content,
                        )?;
                        session_entity_id
                    } else if steer_session(
                        &ctx,
                        &temper_api_url,
                        &ctx.tenant,
                        &session_entity_id,
                        content,
                    )
                    .is_ok()
                    {
                        session_entity_id
                    } else {
                        // Steer failed — create a new session under the same agent
                        continue_with_new_session(
                            &ctx,
                            &temper_api_url,
                            &ctx.tenant,
                            &cs,
                            &cs_id,
                            &agent_entity_id,
                            &session,
                            &session_entity_id,
                            content,
                            command,
                            trace_context.as_ref(),
                        )?
                    }
                } else if is_terminal_status(session_status) {
                    // Session is done — create a new session under the same persistent Agent
                    continue_with_new_session(
                        &ctx,
                        &temper_api_url,
                        &ctx.tenant,
                        &cs,
                        &cs_id,
                        &agent_entity_id,
                        &session,
                        &session_entity_id,
                        content,
                        command,
                        trace_context.as_ref(),
                    )?
                } else {
                    // Non-steerable, non-terminal (e.g. Provisioning, WaitingForApproval)
                    // Just wait — don't expire or create a new session
                    session_entity_id
                }
            } else {
                // ChannelSession exists but has no session_entity_id — route fresh
                let route = find_route(&ctx, &temper_api_url, &ctx.tenant, channel_id)?;
                let route_config = route
                    .as_ref()
                    .and_then(|value| nested_str_field(value, &["AgentConfig", "agent_config"]))
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or(default_agent_config);
                let route_agent_id = route
                    .as_ref()
                    .and_then(|value| nested_str_field(value, &["AgentId", "agent_id"]))
                    .unwrap_or("");
                expire_session(&ctx, &temper_api_url, &ctx.tenant, &cs_id).ok();
                let (new_agent_id, new_session_id) = create_session_for_agent(
                    &ctx,
                    &temper_api_url,
                    &ctx.tenant,
                    route_config,
                    route_agent_id,
                    content,
                    command,
                    trace_context.as_ref(),
                )?;
                create_channel_session(
                    &ctx,
                    &temper_api_url,
                    &ctx.tenant,
                    channel_id,
                    thread_id,
                    author_id,
                    &new_agent_id,
                    &new_session_id,
                )?;
                new_session_id
            }
        } else {
            // No existing ChannelSession — route from scratch
            let route = find_route(&ctx, &temper_api_url, &ctx.tenant, channel_id)?;
            let route_config = route
                .as_ref()
                .and_then(|value| nested_str_field(value, &["AgentConfig", "agent_config"]))
                .filter(|value| !value.trim().is_empty())
                .unwrap_or(default_agent_config);
            let route_agent_id = route
                .as_ref()
                .and_then(|value| nested_str_field(value, &["AgentId", "agent_id"]))
                .unwrap_or("");
            let (agent_id, new_session_id) = create_session_for_agent(
                &ctx,
                &temper_api_url,
                &ctx.tenant,
                route_config,
                route_agent_id,
                content,
                command,
                trace_context.as_ref(),
            )?;
            create_channel_session(
                &ctx,
                &temper_api_url,
                &ctx.tenant,
                channel_id,
                thread_id,
                author_id,
                &agent_id,
                &new_session_id,
            )?;
            new_session_id
        };

        ctx.log(
            "info",
            &format!("route_message: routed thread {thread_id} to session {session_id}"),
        );
        set_success_result(
            "",
            &json!({
                "status": "routed",
                "thread_id": thread_id,
                "agent_entity_id": session_id,
            }),
        );
        Ok(())
    })();

    if let Err(error) = result {
        set_error_result(&error);
    }
    0
}

fn resolve_temper_api_url(ctx: &Context, fields: &Value) -> String {
    fields
        .get("temper_api_url")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .or_else(|| {
            ctx.config
                .get("temper_api_url")
                .filter(|s| !s.is_empty())
                .cloned()
        })
        .unwrap_or_else(|| "http://127.0.0.1:3467".to_string())
}

fn odata_headers(ctx: &Context, tenant: &str) -> Vec<(String, String)> {
    let fields = ctx
        .entity_state
        .get("fields")
        .cloned()
        .unwrap_or_else(|| json!({}));
    runtime_headers(
        ctx,
        tenant,
        &fields,
        Some("application/json"),
        Some("application/json"),
    )
}

fn file_headers(
    ctx: &Context,
    tenant: &str,
    workspace_id: &str,
    content_type: Option<&str>,
    accept: Option<&str>,
) -> Vec<(String, String)> {
    if workspace_id.is_empty() {
        runtime_headers(ctx, tenant, &json!({}), content_type, accept)
    } else {
        runtime_headers_for_workspace(ctx, tenant, &json!({}), workspace_id, content_type, accept)
    }
}

fn trace_context_from_trigger_params(trigger_params: &Value) -> Option<TraceContextFields> {
    let trace_id = trigger_params
        .get("gen_ai_parent_trace_id")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())?;
    let span_id = trigger_params
        .get("gen_ai_parent_span_id")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())?;

    Some(TraceContextFields {
        trace_id: trace_id.to_string(),
        span_id: span_id.to_string(),
    })
}

fn apply_trace_context(configure_body: &mut Value, trace_context: &TraceContextFields) {
    let Some(object) = configure_body.as_object_mut() else {
        return;
    };

    object.insert(
        "gen_ai_parent_trace_id".into(),
        json!(trace_context.trace_id.clone()),
    );
    object.insert(
        "gen_ai_parent_span_id".into(),
        json!(trace_context.span_id.clone()),
    );
}

fn list_entities(ctx: &Context, url: &str, tenant: &str) -> Result<Vec<Value>, String> {
    let resp = ctx.http_call("GET", url, &odata_headers(ctx, tenant), "")?;
    if resp.status != 200 {
        return Err(format!("GET {url} failed (HTTP {})", resp.status));
    }
    let parsed: Value = serde_json::from_str(&resp.body).unwrap_or_else(|_| json!({ "value": [] }));
    Ok(parsed
        .get("value")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default())
}

fn find_active_session(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    channel_id: &str,
    thread_id: &str,
    author_id: &str,
) -> Result<Option<Value>, String> {
    let filter = format!(
        "$filter=Status eq 'Active' and channel_id eq '{}' and thread_id eq '{}' and author_id eq '{}'",
        channel_id, thread_id, author_id
    );
    let sessions = list_entities(
        ctx,
        &format!("{temper_api_url}/tdata/ChannelSessions?{filter}"),
        tenant,
    )?;
    Ok(sessions.into_iter().next())
}

fn resume_session(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    session_id: &str,
) -> Result<(), String> {
    let url = format!("{temper_api_url}/tdata/ChannelSessions('{session_id}')/Paw.Channel.Resume");
    let _ = ctx.http_call(
        "POST",
        &url,
        &odata_headers(ctx, tenant),
        r#"{"last_message_at":"resumed"}"#,
    )?;
    Ok(())
}

fn expire_session(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    session_id: &str,
) -> Result<(), String> {
    let url = format!("{temper_api_url}/tdata/ChannelSessions('{session_id}')/Paw.Channel.Expire");
    let _ = ctx.http_call("POST", &url, &odata_headers(ctx, tenant), "{}")?;
    Ok(())
}

fn cancel_session(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    session_id: &str,
) -> Result<(), String> {
    let url = format!("{temper_api_url}/tdata/Sessions('{session_id}')/TemperPaw.Cancel");
    let _ = ctx.http_call("POST", &url, &odata_headers(ctx, tenant), "{}")?;
    Ok(())
}

fn find_route(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    channel_id: &str,
) -> Result<Option<Value>, String> {
    let routes = list_entities(ctx, &format!("{temper_api_url}/tdata/AgentRoutes"), tenant)?;
    let mut best_route: Option<(i32, Value)> = None;
    for route in routes {
        if nested_str_field(&route, &["Status", "status"]) != Some("Active") {
            continue;
        }
        let route_channel_id = nested_str_field(&route, &["ChannelId", "channel_id"]).unwrap_or("");
        if !route_channel_id.is_empty() && route_channel_id != channel_id {
            continue;
        }

        // Prefer channel-specific routes over the global fallback route.
        let score = if route_channel_id == channel_id {
            10
        } else {
            0
        };
        if best_route
            .as_ref()
            .map(|(best_score, _)| score > *best_score)
            .unwrap_or(true)
        {
            best_route = Some((score, route));
        }
    }
    Ok(best_route.map(|(_, route)| route))
}

/// Fetch the persistent Agent entity and extract its configuration.
fn fetch_agent_config(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    agent_id: &str,
) -> Result<Value, String> {
    let url = format!("{temper_api_url}/tdata/Agents('{agent_id}')");
    fetch_entity(ctx, &url, tenant)
}

/// Create a new Session for a persistent Agent.
///
/// If `route_agent_id` is non-empty, fetches the Agent entity and uses its
/// config (soul_id, model, provider, tools_enabled, max_turns).  Otherwise
/// falls back to `route_config` JSON for backward-compatible ad-hoc routing.
///
/// Returns `(agent_entity_id, session_entity_id)`.
fn create_session_for_agent(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    route_config: &str,
    route_agent_id: &str,
    user_message: &str,
    command: &str,
    trace_context: Option<&TraceContextFields>,
) -> Result<(String, String), String> {
    let config: Value = serde_json::from_str(route_config).unwrap_or_else(|_| json!({}));

    // If route points to a persistent Agent, fetch its config
    let (agent_id, agent_soul_id, agent_model, agent_provider, agent_tools, agent_max_turns) =
        if !route_agent_id.is_empty() {
            let agent = fetch_agent_config(ctx, temper_api_url, tenant, route_agent_id)?;
            let soul_id = nested_str_field(&agent, &["SoulId", "soul_id"])
                .unwrap_or("")
                .to_string();
            let model = nested_str_field(&agent, &["Model", "model"])
                .unwrap_or("claude-sonnet-4-6")
                .to_string();
            let provider = nested_str_field(&agent, &["Provider", "provider"])
                .unwrap_or("anthropic")
                .to_string();
            let tools = nested_str_field(&agent, &["ToolsEnabled", "tools_enabled"])
                .filter(|s| !s.is_empty())
                .unwrap_or(DEFAULT_TOOLS_ENABLED)
                .to_string();
            let max_turns = nested_str_field(&agent, &["MaxTurns", "max_turns"])
                .unwrap_or("200")
                .to_string();
            (
                route_agent_id.to_string(),
                soul_id,
                model,
                provider,
                tools,
                max_turns,
            )
        } else {
            // Ad-hoc routing: no persistent Agent, use route_config JSON
            let soul_id = config
                .get("soul_id")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            let model = config
                .get("model")
                .and_then(Value::as_str)
                .unwrap_or("claude-sonnet-4-6")
                .to_string();
            let provider = config
                .get("provider")
                .and_then(Value::as_str)
                .unwrap_or("anthropic")
                .to_string();
            let tools = config
                .get("tools_enabled")
                .and_then(Value::as_str)
                .unwrap_or(DEFAULT_TOOLS_ENABLED)
                .to_string();
            let max_turns = config
                .get("max_turns")
                .and_then(Value::as_str)
                .unwrap_or("200")
                .to_string();
            (String::new(), soul_id, model, provider, tools, max_turns)
        };

    // Create blank Session entity
    let session_id = create_blank_session(ctx, temper_api_url, tenant)?;

    // Determine mode-specific tools and session_mode
    let (session_mode, tools_enabled, pre_plan_tools) = if command == "plan" {
        ("plan", PLAN_MODE_TOOLS.to_string(), agent_tools.clone())
    } else {
        ("execute", agent_tools.clone(), String::new())
    };

    let mut configure_body = json!({
        "system_prompt": config.get("system_prompt").and_then(Value::as_str).unwrap_or(""),
        "user_message": user_message,
        "model": agent_model,
        "provider": agent_provider,
        "tools_enabled": tools_enabled,
        "max_turns": agent_max_turns,
        "workdir": config.get("workdir").and_then(Value::as_str).unwrap_or(DEFAULT_WORKDIR),
        "sandbox_url": config.get("sandbox_url").and_then(Value::as_str).unwrap_or(""),
        "temper_api_url": config.get("temper_api_url").and_then(Value::as_str).unwrap_or(""),
        "soul_id": agent_soul_id,
        "agent_id": agent_id,
        "parent_session_id": config.get("parent_session_id").and_then(Value::as_str).unwrap_or(""),
        "session_depth": config.get("session_depth").and_then(Value::as_str).unwrap_or("0"),
        "max_follow_ups": config.get("max_follow_ups").and_then(Value::as_str).unwrap_or("5"),
        "hook_policy": config.get("hook_policy").and_then(Value::as_str).unwrap_or("none"),
        "reserve_tokens": config.get("reserve_tokens").and_then(Value::as_str).unwrap_or("20000"),
        "keep_recent_tokens": config.get("keep_recent_tokens").and_then(Value::as_str).unwrap_or("10000"),
        "compaction_model": config.get("compaction_model").and_then(Value::as_str).unwrap_or(""),
        "heartbeat_timeout_seconds": config.get("heartbeat_timeout_seconds").and_then(Value::as_str).unwrap_or("300"),
        "project_harness_id": config.get("project_harness_id").and_then(Value::as_str).unwrap_or(""),
        "project_id": config.get("project_id").and_then(Value::as_str).unwrap_or(""),
        "session_mode": session_mode,
        "pre_plan_tools_enabled": pre_plan_tools,
    });
    if let Some(trace_context) = trace_context {
        apply_trace_context(&mut configure_body, trace_context);
    }
    let configure_url =
        format!("{temper_api_url}/tdata/Sessions('{session_id}')/TemperPaw.Configure");
    ctx.log(
        "info",
        &format!(
            "route_message: creating session {session_id} for agent {agent_id} via {configure_url}"
        ),
    );
    let configure_resp = ctx.http_call(
        "POST",
        &configure_url,
        &odata_headers(ctx, tenant),
        &configure_body.to_string(),
    )?;
    if !(200..300).contains(&configure_resp.status) {
        return Err(format!(
            "configure Session failed (HTTP {}): {}",
            configure_resp.status,
            truncate_error_body(&configure_resp.body)
        ));
    }

    // Provision is auto-scheduled by the Configure action's spec effect
    // (session.ioa.toml: effect = [{ type = "schedule", action = "Provision", delay_seconds = 0 }]).
    // No explicit Provision call needed — it would race with the scheduled one.
    Ok((agent_id, session_id))
}

/// Create a new Session when the prior Session has ended (terminal) or steer failed.
///
/// Reads the Agent ID from the ChannelSession (persistent Agent), fetches
/// the Agent entity for soul_id/config, carries forward session tree state
/// from the prior Session, and updates the ChannelSession to point to the
/// new Session via UpdateSession.
fn continue_with_new_session(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    _channel_session: &Value,
    cs_id: &str,
    agent_entity_id: &str,
    prior_session: &Value,
    _prior_session_id: &str,
    user_message: &str,
    command: &str,
    trace_context: Option<&TraceContextFields>,
) -> Result<String, String> {
    let fields = prior_session
        .get("fields")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let session_file_id = str_field(&fields, &["session_file_id", "SessionFileId"]).unwrap_or("");
    let conversation_file_id =
        str_field(&fields, &["conversation_file_id", "ConversationFileId"]).unwrap_or("");
    let prior_leaf_id = str_field(&fields, &["session_leaf_id", "SessionLeafId"]).unwrap_or("");
    let workspace_id = resolve_workspace_id_for_session(
        ctx,
        temper_api_url,
        tenant,
        &fields,
        session_file_id,
    )?;

    let new_leaf_id = if !session_file_id.is_empty() {
        Some(append_user_message_to_session(
            ctx,
            temper_api_url,
            tenant,
            &workspace_id,
            session_file_id,
            prior_leaf_id,
            user_message,
        )?)
    } else {
        None
    };

    if new_leaf_id.is_none() && !conversation_file_id.is_empty() {
        append_user_message_to_conversation(
            ctx,
            temper_api_url,
            tenant,
            &workspace_id,
            conversation_file_id,
            user_message,
        )?;
    }

    // Prepend a session-continuation notice so the agent naturally acknowledges the new session
    let user_message_with_notice = format!(
        "[System: A new session has started. Your previous conversation context and memories are preserved.]\n\n{}",
        user_message
    );

    let new_session_id = create_blank_session(ctx, temper_api_url, tenant)?;

    // Fetch Agent entity to get soul_id and config (if we have a persistent Agent)
    let (soul_id, agent_model, agent_provider, agent_tools, agent_max_turns) =
        if !agent_entity_id.is_empty() {
            match fetch_agent_config(ctx, temper_api_url, tenant, agent_entity_id) {
                Ok(agent) => {
                    let sid = nested_str_field(&agent, &["SoulId", "soul_id"])
                        .unwrap_or("")
                        .to_string();
                    let model = nested_str_field(&agent, &["Model", "model"])
                        .unwrap_or("")
                        .to_string();
                    let provider = nested_str_field(&agent, &["Provider", "provider"])
                        .unwrap_or("")
                        .to_string();
                    let tools = nested_str_field(&agent, &["ToolsEnabled", "tools_enabled"])
                        .unwrap_or("")
                        .to_string();
                    let max_turns = nested_str_field(&agent, &["MaxTurns", "max_turns"])
                        .unwrap_or("")
                        .to_string();
                    (sid, model, provider, tools, max_turns)
                }
                Err(_) => (
                    String::new(),
                    String::new(),
                    String::new(),
                    String::new(),
                    String::new(),
                ),
            }
        } else {
            (
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
            )
        };

    // Use Agent entity values when available, otherwise fall back to prior session fields
    let effective_soul_id = if !soul_id.is_empty() {
        &soul_id
    } else {
        str_field(&fields, &["soul_id", "SoulId"]).unwrap_or("")
    };
    let effective_model = if !agent_model.is_empty() {
        &agent_model
    } else {
        str_field(&fields, &["model", "Model"]).unwrap_or("claude-sonnet-4-6")
    };
    let effective_provider = if !agent_provider.is_empty() {
        &agent_provider
    } else {
        str_field(&fields, &["provider", "Provider"]).unwrap_or("anthropic")
    };
    let effective_tools = if !agent_tools.is_empty() {
        &agent_tools
    } else {
        // If prior session was in plan mode, its tools_enabled is the restricted
        // PLAN_MODE_TOOLS set. Use the stashed pre_plan_tools_enabled instead so
        // the continuation session starts with the full tool set.
        let prior_mode = str_field(&fields, &["session_mode", "SessionMode"]).unwrap_or("execute");
        if prior_mode == "plan" {
            str_field(&fields, &["pre_plan_tools_enabled", "PrePlanToolsEnabled"])
                .filter(|s| !s.is_empty())
                .unwrap_or(DEFAULT_TOOLS_ENABLED)
        } else {
            str_field(&fields, &["tools_enabled", "ToolsEnabled"]).unwrap_or(DEFAULT_TOOLS_ENABLED)
        }
    };
    let effective_max_turns = if !agent_max_turns.is_empty() {
        &agent_max_turns
    } else {
        str_field(&fields, &["max_turns", "MaxTurns"]).unwrap_or("200")
    };

    configure_session_from_prior(
        ctx,
        temper_api_url,
        tenant,
        &new_session_id,
        &fields,
        &user_message_with_notice,
        agent_entity_id,
        effective_soul_id,
        effective_model,
        effective_provider,
        effective_tools,
        effective_max_turns,
        new_leaf_id.as_deref().unwrap_or(prior_leaf_id),
        command,
        trace_context,
    )?;

    // Update ChannelSession: agent_entity_id stays the same, only session_entity_id changes
    update_session_binding(
        ctx,
        temper_api_url,
        tenant,
        cs_id,
        &new_session_id,
    )?;
    Ok(new_session_id)
}

fn create_blank_session(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
) -> Result<String, String> {
    let create_body = "{ }".to_string();
    ctx.log(
        "info",
        &format!(
            "route_message: creating session via {temper_api_url}/tdata/Sessions with {} bytes",
            create_body.len()
        ),
    );
    let create_resp = ctx.http_call(
        "POST",
        &format!("{temper_api_url}/tdata/Sessions"),
        &odata_headers(ctx, tenant),
        &create_body,
    )?;
    if !(200..300).contains(&create_resp.status) {
        return Err(format!(
            "create Session failed via {temper_api_url} (HTTP {}): {}",
            create_resp.status,
            truncate_error_body(&create_resp.body)
        ));
    }
    let parsed: Value = serde_json::from_str(&create_resp.body).unwrap_or_else(|_| json!({}));
    let session_id = parsed
        .get("entity_id")
        .or_else(|| parsed.get("Id"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    if session_id.is_empty() {
        return Err("created Session missing entity_id".to_string());
    }
    Ok(session_id)
}

fn configure_session_from_prior(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    session_id: &str,
    fields: &Value,
    user_message: &str,
    agent_entity_id: &str,
    soul_id: &str,
    model: &str,
    provider: &str,
    tools_enabled: &str,
    max_turns: &str,
    session_leaf_id: &str,
    command: &str,
    trace_context: Option<&TraceContextFields>,
) -> Result<(), String> {
    // Determine mode-specific tools and session_mode
    let (session_mode, effective_tools, pre_plan_tools) = if command == "plan" {
        ("plan", PLAN_MODE_TOOLS, tools_enabled.to_string())
    } else {
        ("execute", tools_enabled, String::new())
    };

    // Include resume-specific fields (workspace, conversation, session tree) in Configure
    // so they're stored as session fields before auto-Provision fires. The provision_sandbox
    // integration checks these to decide whether to restore an existing workspace or provision new.
    let mut configure_body = json!({
        "system_prompt": str_field(fields, &["system_prompt", "SystemPrompt"]).unwrap_or(""),
        "user_message": user_message,
        "model": model,
        "provider": provider,
        "tools_enabled": effective_tools,
        "max_turns": max_turns,
        "workdir": str_field(fields, &["workdir", "Workdir"]).unwrap_or(DEFAULT_WORKDIR),
        // Don't carry forward sandbox_url/sandbox_id from the prior session —
        // Tensorlake sandboxes expire and stale URLs cause infinite retry loops.
        // Let provision_sandbox create a fresh sandbox for each continuation.
        "sandbox_url": "",
        "sandbox_id": "",
        "temper_api_url": str_field(fields, &["temper_api_url", "TemperApiUrl"]).unwrap_or(""),
        "soul_id": soul_id,
        "agent_id": agent_entity_id,
        "parent_session_id": if !agent_entity_id.is_empty() {
            str_field(fields, &["parent_session_id", "ParentSessionId"]).unwrap_or("")
        } else {
            str_field(fields, &["parent_session_id", "ParentSessionId"]).unwrap_or("")
        },
        "session_depth": str_field(fields, &["session_depth", "SessionDepth"]).unwrap_or("0"),
        "max_follow_ups": str_field(fields, &["max_follow_ups", "MaxFollowUps"]).unwrap_or("5"),
        "hook_policy": str_field(fields, &["hook_policy", "HookPolicy"]).unwrap_or("none"),
        "reserve_tokens": str_field(fields, &["reserve_tokens", "ReserveTokens"]).unwrap_or("20000"),
        "keep_recent_tokens": str_field(fields, &["keep_recent_tokens", "KeepRecentTokens"]).unwrap_or("10000"),
        "compaction_model": str_field(fields, &["compaction_model", "CompactionModel"]).unwrap_or(""),
        "heartbeat_timeout_seconds": str_field(fields, &["heartbeat_timeout_seconds", "HeartbeatTimeoutSeconds"]).unwrap_or("300"),
        // Resume fields — folded into Configure so auto-Provision can restore prior state.
        "workspace_id": str_field(fields, &["workspace_id", "WorkspaceId"]).unwrap_or(""),
        "conversation_file_id": str_field(fields, &["conversation_file_id", "ConversationFileId"]).unwrap_or(""),
        "file_manifest_id": str_field(fields, &["file_manifest_id", "FileManifestId"]).unwrap_or(""),
        "session_file_id": str_field(fields, &["session_file_id", "SessionFileId"]).unwrap_or(""),
        "session_leaf_id": session_leaf_id,
        "project_harness_id": str_field(fields, &["project_harness_id", "ProjectHarnessId"]).unwrap_or(""),
        "project_id": str_field(fields, &["project_id", "ProjectId"]).unwrap_or(""),
        "session_mode": session_mode,
        "pre_plan_tools_enabled": pre_plan_tools,
    });
    if let Some(trace_context) = trace_context {
        apply_trace_context(&mut configure_body, trace_context);
    }
    let configure_url =
        format!("{temper_api_url}/tdata/Sessions('{session_id}')/TemperPaw.Configure");
    let configure_resp = ctx.http_call(
        "POST",
        &configure_url,
        &odata_headers(ctx, tenant),
        &configure_body.to_string(),
    )?;
    if !(200..300).contains(&configure_resp.status) {
        return Err(format!(
            "configure continuation Session failed (HTTP {}): {}",
            configure_resp.status,
            truncate_error_body(&configure_resp.body)
        ));
    }
    // Provision is auto-scheduled by Configure's spec effect. No explicit Resume needed —
    // the resume fields are already stored and provision_sandbox will restore the workspace.
    Ok(())
}

/// Update a ChannelSession to point to a new Session via the UpdateSession action.
/// The agent_entity_id stays the same (persistent Agent); only session_entity_id changes.
fn update_session_binding(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    cs_id: &str,
    new_session_id: &str,
) -> Result<(), String> {
    let url = format!(
        "{temper_api_url}/tdata/ChannelSessions('{cs_id}')/Paw.Channel.UpdateSession"
    );
    let body = json!({
        "session_entity_id": new_session_id,
        "last_message_at": "continued",
    });
    // Use the normal runtime headers so production bearer auth and the invoking
    // agent identity are forwarded consistently with the other ChannelSession actions.
    let headers = odata_headers(ctx, tenant);
    let resp = ctx.http_call("POST", &url, &headers, &body.to_string())?;
    if !(200..300).contains(&resp.status) {
        return Err(format!(
            "ChannelSession.UpdateSession failed (HTTP {})",
            resp.status
        ));
    }
    Ok(())
}

fn append_user_message_to_session(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    workspace_id: &str,
    session_file_id: &str,
    session_leaf_id: &str,
    user_message: &str,
) -> Result<String, String> {
    let session_jsonl = read_file_value(ctx, temper_api_url, tenant, workspace_id, session_file_id)?;
    let mut tree = SessionTree::from_jsonl(&session_jsonl);
    let mut parent_id = if !session_leaf_id.is_empty() {
        session_leaf_id.to_string()
    } else {
        tree.last_entry_id()
            .map(|value| value.to_string())
            .ok_or("session tree is empty")?
    };
    if let Some(interrupted_results) = tree.interrupted_tool_results_for_leaf(&parent_id) {
        let note = "Tool execution was interrupted because the previous agent run ended before returning results.";
        let tokens = estimate_tokens(note);
        let (tool_result_id, _) =
            tree.append_tool_results(&parent_id, &interrupted_results, tokens);
        parent_id = tool_result_id;
    }
    let tokens = estimate_tokens(user_message);
    let (new_leaf_id, _) = if !workspace_id.is_empty() {
        let file_name = format!("session-user-{}.txt", tree.len());
        match create_content_file(
            ctx,
            temper_api_url,
            tenant,
            workspace_id,
            &file_name,
            user_message,
        ) {
            Ok(content_file_id) => tree.append_user_message_file(&parent_id, &content_file_id, tokens),
            Err(err) => {
                ctx.log(
                    "warn",
                    &format!(
                        "route_message: failed to externalize continued user message to TemperFS: {err}"
                    ),
                );
                tree.append_user_message(&parent_id, user_message, tokens)
            }
        }
    } else {
        tree.append_user_message(&parent_id, user_message, tokens)
    };
    write_file_value(
        ctx,
        temper_api_url,
        tenant,
        workspace_id,
        session_file_id,
        &tree.to_jsonl(),
    )?;
    Ok(new_leaf_id)
}

fn append_user_message_to_conversation(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    workspace_id: &str,
    conversation_file_id: &str,
    user_message: &str,
) -> Result<(), String> {
    let raw = read_file_value(ctx, temper_api_url, tenant, workspace_id, conversation_file_id)?;
    let parsed: Value = serde_json::from_str(&raw).unwrap_or_else(|_| json!({ "messages": [] }));
    let mut messages = parsed
        .get("messages")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    messages.push(json!({ "role": "user", "content": user_message }));
    let updated = json!({ "messages": messages }).to_string();
    write_file_value(
        ctx,
        temper_api_url,
        tenant,
        workspace_id,
        conversation_file_id,
        &updated,
    )
}

fn resolve_workspace_id_for_session(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    fields: &Value,
    session_file_id: &str,
) -> Result<String, String> {
    if let Some(workspace_id) = str_field(fields, &["workspace_id", "WorkspaceId"]) {
        if !workspace_id.is_empty() {
            return Ok(workspace_id.to_string());
        }
    }
    if session_file_id.is_empty() {
        return Ok(String::new());
    }
    let session_file = fetch_entity(
        ctx,
        &format!("{temper_api_url}/tdata/Files('{session_file_id}')"),
        tenant,
    )?;
    Ok(nested_str_field(&session_file, &["workspace_id", "WorkspaceId"])
        .unwrap_or("")
        .to_string())
}

fn read_file_value(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    workspace_id: &str,
    file_id: &str,
) -> Result<String, String> {
    let url = format!("{temper_api_url}/tdata/Files('{file_id}')/$value");
    let headers = file_headers(ctx, tenant, workspace_id, None, None);
    let resp = ctx.http_call("GET", &url, &headers, "")?;
    if resp.status == 200 {
        Ok(resp.body)
    } else {
        Err(format!("GET {url} failed (HTTP {})", resp.status))
    }
}

fn write_file_value(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    workspace_id: &str,
    file_id: &str,
    body: &str,
) -> Result<(), String> {
    let url = format!("{temper_api_url}/tdata/Files('{file_id}')/$value");
    let headers = file_headers(ctx, tenant, workspace_id, Some("text/plain"), None);
    let resp = ctx.http_call("PUT", &url, &headers, body)?;
    if (200..300).contains(&resp.status) {
        Ok(())
    } else {
        Err(format!("PUT {url} failed (HTTP {})", resp.status))
    }
}

fn fetch_entity(ctx: &Context, url: &str, tenant: &str) -> Result<Value, String> {
    let resp = ctx.http_call("GET", url, &odata_headers(ctx, tenant), "")?;
    if resp.status != 200 {
        return Err(format!("GET {url} failed (HTTP {})", resp.status));
    }
    serde_json::from_str(&resp.body).map_err(|e| format!("failed to parse entity JSON: {e}"))
}

fn truncate_error_body(body: &str) -> String {
    const LIMIT: usize = 240;
    if body.len() <= LIMIT {
        body.to_string()
    } else {
        format!("{}...", &body[..LIMIT])
    }
}

fn session_entity_url(temper_api_url: &str, session_id: &str) -> String {
    format!("{temper_api_url}/tdata/Sessions('{session_id}')")
}

fn is_terminal_status(status: &str) -> bool {
    matches!(status, "Completed" | "Failed" | "Cancelled")
}

fn is_steerable_status(status: &str) -> bool {
    matches!(status, "Thinking" | "Executing" | "Steering" | "Compacting")
}

fn estimate_tokens(text: &str) -> usize {
    text.len() / 4
}

fn create_channel_session(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    channel_id: &str,
    thread_id: &str,
    author_id: &str,
    agent_entity_id: &str,
    session_entity_id: &str,
) -> Result<(), String> {
    let create_resp = ctx.http_call(
        "POST",
        &format!("{temper_api_url}/tdata/ChannelSessions"),
        &odata_headers(ctx, tenant),
        "{}",
    )?;
    if !(200..300).contains(&create_resp.status) {
        return Err(format!(
            "create ChannelSession failed (HTTP {})",
            create_resp.status
        ));
    }
    let parsed: Value = serde_json::from_str(&create_resp.body).unwrap_or_else(|_| json!({}));
    let cs_id = parsed
        .get("entity_id")
        .or_else(|| parsed.get("Id"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    if cs_id.is_empty() {
        return Err("ChannelSession creation missing entity_id".to_string());
    }
    let create_url =
        format!("{temper_api_url}/tdata/ChannelSessions('{cs_id}')/Paw.Channel.Create");
    let body = json!({
        "channel_id": channel_id,
        "thread_id": thread_id,
        "author_id": author_id,
        "agent_entity_id": agent_entity_id,
        "session_entity_id": session_entity_id,
        "last_message_at": "created",
    });
    let resp = ctx.http_call(
        "POST",
        &create_url,
        &odata_headers(ctx, tenant),
        &body.to_string(),
    )?;
    if !(200..300).contains(&resp.status) {
        return Err(format!(
            "ChannelSession.Create failed (HTTP {})",
            resp.status
        ));
    }
    Ok(())
}

fn switch_mode_and_steer(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    session_id: &str,
    command: &str,
    content: &str,
) -> Result<(), String> {
    // 1. Fetch the Session entity to read current tools_enabled
    let session_url = format!("{temper_api_url}/tdata/Sessions('{session_id}')");
    let session_resp = ctx.http_call("GET", &session_url, &odata_headers(ctx, tenant), "")?;
    let session: Value = if session_resp.status == 200 {
        serde_json::from_str(&session_resp.body).unwrap_or_else(|_| json!({}))
    } else {
        json!({})
    };

    let current_tools =
        nested_str_field(&session, &["ToolsEnabled", "tools_enabled"]).unwrap_or(DEFAULT_TOOLS_ENABLED);

    // 2. Build SwitchMode body
    let mut body = serde_json::Map::new();
    body.insert("session_mode".into(), json!(command));

    if command == "plan" {
        body.insert("pre_plan_tools_enabled".into(), json!(current_tools));
        body.insert("tools_enabled".into(), json!(PLAN_MODE_TOOLS));
    } else {
        // "execute" — restore stashed tools
        let stashed = nested_str_field(&session, &["PrePlanToolsEnabled", "pre_plan_tools_enabled"])
            .filter(|s| !s.is_empty())
            .unwrap_or(DEFAULT_TOOLS_ENABLED);
        body.insert("tools_enabled".into(), json!(stashed));
        body.insert("pre_plan_tools_enabled".into(), json!(""));
    }

    // 3. Dispatch SwitchMode
    let switch_url = format!("{temper_api_url}/tdata/Sessions('{session_id}')/TemperPaw.SwitchMode");
    let resp = ctx.http_call(
        "POST",
        &switch_url,
        &odata_headers(ctx, tenant),
        &json!(body).to_string(),
    )?;
    if !(200..300).contains(&resp.status) {
        return Err(format!(
            "SwitchMode failed (HTTP {}): {}",
            resp.status,
            truncate_error_body(&resp.body)
        ));
    }

    // 4. Steer with the task text (if any)
    if !content.is_empty() {
        steer_session(ctx, temper_api_url, tenant, session_id, content)?;
    }

    Ok(())
}

fn steer_session(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    session_id: &str,
    message: &str,
) -> Result<(), String> {
    let session_url = format!("{temper_api_url}/tdata/Sessions('{session_id}')");
    let session_resp = ctx.http_call("GET", &session_url, &odata_headers(ctx, tenant), "")?;
    let mut queue = if session_resp.status == 200 {
        let parsed: Value = serde_json::from_str(&session_resp.body).unwrap_or_else(|_| json!({}));
        serde_json::from_str::<Vec<Value>>(
            nested_str_field(&parsed, &["SteeringMessages", "steering_messages"]).unwrap_or("[]"),
        )
        .unwrap_or_default()
    } else {
        Vec::new()
    };
    queue.push(json!({ "content": message }));
    let steer_url = format!("{temper_api_url}/tdata/Sessions('{session_id}')/TemperPaw.Steer");
    let body = json!({
        "steering_messages": serde_json::to_string(&queue).unwrap_or_else(|_| "[]".to_string()),
    });
    let resp = ctx.http_call(
        "POST",
        &steer_url,
        &odata_headers(ctx, tenant),
        &body.to_string(),
    )?;
    if !(200..300).contains(&resp.status) {
        return Err(format!("steer session failed (HTTP {})", resp.status));
    }
    Ok(())
}

fn str_field<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
}

fn nested_str_field<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a str> {
    str_field(value, keys).or_else(|| {
        value
            .get("fields")
            .and_then(|fields| str_field(fields, keys))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trace_context_is_read_from_receive_message_trigger_params() {
        let trace = trace_context_from_trigger_params(&json!({
            "gen_ai_parent_trace_id": "4bf92f3577b34da6a3ce929d0e0e4736",
            "gen_ai_parent_span_id": "00f067aa0ba902b7",
        }))
        .expect("trace context should be present");

        assert_eq!(trace.trace_id, "4bf92f3577b34da6a3ce929d0e0e4736");
        assert_eq!(trace.span_id, "00f067aa0ba902b7");
    }

    #[test]
    fn configure_body_carries_trace_context_when_available() {
        let trace = TraceContextFields {
            trace_id: "4bf92f3577b34da6a3ce929d0e0e4736".to_string(),
            span_id: "00f067aa0ba902b7".to_string(),
        };
        let mut configure_body = json!({
            "user_message": "hello",
            "session_mode": "execute",
        });

        apply_trace_context(&mut configure_body, trace.as_ref());

        assert_eq!(
            configure_body
                .get("gen_ai_parent_trace_id")
                .and_then(Value::as_str),
            Some("4bf92f3577b34da6a3ce929d0e0e4736")
        );
        assert_eq!(
            configure_body
                .get("gen_ai_parent_span_id")
                .and_then(Value::as_str),
            Some("00f067aa0ba902b7")
        );
    }
}
