//! Entity operations ported from tool_runner/entity_tools.rs.
//!
//! These methods are dispatched from `temper.<method>()` calls in Monty code.
//! They use the same HTTP patterns as dispatch.rs (ctx.http_call, JSON serialization).

use serde_json::{Value, json};
use temper_wasm_sdk::context::Context;

use crate::dispatch;

const DEFAULT_TOOLS_ENABLED: &str = "temper_create,temper_get,temper_list,temper_action,temper_patch,temper_submit_specs,temper_show_spec,temper_specs,temper_upload_wasm,temper_get_trajectories,temper_get_insights,temper_get_decisions,temper_poll_decision,temper_approve_decision,temper_deny_decision,temper_submit_policy,temper_list_policies,temper_get_policy,temper_update_policy,temper_delete_policy,temper_install_app,temper_create_app,temper_list_apps,temper_spawn_session,temper_list_sessions,temper_abort_session,temper_steer_session,temper_save_memory,temper_recall_memory,temper_write,temper_read,temper_ls,temper_grep,temper_glob,temper_edit,temper_rename,temper_search_history,temper_run_coding_agent,temper_get_secret,temper_datadog_query,temper_railway,temper_vercel,temper_web_search,temper_web_fetch,read,write,edit,bash";

// ---------------------------------------------------------------------------
// spawn_session
// ---------------------------------------------------------------------------

pub fn spawn_session(
    ctx: &Context,
    api_url: &str,
    tenant: &str,
    sandbox_url: &str,
    workdir: &str,
    args: &[Value],
) -> Result<Value, String> {
    let input = obj_arg(args, 0, "opts", "spawn_session")?;

    let task = require_str(&input, "task", "spawn_session")?;
    let requested_id = input.get("agent_id").and_then(|v| v.as_str()).unwrap_or("");

    // Extract parent session fields early — used for provider/model inheritance,
    // workspace sharing, depth guard, and project context.
    let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));
    let parent_id = ctx
        .entity_state
        .get("entity_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    // Three-tier fallback: explicit input → parent session fields → hardcoded default.
    let parent_provider = fields.get("provider").and_then(|v| v.as_str()).unwrap_or("");
    let parent_model = fields.get("model").and_then(|v| v.as_str()).unwrap_or("");
    let parent_temperature = fields.get("temperature").and_then(|v| v.as_str()).unwrap_or("");
    let model = input
        .get("model")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .or_else(|| if parent_model.is_empty() { None } else { Some(parent_model) })
        .unwrap_or("claude-sonnet-4-6");
    let provider = input
        .get("provider")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .or_else(|| if parent_provider.is_empty() { None } else { Some(parent_provider) })
        .unwrap_or("anthropic");
    let temperature = input
        .get("temperature")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .or_else(|| if parent_temperature.is_empty() { None } else { Some(parent_temperature) })
        .unwrap_or("1.0");
    let tools = input
        .get("tools")
        .and_then(|v| v.as_str())
        .unwrap_or(DEFAULT_TOOLS_ENABLED);
    let soul_id = input.get("soul_id").and_then(|v| v.as_str()).unwrap_or("");
    // Workspace sharing: allow child to reuse parent's workspace.
    let share_workspace = input
        .get("share_workspace")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let parent_workspace_id = fields
        .get("workspace_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let child_workspace_id = input
        .get("workspace_id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .or_else(|| {
            if share_workspace && !parent_workspace_id.is_empty() {
                Some(parent_workspace_id)
            } else {
                None
            }
        })
        .unwrap_or("");

    let child_sandbox_url = input
        .get("sandbox_url")
        .and_then(|v| v.as_str())
        .filter(|v| !v.is_empty())
        .unwrap_or(sandbox_url);
    let child_workdir = input
        .get("workdir")
        .and_then(|v| v.as_str())
        .unwrap_or(workdir);
    let background = input
        .get("background")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let max_turns = input
        .get("max_turns")
        .and_then(|v| v.as_str())
        .unwrap_or("20");
    let mode = input
        .get("mode")
        .and_then(|v| v.as_str())
        .unwrap_or("execute");
    // In plan mode, override tools to PLAN_MODE_TOOLS unless explicitly provided
    let tools = if mode == "plan" && input.get("tools").is_none() {
        super::dispatch::PLAN_MODE_TOOLS
    } else {
        tools
    };

    let run_tools_timeout_secs = ctx
        .config
        .get("timeout_secs")
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(120);
    let default_wait_timeout_ms = ((run_tools_timeout_secs.saturating_sub(30)).max(30)) * 1000;
    let wait_timeout_ms = input
        .get("timeout_ms")
        .and_then(|v| v.as_i64())
        .or_else(|| {
            ctx.config
                .get("spawn_session_wait_timeout_ms")
                .and_then(|v| v.parse::<i64>().ok())
        })
        .unwrap_or(default_wait_timeout_ms)
        .max(1_000);

    // Depth guard (fields already extracted above)
    let current_depth = fields
        .get("session_depth")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    if current_depth >= 5 {
        return Err("spawn_session: agent_depth guard hit (max depth 5)".into());
    }

    // Create Session entity
    let mut create_body = json!({ "ParentSessionId": parent_id });
    if !requested_id.is_empty() {
        create_body["Id"] = Value::String(requested_id.to_string());
    }
    let resp = http_post(ctx, api_url, tenant, parent_id, "/tdata/Sessions", &create_body)?;
    let child_id = resp
        .get("entity_id")
        .or_else(|| resp.get("Id"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if child_id.is_empty() {
        return Err("spawn_session: created entity has no Id".into());
    }

    // Configure
    let config_body = json!({
        "system_prompt": input.get("system_prompt").and_then(Value::as_str).unwrap_or(""),
        "model": model, "provider": provider, "temperature": temperature, "tools_enabled": tools,
        "soul_id": soul_id, "user_message": task, "parent_session_id": parent_id,
        "sandbox_url": child_sandbox_url, "workdir": child_workdir,
        "workspace_id": child_workspace_id,
        "session_depth": current_depth + 1, "max_turns": max_turns,
        "session_mode": mode,
        "project_harness_id": input
            .get("project_harness_id")
            .and_then(|v| v.as_str())
            .or_else(|| fields.get("project_harness_id").and_then(|v| v.as_str()))
            .unwrap_or(""),
        "project_id": input
            .get("project_id")
            .and_then(|v| v.as_str())
            .or_else(|| fields.get("project_id").and_then(|v| v.as_str()))
            .unwrap_or(""),
    });
    http_post(
        ctx,
        api_url,
        tenant,
        parent_id,
        &format!("/tdata/Sessions('{child_id}')/OpenPaw.Configure"),
        &config_body,
    )?;

    // Configure schedules ProvisionWorkspace automatically (ADR-0022).
    // Sandbox is provisioned lazily on first sandbox tool call.

    if background {
        return Ok(json!({
            "session_id": child_id,
            "status": "provisioning",
            "background": true,
        }));
    }

    // Wait for completion
    let wait_url = format!(
        "/observe/entities/Session/{child_id}/wait?statuses=Completed,Failed,Cancelled&timeout_ms={wait_timeout_ms}&poll_ms=250"
    );
    let entity = http_get(ctx, api_url, tenant, parent_id, &wait_url)?;
    let status = entity_field_str_val(&entity, "Status");
    let result = entity_field_str_val(&entity, "Result");

    Ok(json!({
        "session_id": child_id,
        "status": status,
        "result": result,
    }))
}

// ---------------------------------------------------------------------------
// list_sessions
// ---------------------------------------------------------------------------

pub fn list_sessions(
    ctx: &Context,
    api_url: &str,
    tenant: &str,
    args: &[Value],
) -> Result<Value, String> {
    let input = obj_arg_or_empty(args, 0);
    let eid = ctx_entity_id(ctx);

    let filter = input.get("filter").and_then(|v| v.as_str()).unwrap_or("");
    let top = input.get("top").and_then(|v| v.as_i64()).unwrap_or(50);

    let mut path = String::from("/tdata/Sessions");
    let mut query_parts: Vec<String> = Vec::new();
    if !filter.is_empty() {
        query_parts.push(format!("$filter={}", urlenc(filter)));
    } else if !eid.is_empty() {
        query_parts.push(format!(
            "$filter=ParentSessionId%20eq%20'{}'",
            urlenc(eid)
        ));
    }
    if top > 0 {
        query_parts.push(format!("$top={top}"));
    }
    if !query_parts.is_empty() {
        path.push('?');
        path.push_str(&query_parts.join("&"));
    }

    let resp = http_get(ctx, api_url, tenant, eid, &path)?;
    Ok(resp.get("value").cloned().unwrap_or(resp))
}

// ---------------------------------------------------------------------------
// abort_session
// ---------------------------------------------------------------------------

pub fn abort_session(
    ctx: &Context,
    api_url: &str,
    tenant: &str,
    args: &[Value],
) -> Result<Value, String> {
    let input = obj_arg(args, 0, "opts", "abort_session")?;
    let session_id = require_str(&input, "session_id", "abort_session")?;
    let eid = ctx_entity_id(ctx);
    http_post(
        ctx,
        api_url,
        tenant,
        eid,
        &format!("/tdata/Sessions('{session_id}')/OpenPaw.Cancel"),
        &json!({}),
    )?;
    Ok(json!({ "session_id": session_id, "status": "cancelled" }))
}

// ---------------------------------------------------------------------------
// steer_session
// ---------------------------------------------------------------------------

pub fn steer_session(
    ctx: &Context,
    api_url: &str,
    tenant: &str,
    args: &[Value],
) -> Result<Value, String> {
    let input = obj_arg(args, 0, "opts", "steer_session")?;
    let session_id = require_str(&input, "session_id", "steer_session")?;
    let message = require_str(&input, "message", "steer_session")?;
    let eid = ctx_entity_id(ctx);

    // Get current steering messages
    let entity = http_get(
        ctx,
        api_url,
        tenant,
        eid,
        &format!("/tdata/Sessions('{session_id}')"),
    )?;
    let existing = entity_field_str_val(&entity, "SteeringMessages");
    let existing = if existing.is_empty() { "[]" } else { &existing };
    let mut queue: Vec<Value> = serde_json::from_str(existing).unwrap_or_default();
    queue.push(json!({ "content": message }));

    let body = json!({
        "steering_messages": serde_json::to_string(&queue).unwrap_or_else(|_| "[]".to_string())
    });
    http_post(
        ctx,
        api_url,
        tenant,
        eid,
        &format!("/tdata/Sessions('{session_id}')/OpenPaw.Steer"),
        &body,
    )?;
    Ok(json!({ "session_id": session_id, "steered": true }))
}

// ---------------------------------------------------------------------------
// save_memory
// ---------------------------------------------------------------------------

pub fn save_memory(
    ctx: &Context,
    api_url: &str,
    tenant: &str,
    args: &[Value],
) -> Result<Value, String> {
    let input = obj_arg(args, 0, "opts", "save_memory")?;
    let key = require_str(&input, "key", "save_memory")?;
    let content = require_str(&input, "content", "save_memory")?;
    let memory_type = input
        .get("memory_type")
        .and_then(|v| v.as_str())
        .unwrap_or("reference");
    let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));
    let agent_id = fields
        .get("agent_id")
        .or_else(|| fields.get("AgentId"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let soul_id = fields.get("soul_id").and_then(|v| v.as_str()).unwrap_or("");

    let body = json!({
        "Key": key, "Content": content, "MemoryType": memory_type,
        "AgentId": agent_id, "SoulId": soul_id,
    });
    let eid = ctx_entity_id(ctx);
    let resp = http_post(ctx, api_url, tenant, eid, "/tdata/Memories", &body)?;

    // Dispatch Save action
    let memory_id = resp
        .get("entity_id")
        .or_else(|| resp.get("Id"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if !memory_id.is_empty() {
        let _ = http_post(
            ctx,
            api_url,
            tenant,
            eid,
            &format!("/tdata/Memories('{memory_id}')/OpenPaw.Save"),
            &json!({}),
        );
    }
    Ok(json!({ "saved": true, "key": key, "memory_type": memory_type }))
}

// ---------------------------------------------------------------------------
// recall_memory
// ---------------------------------------------------------------------------

pub fn recall_memory(
    ctx: &Context,
    api_url: &str,
    tenant: &str,
    args: &[Value],
) -> Result<Value, String> {
    let input = obj_arg(args, 0, "opts", "recall_memory")?;
    let query = require_str(&input, "query", "recall_memory")?;
    let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));
    let agent_id = fields
        .get("agent_id")
        .or_else(|| fields.get("AgentId"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let eid = ctx_entity_id(ctx);
    let resp = http_get(ctx, api_url, tenant, eid, "/tdata/Memories")?;
    let memories = resp
        .get("value")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|mem| {
            entity_field_str_val(mem, "Status") == "Active"
                && entity_field_str_val(mem, "AgentId") == agent_id
                && (entity_field_str_val(mem, "Key").contains(query)
                    || entity_field_str_val(mem, "Content").contains(query))
        })
        .collect::<Vec<_>>();

    Ok(json!({ "memories": memories }))
}

// ---------------------------------------------------------------------------
// write — temper.write(path, content, opts?)
// ---------------------------------------------------------------------------

pub fn write(
    ctx: &Context,
    api_url: &str,
    tenant: &str,
    args: &[Value],
) -> Result<Value, String> {
    let path = pos_str(args, 0, "path", "write")?;
    let content = pos_str(args, 1, "content", "write")?;
    let opts = obj_arg_or_empty(args, 2);

    let mime_type = opts
        .get("mime_type")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| mime_from_ext(&path).to_string());

    // 1. Resolve the target workspace. Prefer an explicit workspace override,
    // then the session's attached workspace_id, then the legacy "default"
    // workspace name.
    let ws_id = resolve_workspace_id(ctx, api_url, tenant, &opts, true)?;

    // 2. Parse path to get dir_path for MkDir.
    let dir_path = match path.rsplit_once('/') {
        Some(("", _)) => "/",
        Some((d, _)) => d,
        None => "/",
    };

    let eid = ctx_entity_id(ctx);

    // 3. MkDir — create directory hierarchy (FUSE: mkdir -p).
    http_post(
        ctx,
        api_url,
        tenant,
        eid,
        &format!(
            "/tdata/Workspaces('{ws_id}')/Temper.MkDir?await_integration=true"
        ),
        &json!({"path": dir_path}),
    )?;

    // 4. CreateFile — create file entity at path (FUSE: creat).
    let resp = http_post(
        ctx,
        api_url,
        tenant,
        eid,
        &format!(
            "/tdata/Workspaces('{ws_id}')/Temper.CreateFile?await_integration=true"
        ),
        &json!({"path": path, "mime_type": mime_type}),
    )?;

    // Extract file_id from workspace state fields.
    let file_id = resp
        .get("fields")
        .and_then(|f| f.get("last_file_id"))
        .or_else(|| resp.get("last_file_id"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if file_id.is_empty() {
        return Err("temper.write(): CreateFile succeeded but no file_id returned".into());
    }

    // 5. PUT $value — upload content (FUSE: write).
    let url = format!("{api_url}/tdata/Files('{file_id}')/$value");
    let headers = vec![
        ("X-Tenant-Id".to_string(), tenant.to_string()),
        ("Content-Type".to_string(), mime_type.to_string()),
        ("x-temper-principal-kind".to_string(), "agent".to_string()),
        ("x-temper-principal-id".to_string(), eid.to_string()),
        ("x-temper-agent-type".to_string(), "system".to_string()),
    ];
    let resp = ctx.http_call("PUT", &url, &headers, &content)?;
    if resp.status >= 400 {
        return Err(format!(
            "temper.write(): content upload failed (HTTP {})",
            resp.status
        ));
    }

    Ok(json!({
        "file_id": file_id,
        "path": path,
        "workspace_id": ws_id,
    }))
}

// ---------------------------------------------------------------------------
// read — temper.read(path, opts?)
// ---------------------------------------------------------------------------

pub fn read(
    ctx: &Context,
    api_url: &str,
    tenant: &str,
    args: &[Value],
) -> Result<Value, String> {
    let path = pos_str(args, 0, "path", "read")?;
    let opts = obj_arg_or_empty(args, 1);

    // 1. Resolve the target workspace. Prefer an explicit workspace override,
    // then the session's attached workspace_id, then the legacy "default"
    // workspace name.
    let ws_id = resolve_workspace_id(ctx, api_url, tenant, &opts, false)?;

    let eid = ctx_entity_id(ctx);

    // 2. ResolvePath — resolve path to file_id (FUSE: stat).
    let resp = http_post(
        ctx,
        api_url,
        tenant,
        eid,
        &format!(
            "/tdata/Workspaces('{ws_id}')/Temper.ResolvePath?await_integration=true"
        ),
        &json!({"path": path}),
    )?;

    let file_id = resp
        .get("fields")
        .and_then(|f| f.get("last_file_id"))
        .or_else(|| resp.get("last_file_id"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if file_id.is_empty() {
        return Err(format!("temper.read(): file not found at '{path}'"));
    }

    // 3. GET $value — read content (FUSE: read).
    let url = format!("{api_url}/tdata/Files('{file_id}')/$value");
    let headers = vec![("Accept".to_string(), "application/octet-stream".to_string())];
    let resp = ctx.http_call("GET", &url, &headers, "")?;
    if resp.status >= 400 {
        return Err(format!("temper.read(): content read failed (HTTP {})", resp.status));
    }

    let content = resp.body;

    // Support offset/limit for partial reads (0-indexed line numbers)
    let offset = opts.get("offset").and_then(|v| v.as_u64()).map(|v| v as usize);
    let limit = opts.get("limit").and_then(|v| v.as_u64()).map(|v| v as usize);

    if offset.is_some() || limit.is_some() {
        let lines: Vec<&str> = content.lines().collect();
        let start = offset.unwrap_or(0);
        let end = limit.map(|l| (start + l).min(lines.len())).unwrap_or(lines.len());
        if start >= lines.len() {
            return Ok(json!({
                "content": "",
                "total_lines": lines.len(),
                "offset": start,
                "limit": limit.unwrap_or(0),
            }));
        }
        let numbered: Vec<String> = lines[start..end]
            .iter()
            .enumerate()
            .map(|(i, line)| format!("{}\t{}", start + i + 1, line))
            .collect();
        return Ok(json!({
            "content": numbered.join("\n"),
            "total_lines": lines.len(),
            "offset": start,
            "limit": end - start,
        }));
    }

    Ok(json!(content))
}

// ---------------------------------------------------------------------------
// ls — temper.ls(path, opts?)
// ---------------------------------------------------------------------------

pub fn ls(
    ctx: &Context,
    api_url: &str,
    tenant: &str,
    args: &[Value],
) -> Result<Value, String> {
    let path = pos_str(args, 0, "path", "ls")?;
    let opts = obj_arg_or_empty(args, 1);
    let ws_id = resolve_workspace_id(ctx, api_url, tenant, &opts, false)?;
    let eid = ctx_entity_id(ctx);

    let resp = http_post(
        ctx,
        api_url,
        tenant,
        eid,
        &format!("/tdata/Workspaces('{ws_id}')/Temper.ListDir?await_integration=true"),
        &json!({"path": path}),
    )?;

    let listing = resp
        .get("fields")
        .and_then(|f| f.get("last_listing"))
        .or_else(|| resp.get("last_listing"))
        .and_then(|v| v.as_str())
        .unwrap_or("[]");

    let parsed: Value = serde_json::from_str(listing).unwrap_or(json!([]));
    Ok(parsed)
}

// ---------------------------------------------------------------------------
// edit — temper.edit(path, old_string, new_string, opts?)
// ---------------------------------------------------------------------------

pub fn edit(
    ctx: &Context,
    api_url: &str,
    tenant: &str,
    args: &[Value],
) -> Result<Value, String> {
    let path = pos_str(args, 0, "path", "edit")?;
    let old_string = pos_str(args, 1, "old_string", "edit")?;
    let new_string = pos_str(args, 2, "new_string", "edit")?;
    let opts = obj_arg_or_empty(args, 3);

    // Read current content
    let read_args = vec![json!(path.clone()), opts.clone()];
    let content_val = read(ctx, api_url, tenant, &read_args)?;
    let content = content_val.as_str().unwrap_or("").to_string();

    if !content.contains(&old_string) {
        return Err(format!("temper.edit(): old_string not found in file at '{path}'"));
    }

    let new_content = content.replacen(&old_string, &new_string, 1);

    // Write back
    let write_args = vec![json!(path), json!(new_content), opts];
    write(ctx, api_url, tenant, &write_args)?;

    Ok(json!({"ok": true}))
}

// ---------------------------------------------------------------------------
// rename — temper.rename(old_path, new_path, opts?)
// ---------------------------------------------------------------------------

pub fn rename(
    ctx: &Context,
    api_url: &str,
    tenant: &str,
    args: &[Value],
) -> Result<Value, String> {
    let old_path = pos_str(args, 0, "old_path", "rename")?;
    let new_path = pos_str(args, 1, "new_path", "rename")?;
    let opts = obj_arg_or_empty(args, 2);
    let ws_id = resolve_workspace_id(ctx, api_url, tenant, &opts, false)?;
    let eid = ctx_entity_id(ctx);

    let resp = http_post(
        ctx,
        api_url,
        tenant,
        eid,
        &format!("/tdata/Workspaces('{ws_id}')/Temper.Rename?await_integration=true"),
        &json!({"path": old_path, "new_path": new_path}),
    )?;

    let file_id = resp
        .get("fields")
        .and_then(|f| f.get("last_file_id"))
        .or_else(|| resp.get("last_file_id"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    Ok(json!({
        "file_id": file_id,
        "old_path": old_path,
        "new_path": new_path,
    }))
}

// ---------------------------------------------------------------------------
// search_history — temper.search_history(pattern)
// ---------------------------------------------------------------------------

pub fn search_history(
    ctx: &Context,
    api_url: &str,
    tenant: &str,
    args: &[Value],
) -> Result<Value, String> {
    let pattern = pos_str(args, 0, "pattern", "search_history")?;
    let eid = ctx_entity_id(ctx);

    // Get session_file_id from entity state
    let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));
    let session_file_id = fields
        .get("session_file_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if session_file_id.is_empty() {
        return Err("search_history: no session_file_id in entity state".into());
    }

    // Read full session JSONL
    let url = format!("{api_url}/tdata/Files('{session_file_id}')/$value");
    let headers = runtime_headers(tenant, eid);
    let resp = ctx.http_call("GET", &url, &headers, "")?;
    if resp.status >= 400 {
        return Err(format!(
            "search_history: failed to read session file (HTTP {})",
            resp.status
        ));
    }

    let tree = session_tree_lib::SessionTree::from_jsonl(&resp.body);
    let pattern_lower = pattern.to_lowercase();
    let mut matches: Vec<Value> = Vec::new();
    let mut content_fetches = 0;
    const MAX_CONTENT_FETCHES: usize = 20;
    const MAX_RESULTS: usize = 50;

    for entry_id in tree.entry_ids() {
        if matches.len() >= MAX_RESULTS {
            break;
        }

        let entry = match tree.get(entry_id) {
            Some(e) => e,
            None => continue,
        };

        // Extract role from entry data
        let role = entry
            .data
            .get("role")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let entry_type_str = entry.entry_type.as_str();

        // Try inline content first
        let inline_content = extract_entry_text(&entry.data);
        let inline_lower = inline_content.to_lowercase();

        if inline_lower.contains(&pattern_lower) {
            // Find matching excerpt
            let excerpt = extract_excerpt(&inline_content, &pattern, 500);
            matches.push(json!({
                "entry_id": entry_id,
                "role": role,
                "entry_type": entry_type_str,
                "excerpt": excerpt,
                "source": "inline",
            }));
            continue;
        }

        // If entry has content_file_id and no inline match, fetch the file content
        if let Some(ref content_file_id) = entry.content_file_id {
            if content_fetches >= MAX_CONTENT_FETCHES {
                continue;
            }
            content_fetches += 1;

            let file_url = format!("{api_url}/tdata/Files('{content_file_id}')/$value");
            let file_resp = ctx.http_call("GET", &file_url, &headers, "");
            if let Ok(file_resp) = file_resp {
                if file_resp.status < 400 {
                    let file_lower = file_resp.body.to_lowercase();
                    if file_lower.contains(&pattern_lower) {
                        let excerpt = extract_excerpt(&file_resp.body, &pattern, 500);
                        matches.push(json!({
                            "entry_id": entry_id,
                            "role": role,
                            "entry_type": entry_type_str,
                            "excerpt": excerpt,
                            "source": "content_file",
                            "content_file_id": content_file_id,
                        }));
                    }
                }
            }
        }
    }

    Ok(json!({
        "matches": matches,
        "total": matches.len(),
        "entries_searched": tree.entry_ids().len(),
        "content_files_fetched": content_fetches,
    }))
}

/// Extract text content from a session entry's data field.
fn extract_entry_text(data: &Value) -> String {
    // Try "content" as string
    if let Some(s) = data.get("content").and_then(|v| v.as_str()) {
        return s.to_string();
    }
    // Try "content" as array of content blocks (Anthropic format)
    if let Some(arr) = data.get("content").and_then(|v| v.as_array()) {
        let parts: Vec<String> = arr
            .iter()
            .filter_map(|block| {
                if let Some(text) = block.get("text").and_then(|v| v.as_str()) {
                    Some(text.to_string())
                } else {
                    None
                }
            })
            .collect();
        if !parts.is_empty() {
            return parts.join("\n");
        }
    }
    // Try "summary" field (compaction entries)
    if let Some(s) = data.get("summary").and_then(|v| v.as_str()) {
        return s.to_string();
    }
    String::new()
}

/// Extract a truncated excerpt around the first match of pattern in text.
fn extract_excerpt(text: &str, pattern: &str, max_len: usize) -> String {
    let text_lower = text.to_lowercase();
    let pattern_lower = pattern.to_lowercase();
    if let Some(pos) = text_lower.find(&pattern_lower) {
        let start = pos.saturating_sub(max_len / 4);
        let end = (pos + pattern.len() + max_len * 3 / 4).min(text.len());
        // Align to char boundaries
        let start = text[..start]
            .char_indices()
            .last()
            .map(|(i, _)| i)
            .unwrap_or(0);
        let end = text[..end]
            .char_indices()
            .last()
            .map(|(i, c)| i + c.len_utf8())
            .unwrap_or(text.len())
            .min(text.len());
        let excerpt = &text[start..end];
        if start > 0 || end < text.len() {
            let mut result = String::new();
            if start > 0 {
                result.push_str("...");
            }
            result.push_str(excerpt);
            if end < text.len() {
                result.push_str("...");
            }
            result
        } else {
            excerpt.to_string()
        }
    } else {
        text[..text.len().min(max_len)].to_string()
    }
}

// ---------------------------------------------------------------------------
// grep — temper.grep(pattern, path, opts?)
// ---------------------------------------------------------------------------

pub fn grep(
    ctx: &Context,
    api_url: &str,
    tenant: &str,
    args: &[Value],
) -> Result<Value, String> {
    let pattern = pos_str(args, 0, "pattern", "grep")?;
    let path = pos_str(args, 1, "path", "grep")?;
    let opts = obj_arg_or_empty(args, 2);
    let ws_id = resolve_workspace_id(ctx, api_url, tenant, &opts, false)?;
    let eid = ctx_entity_id(ctx);
    let case_insensitive = opts
        .get("case_insensitive")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let max_results = opts
        .get("max_results")
        .and_then(|v| v.as_u64())
        .unwrap_or(50) as usize;

    let search_pattern = if case_insensitive {
        pattern.to_lowercase()
    } else {
        pattern.clone()
    };

    // Try resolving as a single file first
    let file_paths = resolve_grep_targets(ctx, api_url, tenant, eid, &ws_id, &path)?;

    let mut matches: Vec<Value> = Vec::new();

    for file_path in file_paths {
        if matches.len() >= max_results {
            break;
        }

        // Read file content
        let read_args = vec![json!(file_path), json!({"workspace_id": ws_id})];
        let content_val = match read(ctx, api_url, tenant, &read_args) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let content = content_val.as_str().unwrap_or("");

        for (line_num, line) in content.lines().enumerate() {
            if matches.len() >= max_results {
                break;
            }
            let haystack = if case_insensitive {
                line.to_lowercase()
            } else {
                line.to_string()
            };
            if haystack.contains(&search_pattern) {
                matches.push(json!({
                    "file": file_path,
                    "line_number": line_num + 1,
                    "line": line,
                }));
            }
        }
    }

    Ok(json!({ "matches": matches, "total": matches.len() }))
}

/// Resolve grep targets: if path is a file return vec![path], if dir return all files recursively.
fn resolve_grep_targets(
    ctx: &Context,
    api_url: &str,
    tenant: &str,
    principal_id: &str,
    ws_id: &str,
    path: &str,
) -> Result<Vec<String>, String> {
    // Try to resolve as a file first
    let resp = http_post(
        ctx,
        api_url,
        tenant,
        principal_id,
        &format!("/tdata/Workspaces('{ws_id}')/Temper.ResolvePath?await_integration=true"),
        &json!({"path": path}),
    );

    if let Ok(ref r) = resp {
        let file_id = r
            .get("fields")
            .and_then(|f| f.get("last_file_id"))
            .or_else(|| r.get("last_file_id"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if !file_id.is_empty() {
            return Ok(vec![path.to_string()]);
        }
    }

    // It's a directory — list recursively
    let mut files = Vec::new();
    list_dir_recursive(ctx, api_url, tenant, principal_id, ws_id, path, 0, 5, &mut files, 500)?;
    Ok(files)
}

// ---------------------------------------------------------------------------
// glob — temper.glob(pattern, path?)
// ---------------------------------------------------------------------------

pub fn glob_files(
    ctx: &Context,
    api_url: &str,
    tenant: &str,
    args: &[Value],
) -> Result<Value, String> {
    let pattern = pos_str(args, 0, "pattern", "glob")?;
    let path = args
        .get(1)
        .and_then(|v| v.as_str())
        .unwrap_or("/")
        .to_string();
    let opts = obj_arg_or_empty(args, 2);
    let ws_id = resolve_workspace_id(ctx, api_url, tenant, &opts, false)?;
    let eid = ctx_entity_id(ctx);

    let mut all_files = Vec::new();
    list_dir_recursive(ctx, api_url, tenant, eid, &ws_id, &path, 0, 5, &mut all_files, 500)?;

    let matches: Vec<&String> = all_files
        .iter()
        .filter(|f| glob_match(&pattern, f))
        .collect();

    Ok(json!(matches))
}

// ---------------------------------------------------------------------------
// list_dir_recursive — shared helper for grep/glob
// ---------------------------------------------------------------------------

fn list_dir_recursive(
    ctx: &Context,
    api_url: &str,
    tenant: &str,
    principal_id: &str,
    ws_id: &str,
    dir_path: &str,
    depth: usize,
    max_depth: usize,
    out: &mut Vec<String>,
    max_files: usize,
) -> Result<(), String> {
    if depth >= max_depth || out.len() >= max_files {
        return Ok(());
    }

    let resp = http_post(
        ctx,
        api_url,
        tenant,
        principal_id,
        &format!("/tdata/Workspaces('{ws_id}')/Temper.ListDir?await_integration=true"),
        &json!({"path": dir_path}),
    )?;

    let listing_str = resp
        .get("fields")
        .and_then(|f| f.get("last_listing"))
        .or_else(|| resp.get("last_listing"))
        .and_then(|v| v.as_str())
        .unwrap_or("[]");

    let listing: Vec<Value> = serde_json::from_str(listing_str).unwrap_or_default();

    for entry in &listing {
        if out.len() >= max_files {
            break;
        }
        let name = entry.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let kind = entry.get("type").and_then(|v| v.as_str()).unwrap_or("file");
        let full_path = if dir_path == "/" {
            format!("/{name}")
        } else {
            format!("{dir_path}/{name}")
        };

        if kind == "directory" || kind == "dir" {
            list_dir_recursive(ctx, api_url, tenant, principal_id, ws_id, &full_path, depth + 1, max_depth, out, max_files)?;
        } else {
            out.push(full_path);
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// glob_match — simple glob pattern matcher (no regex crate)
// ---------------------------------------------------------------------------

fn glob_match(pattern: &str, path: &str) -> bool {
    // Handle ** (match any path segments)
    if pattern.contains("**") {
        let parts: Vec<&str> = pattern.split("**").collect();
        if parts.len() == 2 {
            let prefix = parts[0].trim_end_matches('/');
            let suffix = parts[1].trim_start_matches('/');
            // Path must start with prefix (or prefix is empty)
            let path_match = if prefix.is_empty() {
                true
            } else {
                path.starts_with(prefix) || path.starts_with(&format!("{prefix}/"))
            };
            if !path_match {
                return false;
            }
            // Suffix must match the tail of the path
            if suffix.is_empty() {
                return true;
            }
            // Match suffix as a simple glob against each possible tail
            let check_path = if prefix.is_empty() { path } else { &path[prefix.len()..] };
            for (i, _) in check_path.char_indices() {
                let tail = &check_path[i..];
                if tail.starts_with('/') || i == 0 {
                    let tail = tail.trim_start_matches('/');
                    if simple_glob_match(suffix, tail) {
                        return true;
                    }
                }
            }
            return false;
        }
    }
    simple_glob_match(pattern, path)
}

/// Match a simple glob pattern (supports * and ?) against a string.
/// * matches any characters except /. ? matches exactly one character except /.
fn simple_glob_match(pattern: &str, text: &str) -> bool {
    let pat: Vec<char> = pattern.chars().collect();
    let txt: Vec<char> = text.chars().collect();
    simple_glob_match_recursive(&pat, 0, &txt, 0)
}

fn simple_glob_match_recursive(pattern: &[char], pi: usize, text: &[char], ti: usize) -> bool {
    if pi == pattern.len() && ti == text.len() {
        return true;
    }
    if pi == pattern.len() {
        return false;
    }

    match pattern[pi] {
        '*' => {
            // * matches zero or more characters (not /)
            // Try matching zero characters, then one, then two, etc.
            let mut t = ti;
            loop {
                if simple_glob_match_recursive(pattern, pi + 1, text, t) {
                    return true;
                }
                if t >= text.len() || text[t] == '/' {
                    return false;
                }
                t += 1;
            }
        }
        '?' => {
            if ti < text.len() && text[ti] != '/' {
                simple_glob_match_recursive(pattern, pi + 1, text, ti + 1)
            } else {
                false
            }
        }
        c => {
            if ti < text.len() && text[ti] == c {
                simple_glob_match_recursive(pattern, pi + 1, text, ti + 1)
            } else {
                false
            }
        }
    }
}

// ---------------------------------------------------------------------------
// run_coding_agent
// ---------------------------------------------------------------------------

pub fn run_coding_agent(
    ctx: &Context,
    _api_url: &str,
    _tenant: &str,
    sandbox_url: &str,
    workdir: &str,
    args: &[Value],
) -> Result<Value, String> {
    use wasm_helpers::sandbox::{self, SandboxHandle};

    let input = obj_arg(args, 0, "opts", "run_coding_agent")?;
    let agent_type = require_str(&input, "agent_type", "run_coding_agent")?;
    let task = require_str(&input, "task", "run_coding_agent")?;
    let agent_workdir = input
        .get("workdir")
        .and_then(|v| v.as_str())
        .unwrap_or(workdir);
    let background = input
        .get("background")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if sandbox_url.is_empty() {
        return Err("run_coding_agent: sandbox_url is empty".into());
    }

    // Build sandbox handle from entity state
    let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));
    let provider = fields
        .get("sandbox_provider")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .or_else(|| dispatch::peek_lazy_sandbox_provider())
        .unwrap_or_else(|| {
            sandbox::resolve_sandbox_provider(ctx, &fields)
                .unwrap_or_else(|_| "tensorlake".to_string())
        });
    let sandbox_id = fields
        .get("sandbox_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let handle = SandboxHandle {
        sandbox_url: sandbox_url.to_string(),
        sandbox_id,
        provider,
    };

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
        format!("nohup bash -c '{command}' > /tmp/coding-agent-{agent_type}.log 2>&1 & echo $!")
    } else {
        command.clone()
    };

    // Execute via sandbox provider abstraction
    let result = sandbox::sandbox_exec(ctx, &handle, &final_cmd, agent_workdir)?;

    if background {
        return Ok(json!({
            "agent_type": agent_type,
            "status": "started_background",
            "command": final_cmd,
        }));
    }

    let mut output = result.stdout;
    if !result.stderr.is_empty() {
        if !output.is_empty() {
            output.push('\n');
        }
        output.push_str(&result.stderr);
    }

    Ok(json!({
        "agent_type": agent_type,
        "output": output,
    }))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn require_str<'a>(input: &'a Value, key: &str, method: &str) -> Result<&'a str, String> {
    input
        .get(key)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("temper.{method}(): missing '{key}'"))
}

fn obj_arg(args: &[Value], idx: usize, name: &str, method: &str) -> Result<Value, String> {
    args.get(idx)
        .filter(|v| v.is_object())
        .cloned()
        .ok_or_else(|| {
            format!("temper.{method}(): missing object argument '{name}' at position {idx}")
        })
}

fn obj_arg_or_empty(args: &[Value], idx: usize) -> Value {
    args.get(idx)
        .filter(|v| v.is_object())
        .cloned()
        .unwrap_or(json!({}))
}

fn ctx_entity_id(ctx: &Context) -> &str {
    ctx.entity_state
        .get("entity_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
}

fn entity_field_str_val(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

/// Minimal headers for internal Temper API calls.
/// Auth headers (tenant, principal, agent-type, bearer token) are injected
/// by the WASM host for internal calls — see ADR-0043.
fn internal_headers() -> Vec<(String, String)> {
    vec![("Content-Type".to_string(), "application/json".to_string())]
}

fn http_get(
    ctx: &Context,
    api_url: &str,
    _tenant: &str,
    _principal_id: &str,
    path: &str,
) -> Result<Value, String> {
    let url = format!("{api_url}{path}");
    let headers = internal_headers();
    let resp = ctx.http_call("GET", &url, &headers, "")?;
    if let Some(denial) = dispatch::check_cedar_denial(resp.status, &resp.body) {
        return Err(denial);
    }
    if resp.status >= 400 {
        return Err(format!("HTTP GET {path}: {} {}", resp.status, resp.body));
    }
    serde_json::from_str(&resp.body)
        .map_err(|e| format!("failed to parse response from {path}: {e}"))
}

fn http_post(
    ctx: &Context,
    api_url: &str,
    _tenant: &str,
    _principal_id: &str,
    path: &str,
    body: &Value,
) -> Result<Value, String> {
    let url = format!("{api_url}{path}");
    let headers = internal_headers();
    let resp = ctx.http_call("POST", &url, &headers, &body.to_string())?;
    if let Some(denial) = dispatch::check_cedar_denial(resp.status, &resp.body) {
        return Err(denial);
    }
    if resp.status >= 400 {
        return Err(format!("HTTP POST {path}: {} {}", resp.status, resp.body));
    }
    if resp.body.is_empty() {
        return Ok(json!({"ok": true}));
    }
    serde_json::from_str(&resp.body)
        .map_err(|e| format!("failed to parse response from {path}: {e}"))
}

fn pos_str(args: &[Value], idx: usize, name: &str, method: &str) -> Result<String, String> {
    args.get(idx)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .ok_or_else(|| format!("temper.{method}(): missing '{name}' at position {idx}"))
}

fn find_workspace(
    ctx: &Context,
    api_url: &str,
    tenant: &str,
    name: &str,
) -> Result<Option<String>, String> {
    let name_enc = urlenc(name);
    let eid = ctx_entity_id(ctx);
    let resp = http_get(
        ctx,
        api_url,
        tenant,
        eid,
        &format!("/tdata/Workspaces?$filter=Name%20eq%20'{name_enc}'"),
    )?;
    let items = resp.get("value").and_then(|v| v.as_array());
    Ok(items
        .and_then(|arr| arr.first())
        .and_then(|v| {
            v.get("entity_id")
                .or_else(|| v.get("Id"))
                .and_then(|v| v.as_str())
        })
        .map(|s| s.to_string()))
}

fn session_workspace_id(ctx: &Context) -> Option<String> {
    ctx.entity_state
        .get("fields")
        .and_then(|fields| fields.get("workspace_id"))
        .and_then(|value| value.as_str())
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
}

fn resolve_workspace_id(
    ctx: &Context,
    api_url: &str,
    tenant: &str,
    opts: &Value,
    create_if_missing: bool,
) -> Result<String, String> {
    if let Some(workspace_id) = opts
        .get("workspace_id")
        .and_then(|value| value.as_str())
        .filter(|value| !value.is_empty())
    {
        return Ok(workspace_id.to_string());
    }

    if let Some(workspace_name) = opts
        .get("workspace")
        .and_then(|value| value.as_str())
        .filter(|value| !value.is_empty())
    {
        return if create_if_missing {
            ensure_workspace(ctx, api_url, tenant, workspace_name)
        } else {
            find_workspace(ctx, api_url, tenant, workspace_name)?.ok_or_else(|| {
                format!("workspace '{}' not found", workspace_name)
            })
        };
    }

    if let Some(workspace_id) = session_workspace_id(ctx) {
        return Ok(workspace_id);
    }

    if create_if_missing {
        ensure_workspace(ctx, api_url, tenant, "default")
    } else {
        find_workspace(ctx, api_url, tenant, "default")?
            .ok_or_else(|| "workspace 'default' not found".to_string())
    }
}

fn ensure_workspace(
    ctx: &Context,
    api_url: &str,
    tenant: &str,
    name: &str,
) -> Result<String, String> {
    if let Some(id) = find_workspace(ctx, api_url, tenant, name)? {
        return Ok(id);
    }
    let eid = ctx_entity_id(ctx);
    let resp = http_post(
        ctx,
        api_url,
        tenant,
        eid,
        "/tdata/Workspaces",
        &json!({"Name": name}),
    )?;
    resp.get("entity_id")
        .or_else(|| resp.get("Id"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "temper.write(): Workspace created but no Id returned".into())
}

fn mime_from_ext(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or("") {
        "md" | "markdown" => "text/markdown",
        "txt" => "text/plain",
        "json" => "application/json",
        "yaml" | "yml" => "application/yaml",
        "toml" => "application/toml",
        "html" | "htm" => "text/html",
        "css" => "text/css",
        "js" => "application/javascript",
        "ts" => "application/typescript",
        "rs" => "text/x-rust",
        "py" => "text/x-python",
        "xml" => "application/xml",
        "csv" => "text/csv",
        _ => "application/octet-stream",
    }
}

fn urlenc(s: &str) -> String {
    s.replace('%', "%25")
        .replace(' ', "%20")
        .replace('&', "%26")
        .replace('=', "%3D")
        .replace('?', "%3F")
        .replace('#', "%23")
        .replace('\'', "%27")
}
