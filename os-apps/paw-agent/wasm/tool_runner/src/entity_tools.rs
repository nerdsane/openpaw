use temper_wasm_sdk::prelude::*;

// TODO(platform): split these OData-governed tools into dedicated WASM modules
// once cross-module delegation is supported. Keeping them isolated in this Rust
// module makes the policy and extraction boundary explicit today.

pub(crate) fn is_entity_tool(name: &str) -> bool {
    matches!(
        name,
        "save_memory"
            | "recall_memory"
            | "spawn_agent"
            | "list_agents"
            | "abort_agent"
            | "steer_agent"
            | "read_entity"
            | "file_upload"
            | "temper_create"
            | "temper_get"
            | "temper_list"
            | "temper_action"
            | "run_coding_agent"
    )
}

pub(crate) fn execute(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    fields: &Value,
    tool_name: &str,
    input: &Value,
) -> Result<String, String> {
    match tool_name {
        "save_memory" => {
            let key = input
                .get("key")
                .and_then(|v| v.as_str())
                .ok_or("save_memory: missing 'key'")?;
            let content = input
                .get("content")
                .and_then(|v| v.as_str())
                .ok_or("save_memory: missing 'content'")?;
            let memory_type = input
                .get("memory_type")
                .and_then(|v| v.as_str())
                .unwrap_or("reference");
            let agent_id = ctx
                .entity_state
                .get("entity_id")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let soul_id = fields.get("soul_id").and_then(|v| v.as_str()).unwrap_or("");
            let body = json!({
                "Key": key, "Content": content, "MemoryType": memory_type,
                "AgentId": agent_id, "SoulId": soul_id,
            });
            let url = format!("{temper_api_url}/tdata/Memories");
            let resp = ctx.http_call(
                "POST",
                &url,
                &crate::odata_headers(ctx, tenant),
                &serde_json::to_string(&body).unwrap_or_default(),
            )?;
            if resp.status >= 200 && resp.status < 300 {
                let parsed: Value = serde_json::from_str(&resp.body).unwrap_or(json!({}));
                let entity_id = parsed
                    .get("entity_id")
                    .or_else(|| parsed.get("Id"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if !entity_id.is_empty() {
                    let action_url =
                        format!("{temper_api_url}/tdata/Memories('{entity_id}')/OpenPaw.Save");
                    let _ = ctx.http_call("POST", &action_url, &crate::odata_headers(ctx, tenant), "{}");
                }
                Ok(format!("Memory saved: key={key}, type={memory_type}"))
            } else {
                Err(format!(
                    "save_memory failed (HTTP {}): {}",
                    resp.status,
                    &resp.body[..resp.body.len().min(200)]
                ))
            }
        }
        "recall_memory" => {
            let query = input
                .get("query")
                .and_then(|v| v.as_str())
                .ok_or("recall_memory: missing 'query'")?;
            let entity_id = ctx
                .entity_state
                .get("entity_id")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let url = format!("{temper_api_url}/tdata/Memories");
            let resp = ctx.http_call("GET", &url, &crate::odata_headers(ctx, tenant), "")?;
            if resp.status == 200 {
                let parsed: Value = serde_json::from_str(&resp.body).unwrap_or(json!({}));
                let memories = parsed
                    .get("value")
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|mem| {
                        crate::entity_field_str(mem, &["Status"]) == Some("Active")
                            && crate::entity_field_str(mem, &["AgentId"]).unwrap_or("") == entity_id
                            && (crate::entity_field_str(mem, &["Key"])
                                .unwrap_or("")
                                .contains(query)
                                || crate::entity_field_str(mem, &["Content"])
                                    .unwrap_or("")
                                    .contains(query))
                    })
                    .collect::<Vec<_>>();
                if memories.is_empty() {
                    Ok("No memories found matching query.".to_string())
                } else {
                    let mut result = String::new();
                    for mem in &memories {
                        let k = crate::entity_field_str(mem, &["Key"]).unwrap_or("?");
                        let c = crate::entity_field_str(mem, &["Content"]).unwrap_or("");
                        let t = crate::entity_field_str(mem, &["MemoryType"]).unwrap_or("?");
                        result.push_str(&format!("- [{t}] {k}: {c}\n"));
                    }
                    Ok(result)
                }
            } else {
                Err(format!("recall_memory failed (HTTP {})", resp.status))
            }
        }
        "spawn_agent" => {
            let task = input
                .get("task")
                .and_then(|v| v.as_str())
                .ok_or("spawn_agent: missing 'task'")?;
            let requested_id = input.get("agent_id").and_then(|v| v.as_str()).unwrap_or("");
            let model = input
                .get("model")
                .and_then(|v| v.as_str())
                .unwrap_or_else(|| {
                    fields
                        .get("model")
                        .and_then(|v| v.as_str())
                        .unwrap_or("claude-sonnet-4-20250514")
                });
            let provider = input
                .get("provider")
                .and_then(|v| v.as_str())
                .unwrap_or_else(|| {
                    fields
                        .get("provider")
                        .and_then(|v| v.as_str())
                        .unwrap_or("anthropic")
                });
            let tools = input
                .get("tools")
                .and_then(|v| v.as_str())
                .unwrap_or_else(|| {
                    fields
                        .get("tools_enabled")
                        .and_then(|v| v.as_str())
                        .unwrap_or("read,write,edit,bash")
                });
            let soul_id = input
                .get("soul_id")
                .and_then(|v| v.as_str())
                .unwrap_or_else(|| fields.get("soul_id").and_then(|v| v.as_str()).unwrap_or(""));
            let normalized_soul_id = normalize_soul_ref(ctx, temper_api_url, tenant, soul_id)
                .unwrap_or_else(|| soul_id.to_string());
            let parent_id = ctx
                .entity_state
                .get("entity_id")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let inherit_sandbox = input
                .get("inherit_sandbox")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            let child_sandbox_url = input
                .get("sandbox_url")
                .and_then(|v| v.as_str())
                .filter(|v| !v.is_empty())
                .or_else(|| {
                    if inherit_sandbox {
                        fields
                            .get("sandbox_url")
                            .and_then(|v| v.as_str())
                            .filter(|v| !v.is_empty())
                    } else {
                        None
                    }
                })
                .unwrap_or("");
            let workdir = fields
                .get("workdir")
                .and_then(|v| v.as_str())
                .unwrap_or("/workspace");
            let child_workdir = input
                .get("workdir")
                .and_then(|v| v.as_str())
                .unwrap_or(workdir);
            let background = input
                .get("background")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let run_tools_timeout_secs = ctx
                .config
                .get("timeout_secs")
                .and_then(|v| v.parse::<i64>().ok())
                .unwrap_or(120);
            let default_wait_timeout_ms =
                ((run_tools_timeout_secs.saturating_sub(30)).max(30)) * 1000;
            let wait_timeout_ms = input
                .get("timeout_ms")
                .and_then(|v| v.as_i64())
                .or_else(|| {
                    ctx.config
                        .get("spawn_agent_wait_timeout_ms")
                        .and_then(|v| v.parse::<i64>().ok())
                })
                .unwrap_or(default_wait_timeout_ms)
                .max(1_000);
            let current_depth = fields
                .get("agent_depth")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            if current_depth >= 5 {
                return Err("spawn_agent: agent_depth guard hit (max depth 5)".to_string());
            }

            let url = format!("{temper_api_url}/tdata/Agents");
            let mut create_body = json!({ "ParentAgentId": parent_id });
            if !requested_id.is_empty() {
                create_body["Id"] = Value::String(requested_id.to_string());
            }
            let resp = ctx.http_call(
                "POST",
                &url,
                &crate::odata_headers(ctx, tenant),
                &create_body.to_string(),
            )?;
            if resp.status < 200 || resp.status >= 300 {
                return Err(format!("spawn_agent: create failed (HTTP {})", resp.status));
            }
            let parsed: Value = serde_json::from_str(&resp.body).unwrap_or(json!({}));
            let child_id = parsed
                .get("entity_id")
                .or_else(|| parsed.get("Id"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if child_id.is_empty() {
                return Err("spawn_agent: created entity has no Id".to_string());
            }

            let config_body = json!({
                "system_prompt": input.get("system_prompt").and_then(Value::as_str).unwrap_or(""),
                "model": model, "provider": provider, "tools_enabled": tools,
                "soul_id": normalized_soul_id, "user_message": task, "parent_agent_id": parent_id,
                "sandbox_url": child_sandbox_url, "workdir": child_workdir, "agent_depth": current_depth + 1,
            });
            let config_url =
                format!("{temper_api_url}/tdata/Agents('{child_id}')/OpenPaw.Configure");
            let resp2 = ctx.http_call(
                "POST",
                &config_url,
                &crate::odata_headers(ctx, tenant),
                &serde_json::to_string(&config_body).unwrap_or_default(),
            )?;
            if resp2.status == 403 {
                handle_cedar_denial(
                    ctx, temper_api_url, tenant, fields,
                    &config_url, "Agents", &child_id, "Configure", &config_body, &resp2.body,
                )?;
            } else if resp2.status < 200 || resp2.status >= 300 {
                return Err(format!(
                    "spawn_agent: configure failed (HTTP {})",
                    resp2.status
                ));
            }

            let prov_url = format!("{temper_api_url}/tdata/Agents('{child_id}')/OpenPaw.Provision");
            let resp3 = ctx.http_call("POST", &prov_url, &crate::odata_headers(ctx, tenant), "{}")?;
            if resp3.status == 403 {
                handle_cedar_denial(
                    ctx, temper_api_url, tenant, fields,
                    &prov_url, "Agents", &child_id, "Provision", &json!({}), &resp3.body,
                )?;
            } else if resp3.status < 200 || resp3.status >= 300 {
                return Err(format!(
                    "spawn_agent: provision failed (HTTP {})",
                    resp3.status
                ));
            }
            if background {
                return Ok(format!(
                    "Child agent {child_id} created and provisioned in background."
                ));
            }

            let result =
                wait_for_child_agent_terminal_state(ctx, temper_api_url, tenant, &child_id, wait_timeout_ms)?;
            let status = agent_status(&result);
            let agent_result = agent_result_summary(&result);
            Ok(format!(
                "Child agent {child_id} finished with status={status}. Result: {agent_result}"
            ))
        }
        "list_agents" => {
            let parent_id = ctx
                .entity_state
                .get("entity_id")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let agents = list_temper_agents(ctx, temper_api_url, tenant)?;
            let child_agents = agents
                .into_iter()
                .filter(|agent| {
                    crate::entity_field_str(agent, &["ParentAgentId"]).unwrap_or("") == parent_id
                })
                .collect::<Vec<_>>();
            if child_agents.is_empty() {
                Ok("No child agents found.".to_string())
            } else {
                let mut result = String::new();
                for agent in &child_agents {
                    let id = agent_display_id(agent);
                    let status = crate::entity_field_str(agent, &["Status"]).unwrap_or("?");
                    result.push_str(&format!("- {id}: {status}\n"));
                }
                Ok(result)
            }
        }
        "abort_agent" => {
            let agent_id = input
                .get("agent_id")
                .and_then(|v| v.as_str())
                .ok_or("abort_agent: missing 'agent_id'")?;
            let resolved_agent_id = resolve_agent_reference(ctx, temper_api_url, tenant, agent_id)?
                .map(|agent| agent_entity_id(&agent).to_string())
                .unwrap_or_else(|| agent_id.to_string());
            let url =
                format!("{temper_api_url}/tdata/Agents('{resolved_agent_id}')/OpenPaw.Cancel");
            let resp = ctx.http_call("POST", &url, &crate::odata_headers(ctx, tenant), "{}")?;
            if resp.status >= 200 && resp.status < 300 {
                Ok(format!("Agent {resolved_agent_id} cancelled."))
            } else {
                Err(format!("cancel_agent failed (HTTP {})", resp.status))
            }
        }
        "steer_agent" => {
            let agent_id = input
                .get("agent_id")
                .and_then(|v| v.as_str())
                .ok_or("steer_agent: missing 'agent_id'")?;
            let message = input
                .get("message")
                .and_then(|v| v.as_str())
                .ok_or("steer_agent: missing 'message'")?;
            let Some(agent) = resolve_agent_reference(ctx, temper_api_url, tenant, agent_id)?
            else {
                return Err(format!("steer_agent: agent '{agent_id}' not found"));
            };
            let resolved_agent_id = agent_entity_id(&agent);
            let existing = crate::entity_field_str(&agent, &["SteeringMessages"])
                .map(str::to_string)
                .unwrap_or_else(|| "[]".to_string());
            let mut queue: Vec<Value> = serde_json::from_str(&existing).unwrap_or_default();
            queue.push(json!({ "content": message }));
            let body = json!({
                "steering_messages": serde_json::to_string(&queue).unwrap_or_else(|_| "[]".to_string())
            });
            let url = format!("{temper_api_url}/tdata/Agents('{resolved_agent_id}')/OpenPaw.Steer");
            let resp = ctx.http_call(
                "POST",
                &url,
                &crate::odata_headers(ctx, tenant),
                &serde_json::to_string(&body).unwrap_or_default(),
            )?;
            if resp.status >= 200 && resp.status < 300 {
                Ok(format!(
                    "Steering message sent to agent {}.",
                    agent_display_id(&agent)
                ))
            } else {
                Err(format!("steer_agent failed (HTTP {})", resp.status))
            }
        }
        "read_entity" => {
            let file_id = input
                .get("file_id")
                .and_then(|v| v.as_str())
                .ok_or("read_entity: missing 'file_id'")?;
            let url = format!("{temper_api_url}/tdata/Files('{file_id}')/$value");
            let headers = crate::file_headers(ctx, tenant, None, None);
            let resp = ctx.http_call("GET", &url, &headers, "")?;
            if resp.status == 200 {
                Ok(resp.body)
            } else {
                Err(format!("read_entity failed (HTTP {})", resp.status))
            }
        }
        "file_upload" => {
            let name = input
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or("file_upload: missing 'name'")?;
            let content = input
                .get("content")
                .and_then(|v| v.as_str())
                .ok_or("file_upload: missing 'content'")?;
            let mime_type = input
                .get("mime_type")
                .and_then(|v| v.as_str())
                .unwrap_or("text/markdown");

            // Create File entity
            let file_body = json!({
                "Name": name,
                "MimeType": mime_type
            });
            let file_url = format!("{temper_api_url}/tdata/Files");
            let file_resp =
                ctx.http_call("POST", &file_url, &crate::odata_headers(ctx, tenant), &file_body.to_string())?;
            if file_resp.status < 200 || file_resp.status >= 300 {
                return Err(format!(
                    "file_upload: File creation failed (HTTP {}): {}",
                    file_resp.status,
                    &file_resp.body[..file_resp.body.len().min(300)]
                ));
            }
            let file_parsed: Value =
                serde_json::from_str(&file_resp.body).unwrap_or(json!({}));
            let file_id = file_parsed
                .get("entity_id")
                .or_else(|| file_parsed.get("Id"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if file_id.is_empty() {
                return Err("file_upload: File created but no Id returned".to_string());
            }

            // Upload content
            let value_url = format!("{temper_api_url}/tdata/Files('{file_id}')/$value");
            let value_headers = crate::file_headers(ctx, tenant, Some(mime_type), None);
            let value_resp = ctx.http_call("PUT", &value_url, &value_headers, content)?;
            if value_resp.status < 200 || value_resp.status >= 300 {
                return Err(format!(
                    "file_upload: content write failed (HTTP {})",
                    value_resp.status
                ));
            }

            Ok(json!({ "file_id": file_id }).to_string())
        }
        "temper_create" => {
            let entity_set = input
                .get("entity_set")
                .and_then(|v| v.as_str())
                .ok_or("temper_create: missing 'entity_set'")?;
            let body = input.get("body").cloned().unwrap_or_else(|| json!({}));
            let url = format!("{temper_api_url}/tdata/{entity_set}");
            let resp = ctx.http_call("POST", &url, &crate::odata_headers(ctx, tenant), &body.to_string())?;
            if resp.status >= 200 && resp.status < 300 {
                Ok(resp.body)
            } else if resp.status == 403 {
                handle_cedar_denial(
                    ctx, temper_api_url, tenant, fields,
                    &url, entity_set, "", "create", &body, &resp.body,
                )
            } else {
                Err(format!(
                    "temper_create failed (HTTP {}): {}",
                    resp.status,
                    &resp.body[..resp.body.len().min(400)]
                ))
            }
        }
        "temper_get" => {
            let entity_set = input
                .get("entity_set")
                .and_then(|v| v.as_str())
                .ok_or("temper_get: missing 'entity_set'")?;
            let entity_id = input
                .get("entity_id")
                .and_then(|v| v.as_str())
                .ok_or("temper_get: missing 'entity_id'")?;
            let mut url = format!("{temper_api_url}/tdata/{entity_set}('{entity_id}')");
            let mut query = Vec::new();
            if let Some(select) = input.get("select").and_then(|v| v.as_str()) {
                if !select.trim().is_empty() {
                    query.push(format!("$select={}", crate::url_encode(select.trim())));
                }
            }
            if let Some(expand) = input.get("expand").and_then(|v| v.as_str()) {
                if !expand.trim().is_empty() {
                    query.push(format!("$expand={}", crate::url_encode(expand.trim())));
                }
            }
            if !query.is_empty() {
                url.push('?');
                url.push_str(&query.join("&"));
            }
            let resp = ctx.http_call("GET", &url, &crate::odata_headers(ctx, tenant), "")?;
            if resp.status == 200 {
                Ok(resp.body)
            } else {
                Err(format!(
                    "temper_get failed (HTTP {}): {}",
                    resp.status,
                    &resp.body[..resp.body.len().min(400)]
                ))
            }
        }
        "temper_list" => {
            let entity_set = input
                .get("entity_set")
                .and_then(|v| v.as_str())
                .ok_or("temper_list: missing 'entity_set'")?;
            let mut url = format!("{temper_api_url}/tdata/{entity_set}");
            let mut query = Vec::new();
            if let Some(filter) = input.get("filter").and_then(|v| v.as_str()) {
                if !filter.trim().is_empty() {
                    query.push(format!("$filter={}", crate::url_encode(filter.trim())));
                }
            }
            if let Some(select) = input.get("select").and_then(|v| v.as_str()) {
                if !select.trim().is_empty() {
                    query.push(format!("$select={}", crate::url_encode(select.trim())));
                }
            }
            if let Some(orderby) = input.get("orderby").and_then(|v| v.as_str()) {
                if !orderby.trim().is_empty() {
                    query.push(format!("$orderby={}", crate::url_encode(orderby.trim())));
                }
            }
            if let Some(top) = input.get("top").and_then(|v| v.as_i64()) {
                if top > 0 {
                    query.push(format!("$top={top}"));
                }
            }
            if !query.is_empty() {
                url.push('?');
                url.push_str(&query.join("&"));
            }
            let resp = ctx.http_call("GET", &url, &crate::odata_headers(ctx, tenant), "")?;
            if resp.status == 200 {
                Ok(resp.body)
            } else {
                Err(format!(
                    "temper_list failed (HTTP {}): {}",
                    resp.status,
                    &resp.body[..resp.body.len().min(400)]
                ))
            }
        }
        "temper_action" => {
            let entity_set = input
                .get("entity_set")
                .and_then(|v| v.as_str())
                .ok_or("temper_action: missing 'entity_set'")?;
            let entity_id = input
                .get("entity_id")
                .and_then(|v| v.as_str())
                .ok_or("temper_action: missing 'entity_id'")?;
            let action = input
                .get("action")
                .and_then(|v| v.as_str())
                .ok_or("temper_action: missing 'action'")?;
            let body = input.get("body").cloned().unwrap_or_else(|| json!({}));
            let bound_action = resolve_bound_action_name(entity_set, action);
            let url = format!("{temper_api_url}/tdata/{entity_set}('{entity_id}')/{bound_action}");
            let resp = ctx.http_call("POST", &url, &crate::odata_headers(ctx, tenant), &body.to_string())?;
            if resp.status >= 200 && resp.status < 300 {
                Ok(resp.body)
            } else if resp.status == 403 {
                handle_cedar_denial(
                    ctx, temper_api_url, tenant, fields,
                    &url, entity_set, entity_id, action, &body, &resp.body,
                )
            } else {
                Err(format!(
                    "temper_action failed (HTTP {}): {}",
                    resp.status,
                    &resp.body[..resp.body.len().min(400)]
                ))
            }
        }
        "run_coding_agent" => {
            let agent_type = input
                .get("agent_type")
                .and_then(|v| v.as_str())
                .ok_or("run_coding_agent: missing 'agent_type'")?;
            let task = input
                .get("task")
                .and_then(|v| v.as_str())
                .ok_or("run_coding_agent: missing 'task'")?;
            let agent_workdir = input
                .get("workdir")
                .and_then(|v| v.as_str())
                .unwrap_or_else(|| {
                    fields
                        .get("workdir")
                        .and_then(|v| v.as_str())
                        .unwrap_or("/workspace")
                });
            let background = input
                .get("background")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let sandbox_url = fields
                .get("sandbox_url")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if sandbox_url.is_empty() {
                return Err("run_coding_agent: sandbox_url is empty".to_string());
            }
            let escaped_task = task.replace('\'', "'\\''");
            let command = match agent_type {
                "claude-code" => format!(
                    "cd {agent_workdir} && claude --permission-mode bypassPermissions --print '{escaped_task}'"
                ),
                "codex" => format!("cd {agent_workdir} && codex exec '{escaped_task}'"),
                "pi" => format!("cd {agent_workdir} && pi -p '{escaped_task}'"),
                "opencode" => format!("cd {agent_workdir} && opencode run '{escaped_task}'"),
                _ => return Err(format!("unsupported coding agent type: {agent_type}")),
            };
            let final_cmd = if background {
                format!(
                    "nohup bash -c '{command}' > /tmp/coding-agent-{agent_type}.log 2>&1 & echo $!"
                )
            } else {
                command
            };
            // Use the shared run_bash_local which handles Tensorlake async process API.
            let output = crate::run_bash_local(ctx, sandbox_url, &final_cmd, agent_workdir)?;
            Ok(format!("Command: {final_cmd}\n{output}"))
        }
        _ => Err(format!("unknown entity tool: {tool_name}")),
    }
}

fn agent_status(value: &Value) -> &str {
    crate::entity_field_str(value, &["Status"]).unwrap_or("Unknown")
}

fn agent_result_summary(value: &Value) -> &str {
    crate::entity_field_str(value, &["Result"]).unwrap_or("")
}

fn is_terminal_agent_status(status: &str) -> bool {
    matches!(status, "Completed" | "Failed" | "Cancelled")
}

fn wait_for_child_agent_terminal_state(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    child_id: &str,
    wait_timeout_ms: i64,
) -> Result<Value, String> {
    let wait_url = format!(
        "{temper_api_url}/observe/entities/Agent/{child_id}/wait?statuses=Completed,Failed,Cancelled&timeout_ms={wait_timeout_ms}&poll_ms=250"
    );
    let resp = ctx.http_call("GET", &wait_url, &crate::odata_headers(ctx, tenant), "")?;
    if resp.status < 200 || resp.status >= 300 {
        return Err(format!(
            "spawn_agent: wait failed for child {child_id} (HTTP {})",
            resp.status
        ));
    }
    let entity: Value = serde_json::from_str(&resp.body).unwrap_or_else(|_| json!({}));
    let status = agent_status(&entity).to_string();
    if is_terminal_agent_status(&status) {
        return Ok(entity);
    }
    let timed_out = entity
        .get("timed_out")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if !timed_out {
        return Err(format!(
            "spawn_agent: child {child_id} returned non-terminal status={status}"
        ));
    }
    Err(format!(
        "spawn_agent: child {child_id} did not reach a terminal state within {wait_timeout_ms}ms; last status={status}"
    ))
}

/// Handle a Cedar 403 denial by creating a PendingApproval entity and
/// returning immediately. Does NOT block or poll.
///
/// The flow is event-driven:
/// 1. This creates a PendingApproval and dispatches Request (→ Discord buttons)
/// 2. Returns Ok with a message — the LLM sees this as a tool result
/// 3. When the human clicks Approve, the `approval_granted` WASM fires,
///    executes the action with system identity, and notifies via Discord
fn handle_cedar_denial(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    _fields: &Value,
    original_url: &str,
    entity_set: &str,
    target_entity_id: &str,
    action: &str,
    body: &Value,
    resp_body: &str,
) -> Result<String, String> {
    let decision_id = parse_decision_id(resp_body);
    let action_desc = format!("{action} on {entity_set}('{target_entity_id}')");

    ctx.log(
        "info",
        &format!("Cedar denied: {action_desc} — requesting human approval"),
    );

    // Create PendingApproval entity
    let create_url = format!("{temper_api_url}/tdata/PendingApprovals");
    let create_resp = ctx.http_call(
        "POST",
        &create_url,
        &crate::odata_headers(ctx, tenant),
        "{}",
    )?;
    if create_resp.status < 200 || create_resp.status >= 300 {
        return Err(format!(
            "Failed to create PendingApproval (HTTP {}): {}. Original denial: {action_desc}",
            create_resp.status,
            &create_resp.body[..create_resp.body.len().min(200)]
        ));
    }
    let parsed: Value =
        serde_json::from_str(&create_resp.body).unwrap_or_else(|_| json!({}));
    let approval_id = parsed
        .get("entity_id")
        .or_else(|| parsed.get("Id"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if approval_id.is_empty() {
        return Err(format!(
            "PendingApproval created but has no ID. Original denial: {action_desc}"
        ));
    }

    // Dispatch Request action (triggers request_approval WASM → Discord buttons)
    let request_body = json!({
        "agent_entity_id": ctx.entity_state.get("entity_id")
            .and_then(|v| v.as_str()).unwrap_or(""),
        "action_description": action_desc,
        "entity_set": entity_set,
        "target_entity_id": target_entity_id,
        "target_action": action,
        "target_body": body.to_string(),
        "target_url": original_url,
        "decision_id": decision_id,
    });
    let request_url = format!(
        "{temper_api_url}/tdata/PendingApprovals('{approval_id}')/OpenPaw.Request"
    );
    let req_resp = ctx.http_call(
        "POST",
        &request_url,
        &crate::odata_headers(ctx, tenant),
        &serde_json::to_string(&request_body).unwrap_or_default(),
    )?;
    if req_resp.status < 200 || req_resp.status >= 300 {
        return Err(format!(
            "PendingApproval.Request failed (HTTP {}). Original denial: {action_desc}",
            req_resp.status,
        ));
    }

    // Return immediately — don't block. The approval_granted WASM will
    // execute the action when the human clicks Approve in Discord.
    Ok(format!(
        "Authorization required. A human has been asked to approve: {action_desc}. \
         PendingApproval:{approval_id}. The action will be executed automatically \
         when approved. You do not need to retry — just inform the user that \
         approval is pending."
    ))
}

/// Extract the Cedar decision ID from a 403 response body.
/// Expected format: `{"error":{"code":"AuthorizationDenied","message":"... (decision: PD-...)"}}`
fn parse_decision_id(body: &str) -> String {
    if let Some(start) = body.find("PD-") {
        let rest = &body[start..];
        let end = rest
            .find(|c: char| c == ')' || c == '"' || c == '}' || c.is_whitespace())
            .unwrap_or(rest.len());
        return rest[..end].to_string();
    }
    String::new()
}

fn resolve_bound_action_name(entity_set: &str, action: &str) -> String {
    let ns = match entity_set {
        "Monitors" | "AlertCycles" | "MonitorScans" => "OpenPaw.Heal",
        "ProjectHarnesses" | "WorkCycles" => "OpenPaw.Harness",
        "Agents" | "Souls" | "PendingApprovals" => "OpenPaw",
        "Issues" | "Plans" => "Paw.PM",
        "Channels" | "AgentRoutes" | "ChannelSessions" => "Paw.Channel",
        _ => "OpenPaw",
    };
    if action.contains('.') {
        action.to_string()
    } else {
        format!("{ns}.{action}")
    }
}

fn agent_entity_id<'a>(agent: &'a Value) -> &'a str {
    crate::entity_field_str(agent, &["Id", "entity_id", "id"]).unwrap_or("")
}

fn agent_display_id<'a>(agent: &'a Value) -> &'a str {
    crate::entity_field_str(agent, &["AgentId", "Id", "entity_id", "id"]).unwrap_or("?")
}

fn list_temper_agents(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
) -> Result<Vec<Value>, String> {
    let url = format!("{temper_api_url}/tdata/Agents");
    let resp = ctx.http_call("GET", &url, &crate::odata_headers(ctx, tenant), "")?;
    if resp.status != 200 {
        return Err(format!(
            "temper agent listing failed (HTTP {})",
            resp.status
        ));
    }
    let parsed: Value = serde_json::from_str(&resp.body).unwrap_or_else(|_| json!({}));
    Ok(parsed
        .get("value")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default())
}

fn resolve_agent_reference(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    agent_reference: &str,
) -> Result<Option<Value>, String> {
    let agents = list_temper_agents(ctx, temper_api_url, tenant)?;
    Ok(agents.into_iter().find(|agent| {
        let entity_id = agent_entity_id(agent);
        let temper_agent_id = crate::entity_field_str(agent, &["AgentId"]).unwrap_or("");
        entity_id == agent_reference || temper_agent_id == agent_reference
    }))
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
    if let Ok(by_id_resp) = ctx.http_call("GET", &by_id_url, &crate::odata_headers(ctx, tenant), "") {
        if by_id_resp.status == 200 {
            let parsed: Value =
                serde_json::from_str(&by_id_resp.body).unwrap_or_else(|_| json!({}));
            return crate::entity_field_str(&parsed, &["Name"]).map(ToString::to_string);
        }
    }

    let escaped = soul_ref.replace('\'', "''");
    let by_name_url =
        format!("{temper_api_url}/tdata/Souls?$filter=Name eq '{escaped}' and Status eq 'Active'");
    if let Ok(by_name_resp) = ctx.http_call("GET", &by_name_url, &crate::odata_headers(ctx, tenant), "") {
        if by_name_resp.status != 200 {
            return None;
        }
        let parsed: Value = serde_json::from_str(&by_name_resp.body).unwrap_or_else(|_| json!({}));
        return parsed
            .get("value")
            .and_then(Value::as_array)
            .and_then(|souls| souls.first())
            .and_then(|soul| crate::entity_field_str(soul, &["Name", "Id"]))
            .map(ToString::to_string);
    }

    None
}
