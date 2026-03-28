use session_tree_lib::SessionTree;
use temper_wasm_sdk::prelude::*;
use wasm_helpers::create_content_file;

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
        if channel_id.is_empty() || thread_id.is_empty() || author_id.is_empty() {
            return Err("route_message: missing channel_id/thread_id/author_id".to_string());
        }

        let existing_session = find_active_session(
            &ctx,
            &temper_api_url,
            &ctx.tenant,
            channel_id,
            thread_id,
            author_id,
        )?;
        let agent_id = if let Some(session) = existing_session {
            let session_id = session
                .get("entity_id")
                .and_then(|v| v.as_str())
                .or_else(|| nested_str_field(&session, &["Id", "entity_id"]))
                .unwrap_or_default()
                .to_string();
            let agent_id = nested_str_field(&session, &["AgentEntityId", "agent_entity_id"])
                .unwrap_or_default()
                .to_string();

            if !agent_id.is_empty() {
                let agent = fetch_entity(
                    &ctx,
                    &agent_entity_url(&temper_api_url, &agent_id),
                    &ctx.tenant,
                )?;
                let agent_status = nested_str_field(&agent, &["Status", "status"]).unwrap_or("");

                if is_steerable_status(agent_status) {
                    resume_session(&ctx, &temper_api_url, &ctx.tenant, &session_id).ok();
                    if steer_existing_agent(&ctx, &temper_api_url, &ctx.tenant, &agent_id, content)
                        .is_ok()
                    {
                        agent_id
                    } else {
                        continue_session(
                            &ctx,
                            &temper_api_url,
                            &ctx.tenant,
                            &session,
                            &session_id,
                            &agent,
                            &agent_id,
                            content,
                        )?
                    }
                } else if is_terminal_status(agent_status) {
                    continue_session(
                        &ctx,
                        &temper_api_url,
                        &ctx.tenant,
                        &session,
                        &session_id,
                        &agent,
                        &agent_id,
                        content,
                    )?
                } else {
                    expire_session(&ctx, &temper_api_url, &ctx.tenant, &session_id).ok();
                    let route = find_route(&ctx, &temper_api_url, &ctx.tenant, channel_id)?;
                    let route_config = route
                        .as_ref()
                        .and_then(|value| nested_str_field(value, &["AgentConfig", "agent_config"]))
                        .filter(|value| !value.trim().is_empty())
                        .unwrap_or(default_agent_config);
                    let route_soul_id = route
                        .as_ref()
                        .and_then(|value| nested_str_field(value, &["SoulId", "soul_id"]))
                        .unwrap_or("");
                    let new_agent_id = create_agent_from_route(
                        &ctx,
                        &temper_api_url,
                        &ctx.tenant,
                        route_config,
                        route_soul_id,
                        content,
                    )?;
                    create_session(
                        &ctx,
                        &temper_api_url,
                        &ctx.tenant,
                        channel_id,
                        thread_id,
                        author_id,
                        &new_agent_id,
                    )?;
                    new_agent_id
                }
            } else {
                let route = find_route(&ctx, &temper_api_url, &ctx.tenant, channel_id)?;
                let route_config = route
                    .as_ref()
                    .and_then(|value| nested_str_field(value, &["AgentConfig", "agent_config"]))
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or(default_agent_config);
                let route_soul_id = route
                    .as_ref()
                    .and_then(|value| nested_str_field(value, &["SoulId", "soul_id"]))
                    .unwrap_or("");
                expire_session(&ctx, &temper_api_url, &ctx.tenant, &session_id).ok();
                let new_agent_id = create_agent_from_route(
                    &ctx,
                    &temper_api_url,
                    &ctx.tenant,
                    route_config,
                    route_soul_id,
                    content,
                )?;
                create_session(
                    &ctx,
                    &temper_api_url,
                    &ctx.tenant,
                    channel_id,
                    thread_id,
                    author_id,
                    &new_agent_id,
                )?;
                new_agent_id
            }
        } else {
            let route = find_route(&ctx, &temper_api_url, &ctx.tenant, channel_id)?;
            let route_config = route
                .as_ref()
                .and_then(|value| nested_str_field(value, &["AgentConfig", "agent_config"]))
                .filter(|value| !value.trim().is_empty())
                .unwrap_or(default_agent_config);
            let route_soul_id = route
                .as_ref()
                .and_then(|value| nested_str_field(value, &["SoulId", "soul_id"]))
                .unwrap_or("");
            let agent_id = create_agent_from_route(
                &ctx,
                &temper_api_url,
                &ctx.tenant,
                route_config,
                route_soul_id,
                content,
            )?;
            create_session(
                &ctx,
                &temper_api_url,
                &ctx.tenant,
                channel_id,
                thread_id,
                author_id,
                &agent_id,
            )?;
            agent_id
        };

        let result_text = wait_for_agent(&ctx, &temper_api_url, &ctx.tenant, &agent_id)?;
        set_success_result(
            "SendReply",
            &json!({
                "thread_id": thread_id,
                "content": result_text,
                "agent_entity_id": agent_id,
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

fn odata_headers(tenant: &str) -> Vec<(String, String)> {
    vec![
        ("x-tenant-id".to_string(), tenant.to_string()),
        ("x-temper-principal-kind".to_string(), "admin".to_string()),
        ("content-type".to_string(), "application/json".to_string()),
        ("accept".to_string(), "application/json".to_string()),
    ]
}

fn list_entities(ctx: &Context, url: &str, tenant: &str) -> Result<Vec<Value>, String> {
    let resp = ctx.http_call("GET", url, &odata_headers(tenant), "")?;
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
        &odata_headers(tenant),
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
    let _ = ctx.http_call("POST", &url, &odata_headers(tenant), "{}")?;
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

fn create_agent_from_route(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    route_config: &str,
    route_soul_id: &str,
    user_message: &str,
) -> Result<String, String> {
    let config: Value = serde_json::from_str(route_config).unwrap_or_else(|_| json!({}));
    let raw_soul_ref = if route_soul_id.is_empty() {
        config.get("soul_id").and_then(Value::as_str).unwrap_or("")
    } else {
        route_soul_id
    };
    let normalized_soul_ref = normalize_soul_ref(ctx, temper_api_url, tenant, raw_soul_ref)
        .unwrap_or_else(|| raw_soul_ref.to_string());
    let create_body = "{ }".to_string();
    ctx.log(
        "info",
        &format!(
            "route_message: creating routed agent via {temper_api_url}/tdata/Agents with {} bytes",
            create_body.len()
        ),
    );
    let create_resp = ctx.http_call(
        "POST",
        &format!("{temper_api_url}/tdata/Agents"),
        &odata_headers(tenant),
        &create_body,
    )?;
    if !(200..300).contains(&create_resp.status) {
        return Err(format!(
            "create Agent failed via {temper_api_url} (HTTP {}): {}",
            create_resp.status,
            truncate_error_body(&create_resp.body)
        ));
    }
    let parsed: Value = serde_json::from_str(&create_resp.body).unwrap_or_else(|_| json!({}));
    let agent_id = parsed
        .get("entity_id")
        .or_else(|| parsed.get("Id"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    if agent_id.is_empty() {
        return Err("route_message: created Agent missing entity_id".to_string());
    }

    let configure_body = json!({
        "system_prompt": config.get("system_prompt").and_then(Value::as_str).unwrap_or(""),
        "user_message": user_message,
        "model": config.get("model").and_then(Value::as_str).unwrap_or("claude-sonnet-4-20250514"),
        "provider": config.get("provider").and_then(Value::as_str).unwrap_or("anthropic"),
        "tools_enabled": config.get("tools_enabled").and_then(Value::as_str).unwrap_or("read_entity"),
        "max_turns": config.get("max_turns").and_then(Value::as_str).unwrap_or("6"),
        "workdir": config.get("workdir").and_then(Value::as_str).unwrap_or("/tmp/workspace"),
        "sandbox_url": config.get("sandbox_url").and_then(Value::as_str).unwrap_or(""),
        "temper_api_url": config.get("temper_api_url").and_then(Value::as_str).unwrap_or(""),
        "soul_id": normalized_soul_ref,
        "parent_agent_id": config.get("parent_agent_id").and_then(Value::as_str).unwrap_or(""),
        "agent_depth": config.get("agent_depth").and_then(Value::as_str).unwrap_or("0"),
        "max_follow_ups": config.get("max_follow_ups").and_then(Value::as_str).unwrap_or("5"),
        "hook_policy": config.get("hook_policy").and_then(Value::as_str).unwrap_or("none"),
        "reserve_tokens": config.get("reserve_tokens").and_then(Value::as_str).unwrap_or("20000"),
        "keep_recent_tokens": config.get("keep_recent_tokens").and_then(Value::as_str).unwrap_or("10000"),
        "compaction_model": config.get("compaction_model").and_then(Value::as_str).unwrap_or(""),
        "heartbeat_timeout_seconds": config.get("heartbeat_timeout_seconds").and_then(Value::as_str).unwrap_or("300"),
    });
    let configure_url = format!("{temper_api_url}/tdata/Agents('{agent_id}')/OpenPaw.Configure");
    let configure_resp = ctx.http_call(
        "POST",
        &configure_url,
        &odata_headers(tenant),
        &configure_body.to_string(),
    )?;
    if !(200..300).contains(&configure_resp.status) {
        return Err(format!(
            "configure Agent failed (HTTP {})",
            configure_resp.status
        ));
    }

    let provision_url = format!("{temper_api_url}/tdata/Agents('{agent_id}')/OpenPaw.Provision");
    let provision_resp = ctx.http_call("POST", &provision_url, &odata_headers(tenant), "{}")?;
    if !(200..300).contains(&provision_resp.status) {
        return Err(format!(
            "provision Agent failed (HTTP {})",
            provision_resp.status
        ));
    }
    Ok(agent_id)
}

fn continue_session(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    session: &Value,
    session_id: &str,
    prior_agent: &Value,
    prior_agent_id: &str,
    user_message: &str,
) -> Result<String, String> {
    let fields = prior_agent
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
            conversation_file_id,
            user_message,
        )?;
    }

    let new_agent_id = create_blank_agent(ctx, temper_api_url, tenant)?;
    let channel_id = nested_str_field(session, &["ChannelId", "channel_id"]).unwrap_or("");
    let route_soul_fallback = if channel_id.is_empty() {
        String::new()
    } else {
        find_route(ctx, temper_api_url, tenant, channel_id)?
            .as_ref()
            .and_then(|route| nested_str_field(route, &["SoulId", "soul_id"]))
            .map(ToString::to_string)
            .unwrap_or_default()
    };
    configure_agent_from_prior(
        ctx,
        temper_api_url,
        tenant,
        &new_agent_id,
        &fields,
        user_message,
        prior_agent_id,
        &route_soul_fallback,
    )?;
    resume_agent_from_prior(
        ctx,
        temper_api_url,
        tenant,
        &new_agent_id,
        &fields,
        new_leaf_id.as_deref().unwrap_or(prior_leaf_id),
    )?;
    update_session_agent_binding(
        ctx,
        temper_api_url,
        tenant,
        session_id,
        session,
        &new_agent_id,
    )?;
    Ok(new_agent_id)
}

fn create_blank_agent(ctx: &Context, temper_api_url: &str, tenant: &str) -> Result<String, String> {
    let create_body = "{ }".to_string();
    ctx.log(
        "info",
        &format!(
            "route_message: creating continuation agent via {temper_api_url}/tdata/Agents with {} bytes",
            create_body.len()
        ),
    );
    let create_resp = ctx.http_call(
        "POST",
        &format!("{temper_api_url}/tdata/Agents"),
        &odata_headers(tenant),
        &create_body,
    )?;
    if !(200..300).contains(&create_resp.status) {
        return Err(format!(
            "create Agent failed via {temper_api_url} (HTTP {}): {}",
            create_resp.status,
            truncate_error_body(&create_resp.body)
        ));
    }
    let parsed: Value = serde_json::from_str(&create_resp.body).unwrap_or_else(|_| json!({}));
    let agent_id = parsed
        .get("entity_id")
        .or_else(|| parsed.get("Id"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    if agent_id.is_empty() {
        return Err("created Agent missing entity_id".to_string());
    }
    Ok(agent_id)
}

fn configure_agent_from_prior(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    agent_id: &str,
    fields: &Value,
    user_message: &str,
    prior_agent_id: &str,
    fallback_soul_ref: &str,
) -> Result<(), String> {
    let prior_soul_ref = str_field(fields, &["soul_id", "SoulId"]).unwrap_or("");
    let soul_ref = normalize_soul_ref(ctx, temper_api_url, tenant, prior_soul_ref)
        .or_else(|| normalize_soul_ref(ctx, temper_api_url, tenant, fallback_soul_ref))
        .unwrap_or_else(|| {
            if !prior_soul_ref.is_empty() {
                prior_soul_ref.to_string()
            } else {
                fallback_soul_ref.to_string()
            }
        });
    let configure_body = json!({
        "system_prompt": str_field(fields, &["system_prompt", "SystemPrompt"]).unwrap_or(""),
        "user_message": user_message,
        "model": str_field(fields, &["model", "Model"]).unwrap_or("claude-sonnet-4-20250514"),
        "provider": str_field(fields, &["provider", "Provider"]).unwrap_or("anthropic"),
        "max_turns": str_field(fields, &["max_turns", "MaxTurns"]).unwrap_or("20"),
        "tools_enabled": str_field(fields, &["tools_enabled", "ToolsEnabled"]).unwrap_or("read,write,edit,bash"),
        "workdir": str_field(fields, &["workdir", "Workdir"]).unwrap_or("/workspace"),
        "sandbox_url": str_field(fields, &["sandbox_url", "SandboxUrl"]).unwrap_or(""),
        "temper_api_url": str_field(fields, &["temper_api_url", "TemperApiUrl"]).unwrap_or(""),
        "soul_id": soul_ref,
        "parent_agent_id": if prior_agent_id.is_empty() {
            str_field(fields, &["parent_agent_id", "ParentAgentId"]).unwrap_or("")
        } else {
            prior_agent_id
        },
        "agent_depth": str_field(fields, &["agent_depth", "AgentDepth"]).unwrap_or("0"),
        "max_follow_ups": str_field(fields, &["max_follow_ups", "MaxFollowUps"]).unwrap_or("5"),
        "hook_policy": str_field(fields, &["hook_policy", "HookPolicy"]).unwrap_or("none"),
        "reserve_tokens": str_field(fields, &["reserve_tokens", "ReserveTokens"]).unwrap_or("20000"),
        "keep_recent_tokens": str_field(fields, &["keep_recent_tokens", "KeepRecentTokens"]).unwrap_or("10000"),
        "compaction_model": str_field(fields, &["compaction_model", "CompactionModel"]).unwrap_or(""),
        "heartbeat_timeout_seconds": str_field(fields, &["heartbeat_timeout_seconds", "HeartbeatTimeoutSeconds"]).unwrap_or("300"),
    });
    let configure_url = format!("{temper_api_url}/tdata/Agents('{agent_id}')/OpenPaw.Configure");
    let configure_resp = ctx.http_call(
        "POST",
        &configure_url,
        &odata_headers(tenant),
        &configure_body.to_string(),
    )?;
    if !(200..300).contains(&configure_resp.status) {
        return Err(format!(
            "configure continuation Agent failed (HTTP {})",
            configure_resp.status
        ));
    }
    Ok(())
}

fn resume_agent_from_prior(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    agent_id: &str,
    fields: &Value,
    session_leaf_id: &str,
) -> Result<(), String> {
    let resume_body = json!({
        "sandbox_url": str_field(fields, &["sandbox_url", "SandboxUrl"]).unwrap_or(""),
        "sandbox_id": str_field(fields, &["sandbox_id", "SandboxId"]).unwrap_or(""),
        "workspace_id": str_field(fields, &["workspace_id", "WorkspaceId"]).unwrap_or(""),
        "conversation_file_id": str_field(fields, &["conversation_file_id", "ConversationFileId"]).unwrap_or(""),
        "file_manifest_id": str_field(fields, &["file_manifest_id", "FileManifestId"]).unwrap_or(""),
        "session_file_id": str_field(fields, &["session_file_id", "SessionFileId"]).unwrap_or(""),
        "session_leaf_id": session_leaf_id,
    });
    let resume_url = format!("{temper_api_url}/tdata/Agents('{agent_id}')/OpenPaw.Resume");
    let resume_resp = ctx.http_call(
        "POST",
        &resume_url,
        &odata_headers(tenant),
        &resume_body.to_string(),
    )?;
    if !(200..300).contains(&resume_resp.status) {
        return Err(format!(
            "resume continuation Agent failed (HTTP {})",
            resume_resp.status
        ));
    }
    Ok(())
}

fn update_session_agent_binding(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    session_id: &str,
    session: &Value,
    agent_id: &str,
) -> Result<(), String> {
    let url = format!("{temper_api_url}/tdata/ChannelSessions('{session_id}')/Paw.Channel.Create");
    let body = json!({
        "channel_id": nested_str_field(session, &["ChannelId", "channel_id"]).unwrap_or(""),
        "thread_id": nested_str_field(session, &["ThreadId", "thread_id"]).unwrap_or(""),
        "author_id": nested_str_field(session, &["AuthorId", "author_id"]).unwrap_or(""),
        "agent_entity_id": agent_id,
        "last_message_at": "continued",
    });
    let resp = ctx.http_call("POST", &url, &odata_headers(tenant), &body.to_string())?;
    if !(200..300).contains(&resp.status) {
        return Err(format!(
            "ChannelSession.Create continuation update failed (HTTP {})",
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
    let session_jsonl = read_file_value(ctx, temper_api_url, tenant, session_file_id)?;
    let mut tree = SessionTree::from_jsonl(&session_jsonl);
    let mut parent_id = if !session_leaf_id.is_empty() {
        session_leaf_id.to_string()
    } else {
        tree.last_entry_id()
            .map(|value| value.to_string())
            .ok_or("session tree is empty")?
    };
    if let Some(interrupted_results) = interrupted_tool_results_for_leaf(&tree, &parent_id) {
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
        session_file_id,
        &tree.to_jsonl(),
    )?;
    Ok(new_leaf_id)
}

fn append_user_message_to_conversation(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    conversation_file_id: &str,
    user_message: &str,
) -> Result<(), String> {
    let raw = read_file_value(ctx, temper_api_url, tenant, conversation_file_id)?;
    let parsed: Value = serde_json::from_str(&raw).unwrap_or_else(|_| json!({ "messages": [] }));
    let mut messages = parsed
        .get("messages")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    messages.push(json!({ "role": "user", "content": user_message }));
    let updated = json!({ "messages": messages }).to_string();
    write_file_value(ctx, temper_api_url, tenant, conversation_file_id, &updated)
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
    file_id: &str,
) -> Result<String, String> {
    let url = format!("{temper_api_url}/tdata/Files('{file_id}')/$value");
    let headers = vec![
        ("x-tenant-id".to_string(), tenant.to_string()),
        ("x-temper-principal-kind".to_string(), "admin".to_string()),
    ];
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
    file_id: &str,
    body: &str,
) -> Result<(), String> {
    let url = format!("{temper_api_url}/tdata/Files('{file_id}')/$value");
    let headers = vec![
        ("x-tenant-id".to_string(), tenant.to_string()),
        ("x-temper-principal-kind".to_string(), "admin".to_string()),
        ("content-type".to_string(), "text/plain".to_string()),
    ];
    let resp = ctx.http_call("PUT", &url, &headers, body)?;
    if (200..300).contains(&resp.status) {
        Ok(())
    } else {
        Err(format!("PUT {url} failed (HTTP {})", resp.status))
    }
}

fn fetch_entity(ctx: &Context, url: &str, tenant: &str) -> Result<Value, String> {
    let resp = ctx.http_call("GET", url, &odata_headers(tenant), "")?;
    if resp.status != 200 {
        return Err(format!("GET {url} failed (HTTP {})", resp.status));
    }
    serde_json::from_str(&resp.body).map_err(|e| format!("failed to parse entity JSON: {e}"))
}

fn normalize_soul_ref(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    soul_ref: &str,
) -> Option<String> {
    if soul_ref.is_empty() {
        return None;
    }

    let by_id_url = format!("{temper_api_url}/tdata/Souls('{soul_ref}')");
    if let Ok(entity) = fetch_entity(ctx, &by_id_url, tenant) {
        return nested_str_field(&entity, &["Name", "name"]).map(ToString::to_string);
    }

    let escaped = soul_ref.replace('\'', "''");
    let by_name_url =
        format!("{temper_api_url}/tdata/Souls?$filter=Name eq '{escaped}' and Status eq 'Active'");
    if let Ok(list) = list_entities(ctx, &by_name_url, tenant) {
        if let Some(entity) = list.into_iter().next() {
            return nested_str_field(&entity, &["Name", "name"])
                .or_else(|| nested_str_field(&entity, &["Id", "entity_id"]))
                .map(ToString::to_string);
        }
    }

    None
}

fn truncate_error_body(body: &str) -> String {
    const LIMIT: usize = 240;
    if body.len() <= LIMIT {
        body.to_string()
    } else {
        format!("{}...", &body[..LIMIT])
    }
}

fn interrupted_tool_results_for_leaf(tree: &SessionTree, leaf_id: &str) -> Option<Value> {
    let entry = tree.get(leaf_id)?;
    let role = entry.data.get("role").and_then(Value::as_str).unwrap_or("");
    if role != "assistant" {
        return None;
    }

    let blocks = entry.data.get("content").and_then(Value::as_array)?;
    let mut results = Vec::new();
    for block in blocks {
        if block.get("type").and_then(Value::as_str) != Some("tool_use") {
            continue;
        }
        let Some(tool_use_id) = block.get("id").and_then(Value::as_str) else {
            continue;
        };
        results.push(json!({
            "type": "tool_result",
            "tool_use_id": tool_use_id,
            "content": "Tool execution was interrupted because the previous agent run ended before returning results.",
            "is_error": true,
        }));
    }

    if results.is_empty() {
        None
    } else {
        Some(Value::Array(results))
    }
}

fn agent_entity_url(temper_api_url: &str, agent_id: &str) -> String {
    format!("{temper_api_url}/tdata/Agents('{agent_id}')")
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

fn create_session(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    channel_id: &str,
    thread_id: &str,
    author_id: &str,
    agent_id: &str,
) -> Result<(), String> {
    let create_resp = ctx.http_call(
        "POST",
        &format!("{temper_api_url}/tdata/ChannelSessions"),
        &odata_headers(tenant),
        "{}",
    )?;
    if !(200..300).contains(&create_resp.status) {
        return Err(format!(
            "create ChannelSession failed (HTTP {})",
            create_resp.status
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
        return Err("ChannelSession creation missing entity_id".to_string());
    }
    let create_url =
        format!("{temper_api_url}/tdata/ChannelSessions('{session_id}')/Paw.Channel.Create");
    let body = json!({
        "channel_id": channel_id,
        "thread_id": thread_id,
        "author_id": author_id,
        "agent_entity_id": agent_id,
        "last_message_at": "created",
    });
    let resp = ctx.http_call(
        "POST",
        &create_url,
        &odata_headers(tenant),
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

fn steer_existing_agent(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    agent_id: &str,
    message: &str,
) -> Result<(), String> {
    let agent_url = format!("{temper_api_url}/tdata/Agents('{agent_id}')");
    let agent_resp = ctx.http_call("GET", &agent_url, &odata_headers(tenant), "")?;
    let mut queue = if agent_resp.status == 200 {
        let parsed: Value = serde_json::from_str(&agent_resp.body).unwrap_or_else(|_| json!({}));
        serde_json::from_str::<Vec<Value>>(
            nested_str_field(&parsed, &["SteeringMessages", "steering_messages"]).unwrap_or("[]"),
        )
        .unwrap_or_default()
    } else {
        Vec::new()
    };
    queue.push(json!({ "content": message }));
    let steer_url = format!("{temper_api_url}/tdata/Agents('{agent_id}')/OpenPaw.Steer");
    let body = json!({
        "steering_messages": serde_json::to_string(&queue).unwrap_or_else(|_| "[]".to_string()),
    });
    let resp = ctx.http_call(
        "POST",
        &steer_url,
        &odata_headers(tenant),
        &body.to_string(),
    )?;
    if !(200..300).contains(&resp.status) {
        return Err(format!("steer agent failed (HTTP {})", resp.status));
    }
    Ok(())
}

fn wait_for_agent(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    agent_id: &str,
) -> Result<String, String> {
    let wait_url = format!(
        "{temper_api_url}/observe/entities/Agent/{agent_id}/wait?statuses=Completed,Failed,Cancelled&timeout_ms=300000&poll_ms=250"
    );
    let wait_headers = vec![
        ("x-tenant-id".to_string(), tenant.to_string()),
        ("x-temper-principal-kind".to_string(), "admin".to_string()),
        ("accept".to_string(), "application/json".to_string()),
    ];
    let wait_resp = ctx.http_call("GET", &wait_url, &wait_headers, "")?;
    if wait_resp.status == 200 {
        let agent: Value = serde_json::from_str(&wait_resp.body)
            .map_err(|e| format!("failed to parse observe wait response: {e}"))?;
        let status = nested_str_field(&agent, &["Status", "status"]).unwrap_or("");
        let timed_out = agent
            .get("timed_out")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let result = agent
            .get("fields")
            .and_then(|v| v.get("result"))
            .or_else(|| agent.get("fields").and_then(|v| v.get("Result")))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        if status == "Completed" {
            return Ok(result);
        }
        if timed_out && !matches!(status, "Failed" | "Cancelled") {
            return Ok(format!("Agent still running (status={status})"));
        }
        let error = agent
            .get("fields")
            .and_then(|v| v.get("error_message"))
            .or_else(|| agent.get("fields").and_then(|v| v.get("ErrorMessage")))
            .or_else(|| agent.get("fields").and_then(|v| v.get("error")))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        if !error.is_empty() {
            return Ok(error);
        }
        return Ok(format!("Agent ended with status={status}"));
    }
    Err(format!("wait_for_agent failed (HTTP {})", wait_resp.status))
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
