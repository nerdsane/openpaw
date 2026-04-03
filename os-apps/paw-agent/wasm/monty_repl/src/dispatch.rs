//! Dispatch `temper.*` and `sandbox.*` method calls to HTTP endpoints.
//!
//! When Monty code calls `temper.create("Issues", {...})`, the Monty
//! interpreter pauses and yields a `FunctionCall`. This module handles
//! that call by making an HTTP request via the WASM host function
//! `ctx.http_call()` and returning the result as JSON.
//!
//! Mirrors the method signatures from `temper-sandbox/src/dispatch.rs`
//! so agents see the exact same Python interface as `mcp__temper__execute`.

use serde_json::{Value, json};
use temper_wasm_sdk::context::Context;

/// Dispatch a `temper.<method>()` or `sandbox.<method>()` call.
///
/// Called by the Monty event loop when user code invokes a method on a
/// dataclass object. `obj_name` is `"temper"` or `"sandbox"`, `method`
/// is the method name, and `args` are the JSON-converted positional args.
pub fn dispatch(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    sandbox_url: &str,
    workdir: &str,
    obj_name: &str,
    method: &str,
    args: &[Value],
) -> Result<Value, String> {
    match obj_name {
        "temper" => dispatch_temper(ctx, temper_api_url, tenant, sandbox_url, workdir, method, args),
        "sandbox" => {
            let sandbox_api_key = ctx.config.get("tensorlake_api_key").cloned().unwrap_or_default();
            dispatch_sandbox(ctx, sandbox_url, workdir, &sandbox_api_key, method, args)
        }
        _ => Err(format!("unknown object: {obj_name}")),
    }
}

// ---------------------------------------------------------------------------
// Temper dispatch
// ---------------------------------------------------------------------------

fn dispatch_temper(
    ctx: &Context,
    api_url: &str,
    tenant: &str,
    sandbox_url: &str,
    workdir: &str,
    method: &str,
    args: &[Value],
) -> Result<Value, String> {
    match method {
        // Entity CRUD
        "list" => temper_list(ctx, api_url, tenant, args),
        "get" => temper_get(ctx, api_url, tenant, args),
        "create" => temper_create(ctx, api_url, tenant, args),
        "action" => temper_action(ctx, api_url, tenant, args),
        "patch" => temper_patch(ctx, api_url, tenant, args),

        // Specs
        "submit_specs" => temper_submit_specs(ctx, api_url, tenant, args),
        "show_spec" | "spec_detail" => temper_show_spec(ctx, api_url, tenant, args),
        "specs" => temper_specs(ctx, api_url, tenant),

        // WASM
        "upload_wasm" => temper_upload_wasm(ctx, api_url, tenant, args),

        // Evolution
        "get_trajectories" => temper_get_trajectories(ctx, api_url, tenant, args),
        "get_insights" => temper_get_insights(ctx, api_url, tenant),

        // Governance — decisions
        "get_decisions" => temper_get_decisions(ctx, api_url, tenant),
        "poll_decision" => temper_poll_decision(ctx, api_url, tenant, args),
        "approve_decision" => temper_approve_decision(ctx, api_url, tenant, args),
        "deny_decision" => temper_deny_decision(ctx, api_url, tenant, args),

        // Governance — Cedar policies (all Cedar-gated by platform)
        "submit_policy" => temper_submit_policy(ctx, api_url, tenant, args),
        "list_policies" => temper_list_policies(ctx, api_url, tenant),
        "get_policy" => temper_get_policy(ctx, api_url, tenant, args),
        "update_policy" => temper_update_policy(ctx, api_url, tenant, args),
        "delete_policy" => temper_delete_policy(ctx, api_url, tenant, args),

        // Apps
        "install_app" => temper_install_app(ctx, api_url, tenant, args),
        "list_apps" => temper_list_apps(ctx, api_url, tenant),

        // Agent identity
        "get_agent_id" => {
            let agent_id = ctx.entity_state.get("entity_id")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            Ok(json!(agent_id))
        }

        // Entity operations (ported from tool_runner)
        "spawn_session" => super::entity_ops::spawn_session(ctx, api_url, tenant, sandbox_url, workdir, args),
        "list_sessions" => super::entity_ops::list_sessions(ctx, api_url, tenant, args),
        "abort_session" => super::entity_ops::abort_session(ctx, api_url, tenant, args),
        "steer_session" => super::entity_ops::steer_session(ctx, api_url, tenant, args),
        "save_memory" => super::entity_ops::save_memory(ctx, api_url, tenant, args),
        "recall_memory" => super::entity_ops::recall_memory(ctx, api_url, tenant, args),
        "file_upload" => super::entity_ops::file_upload(ctx, api_url, tenant, args),
        "read_entity" => super::entity_ops::read_entity(ctx, api_url, tenant, args),
        "run_coding_agent" => super::entity_ops::run_coding_agent(ctx, api_url, tenant, sandbox_url, workdir, args),

        // Secrets (Cedar-gated via access_secret on Secret resource)
        "get_secret" => temper_get_secret(ctx, args),

        // External service integrations (ported from tool_runner)
        "datadog_query" => super::datadog::datadog_query(ctx, args),
        "railway" => super::railway::railway(ctx, args),
        "vercel" => super::vercel::vercel(ctx, args),

        _ => Err(format!(
            "unknown temper method '{method}'. Available: \
             list, get, create, action, patch, submit_specs, show_spec, \
             upload_wasm, get_trajectories, get_insights, \
             get_decisions, poll_decision, approve_decision, deny_decision, \
             submit_policy, list_policies, get_policy, update_policy, delete_policy, \
             get_secret, install_app, list_apps, get_agent_id, \
             spawn_session, list_sessions, abort_session, steer_session, \
             save_memory, recall_memory, file_upload, read_entity, \
             run_coding_agent, datadog_query, railway, vercel"
        )),
    }
}

// --- Entity CRUD ---

fn temper_list(ctx: &Context, api_url: &str, tenant: &str, args: &[Value]) -> Result<Value, String> {
    let entity_set = str_arg(args, 0, "entity_set", "list")?;
    let filter = opt_str_arg(args, 1);
    let path = match filter {
        Some(f) => {
            let encoded = f.replace(' ', "%20").replace('\'', "%27");
            format!("/tdata/{entity_set}?$filter={encoded}")
        }
        None => format!("/tdata/{entity_set}"),
    };
    let resp = http_get(ctx, api_url, tenant, &path)?;
    Ok(resp.get("value").cloned().unwrap_or(resp))
}

fn temper_get(ctx: &Context, api_url: &str, tenant: &str, args: &[Value]) -> Result<Value, String> {
    let entity_set = str_arg(args, 0, "entity_set", "get")?;
    let entity_id = str_arg(args, 1, "entity_id", "get")?;
    let key = escape_odata_key(&entity_id);
    http_get(ctx, api_url, tenant, &format!("/tdata/{entity_set}('{key}')"))
}

fn temper_create(ctx: &Context, api_url: &str, tenant: &str, args: &[Value]) -> Result<Value, String> {
    let entity_set = str_arg(args, 0, "entity_set", "create")?;
    let body = obj_arg(args, 1, "fields", "create")?;
    http_post(ctx, api_url, tenant, &format!("/tdata/{entity_set}"), &body)
}

fn temper_action(ctx: &Context, api_url: &str, tenant: &str, args: &[Value]) -> Result<Value, String> {
    let entity_set = str_arg(args, 0, "entity_set", "action")?;
    let entity_id = str_arg(args, 1, "entity_id", "action")?;
    let action_name = str_arg(args, 2, "action_name", "action")?;
    let body = obj_arg_or_empty(args, 3);
    let key = escape_odata_key(&entity_id);
    http_post(ctx, api_url, tenant, &format!("/tdata/{entity_set}('{key}')/Temper.{action_name}"), &body)
}

fn temper_patch(ctx: &Context, api_url: &str, tenant: &str, args: &[Value]) -> Result<Value, String> {
    let entity_set = str_arg(args, 0, "entity_set", "patch")?;
    let entity_id = str_arg(args, 1, "entity_id", "patch")?;
    let body = obj_arg(args, 2, "fields", "patch")?;
    let key = escape_odata_key(&entity_id);
    http_patch(ctx, api_url, tenant, &format!("/tdata/{entity_set}('{key}')"), &body)
}

// --- Specs ---

fn temper_submit_specs(ctx: &Context, api_url: &str, tenant: &str, args: &[Value]) -> Result<Value, String> {
    let specs = obj_arg(args, 0, "specs", "submit_specs")?;
    let body = json!({ "tenant": tenant, "specs": specs });
    http_post(ctx, api_url, tenant, "/api/specs/load-inline", &body)
}

fn temper_show_spec(ctx: &Context, api_url: &str, tenant: &str, args: &[Value]) -> Result<Value, String> {
    let entity_type = str_arg(args, 0, "entity_type", "show_spec")?;
    http_get(ctx, api_url, tenant, &format!("/observe/specs/{entity_type}"))
}

fn temper_specs(ctx: &Context, api_url: &str, tenant: &str) -> Result<Value, String> {
    http_get(ctx, api_url, tenant, "/observe/specs")
}

// --- WASM ---

fn temper_upload_wasm(ctx: &Context, api_url: &str, tenant: &str, args: &[Value]) -> Result<Value, String> {
    let module_name = str_arg(args, 0, "module_name", "upload_wasm")?;
    let wasm_base64 = str_arg(args, 1, "wasm_base64", "upload_wasm")?;
    let body = json!({ "wasm_base64": wasm_base64 });
    http_post(ctx, api_url, tenant, &format!("/api/wasm/modules/{module_name}"), &body)
}

// --- Evolution ---

fn temper_get_trajectories(ctx: &Context, api_url: &str, tenant: &str, args: &[Value]) -> Result<Value, String> {
    let entity_type = opt_str_arg(args, 0);
    let failed_only = args.get(1).and_then(|v| v.as_bool()).unwrap_or(false);
    let limit = args.get(2).and_then(|v| v.as_u64()).unwrap_or(50);
    let mut path = format!("/api/evolution/trajectories?limit={limit}");
    if let Some(et) = entity_type {
        path.push_str(&format!("&entity_type={et}"));
    }
    if failed_only {
        path.push_str("&failed_only=true");
    }
    http_get(ctx, api_url, tenant, &path)
}

fn temper_get_insights(ctx: &Context, api_url: &str, tenant: &str) -> Result<Value, String> {
    http_get(ctx, api_url, tenant, "/api/evolution/insights")
}

// --- Governance ---

fn temper_get_decisions(ctx: &Context, api_url: &str, tenant: &str) -> Result<Value, String> {
    http_get(ctx, api_url, tenant, "/api/decisions")
}

fn temper_poll_decision(ctx: &Context, api_url: &str, tenant: &str, args: &[Value]) -> Result<Value, String> {
    let decision_id = str_arg(args, 0, "decision_id", "poll_decision")?;
    // Poll once — the agent can call repeatedly if needed.
    // Full blocking poll would exceed WASM execution budget.
    http_get(ctx, api_url, tenant, &format!("/api/decisions/{decision_id}"))
}

// --- Secrets (Cedar-gated via access_secret on Secret resource) ---

fn temper_get_secret(ctx: &Context, args: &[Value]) -> Result<Value, String> {
    let key = str_arg(args, 0, "key", "get_secret")?;
    let value = ctx.get_secret(&key)?;
    Ok(json!(value))
}

// --- Cedar Policy Management (all Cedar-gated by platform) ---

fn temper_submit_policy(ctx: &Context, api_url: &str, tenant: &str, args: &[Value]) -> Result<Value, String> {
    let policy_id = str_arg(args, 0, "policy_id", "submit_policy")?;
    let cedar_text = str_arg(args, 1, "cedar_text", "submit_policy")?;
    let body = json!({ "policy_id": policy_id, "cedar_text": cedar_text });
    http_post(ctx, api_url, tenant, &format!("/api/tenants/{tenant}/policies/create"), &body)
}

fn temper_list_policies(ctx: &Context, api_url: &str, tenant: &str) -> Result<Value, String> {
    http_get(ctx, api_url, tenant, &format!("/api/tenants/{tenant}/policies/list"))
}

fn temper_get_policy(ctx: &Context, api_url: &str, tenant: &str, args: &[Value]) -> Result<Value, String> {
    let policy_id = str_arg(args, 0, "policy_id", "get_policy")?;
    // List all and filter — no single-policy GET endpoint
    let all = http_get(ctx, api_url, tenant, &format!("/api/tenants/{tenant}/policies/list"))?;
    let policies = all.get("policies").and_then(|v| v.as_array());
    if let Some(list) = policies {
        for p in list {
            if p.get("policy_id").and_then(|v| v.as_str()) == Some(&policy_id) {
                return Ok(p.clone());
            }
        }
    }
    Err(format!("policy '{policy_id}' not found"))
}

fn temper_update_policy(ctx: &Context, api_url: &str, tenant: &str, args: &[Value]) -> Result<Value, String> {
    let policy_id = str_arg(args, 0, "policy_id", "update_policy")?;
    let cedar_text = str_arg(args, 1, "cedar_text", "update_policy")?;
    let body = json!({ "cedar_text": cedar_text });
    http_patch(ctx, api_url, tenant, &format!("/api/tenants/{tenant}/policies/entry/{policy_id}"), &body)
}

fn temper_delete_policy(ctx: &Context, api_url: &str, tenant: &str, args: &[Value]) -> Result<Value, String> {
    let policy_id = str_arg(args, 0, "policy_id", "delete_policy")?;
    http_delete(ctx, api_url, tenant, &format!("/api/tenants/{tenant}/policies/entry/{policy_id}"))
}

// --- Decision Management (Cedar-gated by platform) ---

fn temper_approve_decision(ctx: &Context, api_url: &str, tenant: &str, args: &[Value]) -> Result<Value, String> {
    let decision_id = str_arg(args, 0, "decision_id", "approve_decision")?;
    let scope = obj_arg(args, 1, "scope", "approve_decision")?;
    let agent_id = ctx.entity_state.get("entity_id")
        .and_then(|v| v.as_str())
        .unwrap_or("agent");
    let body = json!({ "scope": scope, "decided_by": format!("agent:{agent_id}") });
    http_post(ctx, api_url, tenant, &format!("/api/tenants/{tenant}/decisions/{decision_id}/approve"), &body)
}

fn temper_deny_decision(ctx: &Context, api_url: &str, tenant: &str, args: &[Value]) -> Result<Value, String> {
    let decision_id = str_arg(args, 0, "decision_id", "deny_decision")?;
    let agent_id = ctx.entity_state.get("entity_id")
        .and_then(|v| v.as_str())
        .unwrap_or("agent");
    let body = json!({ "decided_by": format!("agent:{agent_id}") });
    http_post(ctx, api_url, tenant, &format!("/api/tenants/{tenant}/decisions/{decision_id}/deny"), &body)
}

// --- Apps ---

fn temper_install_app(ctx: &Context, api_url: &str, tenant: &str, args: &[Value]) -> Result<Value, String> {
    let app_name = str_arg(args, 0, "app_name", "install_app")?;
    let reason = opt_str_arg(args, 1).unwrap_or_default();
    let payload = opt_str_arg(args, 2).unwrap_or_default();
    let cap_type = opt_str_arg(args, 3).unwrap_or_else(|| "os_app".to_string());
    let agent_id = ctx.entity_state.get("entity_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    // Create a CapabilityRequest entity (Cedar-governed).
    // Use PascalCase keys to match CSDL property names (Temper maps these to IOA variables).
    let body = json!({
        "CapabilityType": cap_type,
        "CapabilityName": app_name,
        "Reason": reason,
        "RequestingAgentId": agent_id,
        "Payload": payload,
    });
    http_post(ctx, api_url, tenant, "/tdata/CapabilityRequests", &body)
}

fn temper_list_apps(ctx: &Context, api_url: &str, tenant: &str) -> Result<Value, String> {
    http_get(ctx, api_url, tenant, "/api/apps")
}

// ---------------------------------------------------------------------------
// Sandbox dispatch
// ---------------------------------------------------------------------------

fn dispatch_sandbox(
    ctx: &Context,
    sandbox_url: &str,
    workdir: &str,
    sandbox_api_key: &str,
    method: &str,
    args: &[Value],
) -> Result<Value, String> {
    if sandbox_url.is_empty() {
        return Err(format!("sandbox.{method}(): no sandbox attached"));
    }

    match method {
        "read" => sandbox_read(ctx, sandbox_url, sandbox_api_key, args),
        "write" => sandbox_write(ctx, sandbox_url, sandbox_api_key, args),
        "edit" => sandbox_edit(ctx, sandbox_url, sandbox_api_key, args),
        "bash" => sandbox_bash(ctx, sandbox_url, sandbox_api_key, workdir, args),
        _ => Err(format!("unknown sandbox method '{method}'. Available: read, write, edit, bash")),
    }
}

fn sandbox_headers(api_key: &str) -> Vec<(String, String)> {
    if api_key.is_empty() {
        vec![]
    } else {
        vec![("Authorization".to_string(), format!("Bearer {api_key}"))]
    }
}

fn sandbox_headers_json(api_key: &str) -> Vec<(String, String)> {
    let mut h = sandbox_headers(api_key);
    h.push(("Content-Type".to_string(), "application/json".to_string()));
    h
}

fn sandbox_read(ctx: &Context, sandbox_url: &str, api_key: &str, args: &[Value]) -> Result<Value, String> {
    let path = str_arg(args, 0, "path", "read")?;
    let url = format!("{sandbox_url}/api/v1/files?path={}", urlenc(&path));
    let resp = ctx.http_call("GET", &url, &sandbox_headers(api_key), "")?;
    if resp.status >= 400 {
        return Err(format!("sandbox.read({path}): {}", resp.body));
    }
    Ok(json!(resp.body))
}

fn sandbox_write(ctx: &Context, sandbox_url: &str, api_key: &str, args: &[Value]) -> Result<Value, String> {
    let path = str_arg(args, 0, "path", "write")?;
    let content = str_arg(args, 1, "content", "write")?;
    let url = format!("{sandbox_url}/api/v1/files?path={}", urlenc(&path));
    let resp = ctx.http_call("PUT", &url, &sandbox_headers(api_key), &content)?;
    if resp.status >= 400 {
        return Err(format!("sandbox.write({path}): {}", resp.body));
    }
    Ok(json!({"ok": true}))
}

fn sandbox_edit(ctx: &Context, sandbox_url: &str, api_key: &str, args: &[Value]) -> Result<Value, String> {
    let path = str_arg(args, 0, "path", "edit")?;
    let old_string = str_arg(args, 1, "old_string", "edit")?;
    let new_string = str_arg(args, 2, "new_string", "edit")?;

    let headers = sandbox_headers(api_key);
    let url = format!("{sandbox_url}/api/v1/files?path={}", urlenc(&path));
    let resp = ctx.http_call("GET", &url, &headers, "")?;
    if resp.status >= 400 {
        return Err(format!("sandbox.edit({path}): read failed: {}", resp.body));
    }

    let content = resp.body;
    if !content.contains(&old_string) {
        return Err(format!("sandbox.edit({path}): old_string not found in file"));
    }
    let new_content = content.replacen(&old_string, &new_string, 1);

    let resp = ctx.http_call("PUT", &url, &headers, &new_content)?;
    if resp.status >= 400 {
        return Err(format!("sandbox.edit({path}): write failed: {}", resp.body));
    }
    Ok(json!({"ok": true}))
}

fn sandbox_bash(ctx: &Context, sandbox_url: &str, api_key: &str, workdir: &str, args: &[Value]) -> Result<Value, String> {
    let command = str_arg(args, 0, "command", "bash")?;
    let headers = sandbox_headers(api_key);
    let headers_json = sandbox_headers_json(api_key);

    let unique = format!("{:x}", std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis());
    let out_file = format!("/tmp/.paw-out-{unique}");
    let err_file = format!("/tmp/.paw-err-{unique}");
    let rc_file = format!("/tmp/.paw-rc-{unique}");

    let wrapped = format!(
        "({command}) > {out_file} 2> {err_file}; echo $? > {rc_file}"
    );

    // Tensorlake API: command is binary path, args is separate array
    let body = json!({
        "command": "/bin/bash",
        "args": ["-c", &wrapped],
    });
    let resp = ctx.http_call(
        "POST",
        &format!("{sandbox_url}/api/v1/processes"),
        &headers_json,
        &body.to_string(),
    )?;
    if resp.status >= 400 {
        return Err(format!("sandbox.bash(): start failed: {}", resp.body));
    }

    // Process started — poll for output files to know when it's done.
    let stdout = ctx.http_call("GET", &format!("{sandbox_url}/api/v1/files?path={}", urlenc(&out_file)), &headers, "")
        .map(|r| r.body).unwrap_or_default();
    let stderr = ctx.http_call("GET", &format!("{sandbox_url}/api/v1/files?path={}", urlenc(&err_file)), &headers, "")
        .map(|r| r.body).unwrap_or_default();
    let exit_code = ctx.http_call("GET", &format!("{sandbox_url}/api/v1/files?path={}", urlenc(&rc_file)), &headers, "")
        .map(|r| r.body.trim().to_string()).unwrap_or_default();

    for f in [&out_file, &err_file, &rc_file] {
        let _ = ctx.http_call("DELETE", &format!("{sandbox_url}/api/v1/files?path={}", urlenc(f)), &headers, "");
    }

    let mut output = String::new();
    if !stdout.is_empty() {
        output.push_str(&stdout);
    }
    if !stderr.is_empty() {
        if !output.is_empty() { output.push('\n'); }
        output.push_str("STDERR: ");
        output.push_str(&stderr);
    }
    output.push_str(&format!("\n[exit code: {exit_code}]"));

    Ok(json!(output))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn str_arg(args: &[Value], idx: usize, name: &str, method: &str) -> Result<String, String> {
    args.get(idx)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| format!("temper.{method}(): missing argument '{name}' at position {idx}"))
}

fn opt_str_arg(args: &[Value], idx: usize) -> Option<String> {
    args.get(idx).and_then(|v| v.as_str()).map(|s| s.to_string())
}

fn obj_arg(args: &[Value], idx: usize, name: &str, method: &str) -> Result<Value, String> {
    args.get(idx)
        .filter(|v| v.is_object())
        .cloned()
        .ok_or_else(|| format!("temper.{method}(): missing object argument '{name}' at position {idx}"))
}

fn obj_arg_or_empty(args: &[Value], idx: usize) -> Value {
    args.get(idx)
        .filter(|v| v.is_object())
        .cloned()
        .unwrap_or(json!({}))
}

fn escape_odata_key(key: &str) -> String {
    key.replace('\'', "''")
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

fn runtime_headers(tenant: &str) -> Vec<(String, String)> {
    vec![
        ("Content-Type".to_string(), "application/json".to_string()),
        ("X-Tenant-Id".to_string(), tenant.to_string()),
    ]
}

fn http_get(ctx: &Context, api_url: &str, tenant: &str, path: &str) -> Result<Value, String> {
    let url = format!("{api_url}{path}");
    let headers = runtime_headers(tenant);
    let resp = ctx.http_call("GET", &url, &headers, "")?;
    if resp.status >= 400 {
        return Err(format!("HTTP GET {path}: {} {}", resp.status, resp.body));
    }
    serde_json::from_str(&resp.body)
        .map_err(|e| format!("failed to parse response from {path}: {e}"))
}

fn http_post(ctx: &Context, api_url: &str, tenant: &str, path: &str, body: &Value) -> Result<Value, String> {
    let url = format!("{api_url}{path}");
    let headers = runtime_headers(tenant);
    let resp = ctx.http_call("POST", &url, &headers, &body.to_string())?;
    if resp.status >= 400 {
        return Err(format!("HTTP POST {path}: {} {}", resp.status, resp.body));
    }
    if resp.body.is_empty() {
        return Ok(json!({"ok": true}));
    }
    serde_json::from_str(&resp.body)
        .map_err(|e| format!("failed to parse response from {path}: {e}"))
}

fn http_patch(ctx: &Context, api_url: &str, tenant: &str, path: &str, body: &Value) -> Result<Value, String> {
    let url = format!("{api_url}{path}");
    let headers = runtime_headers(tenant);
    let resp = ctx.http_call("PATCH", &url, &headers, &body.to_string())?;
    if resp.status >= 400 {
        return Err(format!("HTTP PATCH {path}: {} {}", resp.status, resp.body));
    }
    if resp.body.is_empty() {
        return Ok(json!({"ok": true}));
    }
    serde_json::from_str(&resp.body)
        .map_err(|e| format!("failed to parse response from {path}: {e}"))
}

fn http_delete(ctx: &Context, api_url: &str, tenant: &str, path: &str) -> Result<Value, String> {
    let url = format!("{api_url}{path}");
    let headers = runtime_headers(tenant);
    let resp = ctx.http_call("DELETE", &url, &headers, "")?;
    if resp.status >= 400 {
        return Err(format!("HTTP DELETE {path}: {} {}", resp.status, resp.body));
    }
    if resp.body.is_empty() {
        return Ok(json!({"ok": true}));
    }
    serde_json::from_str(&resp.body)
        .map_err(|e| format!("failed to parse response from {path}: {e}"))
}
