//! Coding Agent Runner — WASM module for spawning coding agent CLI processes.
//!
//! Maps agent_type to CLI commands and executes them in the sandbox.
//! Supports claude-code, codex, pi, and opencode.

use temper_wasm_sdk::prelude::*;

#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        ctx.log("info", "coding_agent_runner: starting");

        let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));
        let sandbox_url = fields.get("sandbox_url").and_then(|v| v.as_str()).unwrap_or("");
        let workdir = fields.get("workdir").and_then(|v| v.as_str()).unwrap_or("/workspace");

        if sandbox_url.is_empty() {
            return Err("coding_agent_runner: sandbox_url is empty".to_string());
        }

        // Read tool input from trigger params
        let input = ctx.trigger_params.get("input").cloned().unwrap_or(json!({}));
        let agent_type = input.get("agent_type").and_then(|v| v.as_str()).unwrap_or("claude-code");
        let task = input.get("task").and_then(|v| v.as_str()).unwrap_or("");
        let task_workdir = input.get("workdir").and_then(|v| v.as_str()).unwrap_or(workdir);

        if task.is_empty() {
            return Err("coding_agent_runner: task is empty".to_string());
        }

        // Map agent_type to CLI command
        let command = match agent_type {
            "claude-code" => format!("claude --permission-mode bypassPermissions --print '{}'", escape_single_quotes(task)),
            "codex" => format!("codex exec '{}'", escape_single_quotes(task)),
            "pi" => format!("pi -p '{}'", escape_single_quotes(task)),
            "opencode" => format!("opencode run '{}'", escape_single_quotes(task)),
            other => return Err(format!("coding_agent_runner: unsupported agent_type: {other}")),
        };

        ctx.log("info", &format!("coding_agent_runner: running {agent_type}: {}", &command[..command.len().min(100)]));

        // Execute via Tensorlake sandbox data plane API using output-redirection polling.
        let output = run_bash_tensorlake(&ctx, sandbox_url, &command, task_workdir)?;

        // Return the output as a tool result
        set_success_result("HandleToolResults", &json!({
            "pending_tool_calls": json!([{
                "type": "tool_result",
                "tool_use_id": input.get("tool_use_id").and_then(|v| v.as_str()).unwrap_or("unknown"),
                "content": output,
            }]).to_string(),
        }));

        Ok(())
    })();

    if let Err(e) = result {
        set_error_result(&e);
    }
    0
}

fn escape_single_quotes(s: &str) -> String {
    s.replace('\'', "'\\''")
}

/// Run bash command via Tensorlake sandbox data plane with output-redirection polling.
fn run_bash_tensorlake(
    ctx: &Context,
    sandbox_url: &str,
    command: &str,
    workdir: &str,
) -> Result<String, String> {
    let api_key = ctx
        .config
        .get("tensorlake_api_key")
        .filter(|s| !s.is_empty() && !s.contains("{secret:"))
        .cloned()
        .unwrap_or_default();

    use core::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let run_id = format!("{:08x}", COUNTER.fetch_add(1, Ordering::Relaxed));

    let out_path = format!("/tmp/.paw-out-{run_id}");
    let err_path = format!("/tmp/.paw-err-{run_id}");
    let rc_path = format!("/tmp/.paw-rc-{run_id}");

    let wrapped = format!(
        "({command}) > {out_path} 2> {err_path}; echo $? > {rc_path}",
    );

    // Start process
    let start_url = format!("{sandbox_url}/api/v1/processes");
    let mut headers = vec![("content-type".to_string(), "application/json".to_string())];
    if !api_key.is_empty() {
        headers.push(("authorization".to_string(), format!("Bearer {api_key}")));
    }
    let body = json!({ "cmd": ["bash", "-c", &wrapped], "cwd": workdir });
    let resp = ctx.http_call("POST", &start_url, &headers, &body.to_string())?;
    if resp.status < 200 || resp.status >= 300 {
        return Err(format!("process start failed (HTTP {}): {}", resp.status, resp.body));
    }

    // Poll for exit code file
    let rc_url = format!("{sandbox_url}/api/v1/files?path={}", url_encode(&rc_path));
    let mut read_headers = Vec::new();
    if !api_key.is_empty() {
        read_headers.push(("authorization".to_string(), format!("Bearer {api_key}")));
    }
    let mut exit_code: i64 = -1;
    let mut found = false;
    for _ in 0..600 {
        if let Ok(r) = ctx.http_call("GET", &rc_url, &read_headers, "") {
            if r.status >= 200 && r.status < 300 {
                exit_code = r.body.trim().parse::<i64>().unwrap_or(-1);
                found = true;
                break;
            }
        }
    }

    if !found {
        return Err("process timed out after ~300s".to_string());
    }

    // Read output
    let out_url = format!("{sandbox_url}/api/v1/files?path={}", url_encode(&out_path));
    let err_url = format!("{sandbox_url}/api/v1/files?path={}", url_encode(&err_path));
    let stdout = ctx.http_call("GET", &out_url, &read_headers, "")
        .map(|r| r.body).unwrap_or_default();
    let stderr = ctx.http_call("GET", &err_url, &read_headers, "")
        .map(|r| r.body).unwrap_or_default();

    // Cleanup (best effort)
    for path in [&out_path, &err_path, &rc_path] {
        let del_url = format!("{sandbox_url}/api/v1/files?path={}", url_encode(path));
        let _ = ctx.http_call("DELETE", &del_url, &read_headers, "");
    }

    let mut output = String::new();
    if !stdout.is_empty() { output.push_str(&stdout); }
    if !stderr.is_empty() {
        if !output.is_empty() { output.push('\n'); }
        output.push_str("STDERR: ");
        output.push_str(&stderr);
    }
    if exit_code != 0 {
        output.push_str(&format!("\n(exit code: {exit_code})"));
    }
    Ok(output)
}

fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 2);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'/' => {
                out.push(b as char);
            }
            _ => {
                out.push_str(&format!("%{b:02X}"));
            }
        }
    }
    out
}
