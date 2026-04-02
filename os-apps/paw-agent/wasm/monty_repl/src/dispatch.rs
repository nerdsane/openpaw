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
        "temper" => dispatch_temper(ctx, temper_api_url, tenant, method, args),
        "sandbox" => dispatch_sandbox(ctx, sandbox_url, workdir, method, args),
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

        // Governance
        "get_decisions" => temper_get_decisions(ctx, api_url, tenant),
        "poll_decision" => temper_poll_decision(ctx, api_url, tenant, args),

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

        // Blocked
        "approve_decision" | "deny_decision" | "set_policy" => Err(format!(
            "temper.{method}() is not available to agents. \
             Governance writes require human approval via Observe UI."
        )),

        _ => Err(format!(
            "unknown temper method '{method}'. Available: \
             list, get, create, action, patch, submit_specs, show_spec, \
             upload_wasm, get_trajectories, get_insights, \
             get_decisions, poll_decision, install_app, list_apps, \
             get_agent_id"
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

// --- Apps ---

fn temper_install_app(ctx: &Context, api_url: &str, tenant: &str, args: &[Value]) -> Result<Value, String> {
    let app_name = str_arg(args, 0, "app_name", "install_app")?;
    let reason = opt_str_arg(args, 1).unwrap_or_default();
    let agent_id = ctx.entity_state.get("entity_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    // Create a CapabilityRequest entity (Cedar-governed)
    let body = json!({
        "CapabilityType": "os_app",
        "CapabilityName": app_name,
        "Reason": reason,
        "RequestingAgentId": agent_id,
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
    method: &str,
    args: &[Value],
) -> Result<Value, String> {
    if sandbox_url.is_empty() {
        return Err(format!("sandbox.{method}(): no sandbox attached"));
    }

    match method {
        "read" => sandbox_read(ctx, sandbox_url, args),
        "write" => sandbox_write(ctx, sandbox_url, args),
        "edit" => sandbox_edit(ctx, sandbox_url, args),
        "bash" => sandbox_bash(ctx, sandbox_url, workdir, args),
        _ => Err(format!("unknown sandbox method '{method}'. Available: read, write, edit, bash")),
    }
}

fn sandbox_read(ctx: &Context, sandbox_url: &str, args: &[Value]) -> Result<Value, String> {
    let path = str_arg(args, 0, "path", "read")?;
    let url = format!("{sandbox_url}/api/v1/files?path={}", urlenc(&path));
    let resp = ctx.http_call("GET", &url, &[], "")?;
    if resp.status >= 400 {
        return Err(format!("sandbox.read({path}): {}", resp.body));
    }
    Ok(json!(resp.body))
}

fn sandbox_write(ctx: &Context, sandbox_url: &str, args: &[Value]) -> Result<Value, String> {
    let path = str_arg(args, 0, "path", "write")?;
    let content = str_arg(args, 1, "content", "write")?;
    let url = format!("{sandbox_url}/api/v1/files?path={}", urlenc(&path));
    let resp = ctx.http_call("PUT", &url, &[], &content)?;
    if resp.status >= 400 {
        return Err(format!("sandbox.write({path}): {}", resp.body));
    }
    Ok(json!({"ok": true}))
}

fn sandbox_edit(ctx: &Context, sandbox_url: &str, args: &[Value]) -> Result<Value, String> {
    let path = str_arg(args, 0, "path", "edit")?;
    let old_string = str_arg(args, 1, "old_string", "edit")?;
    let new_string = str_arg(args, 2, "new_string", "edit")?;

    // Read current content
    let url = format!("{sandbox_url}/api/v1/files?path={}", urlenc(&path));
    let resp = ctx.http_call("GET", &url, &[], "")?;
    if resp.status >= 400 {
        return Err(format!("sandbox.edit({path}): read failed: {}", resp.body));
    }

    // Replace
    let content = resp.body;
    if !content.contains(&old_string) {
        return Err(format!("sandbox.edit({path}): old_string not found in file"));
    }
    let new_content = content.replacen(&old_string, &new_string, 1);

    // Write back
    let resp = ctx.http_call("PUT", &url, &[], &new_content)?;
    if resp.status >= 400 {
        return Err(format!("sandbox.edit({path}): write failed: {}", resp.body));
    }
    Ok(json!({"ok": true}))
}

fn sandbox_bash(ctx: &Context, sandbox_url: &str, workdir: &str, args: &[Value]) -> Result<Value, String> {
    let command = str_arg(args, 0, "command", "bash")?;

    // Use the same output-redirection pattern as tool_runner
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

    // Start process
    let body = json!({
        "command": ["bash", "-c", &wrapped],
        "cwd": workdir,
    });
    let resp = ctx.http_call(
        "POST",
        &format!("{sandbox_url}/api/v1/processes"),
        &[("Content-Type".to_string(), "application/json".to_string())],
        &body.to_string(),
    )?;
    if resp.status >= 400 {
        return Err(format!("sandbox.bash(): start failed: {}", resp.body));
    }

    // Poll for exit code
    let mut attempts = 0;
    loop {
        let rc_resp = ctx.http_call("GET", &format!("{sandbox_url}/api/v1/files?path={}", urlenc(&rc_file)), &[], "")?;
        if rc_resp.status < 400 && !rc_resp.body.trim().is_empty() {
            break;
        }
        attempts += 1;
        if attempts > 600 {
            return Err("sandbox.bash(): timed out waiting for exit code".into());
        }
        // Small busy-wait (WASM doesn't have sleep)
    }

    // Read outputs
    let stdout = ctx.http_call("GET", &format!("{sandbox_url}/api/v1/files?path={}", urlenc(&out_file)), &[], "")
        .map(|r| r.body).unwrap_or_default();
    let stderr = ctx.http_call("GET", &format!("{sandbox_url}/api/v1/files?path={}", urlenc(&err_file)), &[], "")
        .map(|r| r.body).unwrap_or_default();
    let exit_code = ctx.http_call("GET", &format!("{sandbox_url}/api/v1/files?path={}", urlenc(&rc_file)), &[], "")
        .map(|r| r.body.trim().to_string()).unwrap_or_default();

    // Cleanup
    for f in [&out_file, &err_file, &rc_file] {
        let _ = ctx.http_call("DELETE", &format!("{sandbox_url}/api/v1/files?path={}", urlenc(f)), &[], "");
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
