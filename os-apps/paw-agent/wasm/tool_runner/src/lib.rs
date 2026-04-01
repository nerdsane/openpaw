//! Tool Runner — WASM module for executing tool calls in a sandbox.
//!
//! Reads pending_tool_calls from trigger params, executes each tool via
//! HTTP calls to the sandbox API, and returns tool results as callback params.
//!
//! Build: `cargo build --target wasm32-unknown-unknown --release`

use std::collections::BTreeMap;
use temper_wasm_sdk::prelude::*;
use wasm_helpers::{create_content_file, runtime_headers};

mod datadog;
mod entity_tools;
mod railway;
mod vercel;

const MAX_TOOL_RESULT_BYTES: usize = 16 * 1024;

/// Entry point.
#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        ctx.log("info", "tool_runner: starting");

        let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));

        let sandbox_url = fields
            .get("sandbox_url")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let workdir = fields
            .get("workdir")
            .and_then(|v| v.as_str())
            .unwrap_or("/workspace");

        // Temper API URL: read from integration config, default to localhost
        let temper_api_url = ctx
            .config
            .get("temper_api_url")
            .cloned()
            .unwrap_or_else(|| "http://127.0.0.1:3000".to_string());
        let tenant = &ctx.tenant;
        let hook_policy = fields
            .get("hook_policy")
            .and_then(|v| v.as_str())
            .unwrap_or("none");
        let soul_id = fields.get("soul_id").and_then(|v| v.as_str()).unwrap_or("");
        let project_harness_id = fields
            .get("project_harness_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let _ = send_heartbeat(&ctx, &temper_api_url, tenant);

        // Read pending tool calls from trigger params
        let tool_calls_json = ctx
            .trigger_params
            .get("pending_tool_calls")
            .and_then(|v| v.as_str())
            .unwrap_or("[]");

        let tool_calls: Vec<Value> = serde_json::from_str(tool_calls_json)
            .map_err(|e| format!("failed to parse pending_tool_calls: {e}"))?;

        ctx.log(
            "info",
            &format!("tool_runner: executing {} tool calls", tool_calls.len()),
        );

        // Execute each tool call and collect results
        let mut tool_results: Vec<Value> = Vec::new();

        for call in &tool_calls {
            let tool_id = call.get("id").and_then(|v| v.as_str()).unwrap_or("unknown");
            let tool_name = call
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let input = call.get("input").cloned().unwrap_or(json!({}));

            ctx.log(
                "info",
                &format!("tool_runner: executing tool '{tool_name}' id={tool_id}"),
            );
            emit_progress_ignore(
                &ctx,
                json!({
                    "kind": "tool_execution_start",
                    "message": format!("executing tool {tool_name}"),
                    "tool_call_id": tool_id,
                    "tool_name": tool_name,
                }),
            );

            let result = if let Err(error) = validate_tool_input(tool_name, &input) {
                Err(error)
            } else if let Some(error) = evaluate_before_hooks(
                &ctx,
                &temper_api_url,
                tenant,
                soul_id,
                hook_policy,
                tool_name,
                &input,
                project_harness_id,
            )? {
                Err(error)
            } else if entity_tools::is_entity_tool(tool_name) {
                entity_tools::execute(&ctx, &temper_api_url, tenant, &fields, tool_name, &input)
            } else if sandbox_url.is_empty() {
                Err(format!(
                    "sandbox_url is empty — cannot execute sandbox tool '{tool_name}'"
                ))
            } else {
                execute_tool(&ctx, sandbox_url, workdir, tool_name, &input)
            };

            let (content, is_error) = match result {
                Ok(output) => (
                    apply_after_hooks(
                        &ctx,
                        &temper_api_url,
                        tenant,
                        soul_id,
                        hook_policy,
                        tool_name,
                        output,
                    )?,
                    false,
                ),
                Err(e) => (format!("Error: {e}"), true),
            };
            let content = truncate_tool_result(&content);
            let _ = send_heartbeat(&ctx, &temper_api_url, tenant);
            emit_progress_ignore(
                &ctx,
                json!({
                    "kind": "tool_execution_complete",
                    "message": format!("completed tool {tool_name}"),
                    "tool_call_id": tool_id,
                    "tool_name": tool_name,
                    "is_error": is_error,
                }),
            );

            tool_results.push(json!({
                "type": "tool_result",
                "tool_use_id": tool_id,
                "content": content,
                "is_error": is_error,
            }));
        }

        // Session tree and conversation storage
        let conversation_file_id = fields
            .get("conversation_file_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let session_file_id = fields
            .get("session_file_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let session_leaf_id = fields
            .get("session_leaf_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let workspace_id = fields
            .get("workspace_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let results_json = serde_json::to_string(&tool_results).unwrap_or_default();
        let mut params = json!({
            "pending_tool_calls": results_json,
        });

        if !session_file_id.is_empty() && !session_leaf_id.is_empty() {
            // Session tree mode: append tool results
            let session_jsonl =
                read_session_from_temperfs(&ctx, &temper_api_url, tenant, session_file_id)?;
            let mut tree = session_tree_lib::SessionTree::from_jsonl(&session_jsonl);
            let tool_results_value = json!(tool_results.clone());
            let tokens_est = results_json.len() / 4;
            let content_str = serde_json::to_string(&tool_results_value).unwrap_or_default();
            let (new_leaf, _) = if !workspace_id.is_empty() {
                match create_tool_content_file(
                    &ctx,
                    &temper_api_url,
                    tenant,
                    workspace_id,
                    &format!("t-{}", tree.len()),
                    &content_str,
                ) {
                    Ok(content_file_id) => {
                        tree.append_tool_results_file(session_leaf_id, &content_file_id, tokens_est)
                    }
                    Err(_) => tree.append_tool_results(session_leaf_id, &tool_results_value, tokens_est),
                }
            } else {
                tree.append_tool_results(session_leaf_id, &tool_results_value, tokens_est)
            };
            let updated_jsonl = tree.to_jsonl();
            write_session_to_temperfs(
                &ctx,
                &temper_api_url,
                tenant,
                session_file_id,
                &updated_jsonl,
            )?;

            params["pending_tool_calls"] = json!(compact_tool_results_marker(&tool_results));
            params["session_leaf_id"] = json!(new_leaf);
        } else if !conversation_file_id.is_empty() {
            // Legacy flat JSON mode
            let mut messages: Vec<Value> = read_conversation_from_temperfs(
                &ctx,
                &temper_api_url,
                tenant,
                conversation_file_id,
            )?;

            // Append tool results as a user message (Anthropic API format)
            messages.push(json!({
                "role": "user",
                "content": tool_results,
            }));

            let updated_conversation = serde_json::to_string(&messages).unwrap_or_default();
            let body = format!("{{\"messages\":{updated_conversation}}}");
            let url = format!("{temper_api_url}/tdata/Files('{conversation_file_id}')/$value");
            let headers = file_headers(&ctx, tenant, Some("application/json"), None);
            match ctx.http_call("PUT", &url, &headers, &body) {
                Ok(resp) if resp.status >= 200 && resp.status < 300 => {
                    ctx.log(
                        "info",
                        &format!(
                            "tool_runner: wrote conversation to TemperFS ({} bytes)",
                            body.len()
                        ),
                    );
                }
                Ok(resp) => {
                    return Err(format!(
                        "TemperFS conversation write failed (HTTP {}): {}",
                        resp.status,
                        &resp.body[..resp.body.len().min(200)]
                    ));
                }
                Err(e) => {
                    return Err(format!("TemperFS conversation write failed: {e}"));
                }
            }
            params["conversation"] = json!(updated_conversation);
        } else {
            // Inline conversation mode (no TemperFS)
            let mut messages: Vec<Value> = {
                let conversation_json = fields
                    .get("conversation")
                    .and_then(|v| v.as_str())
                    .unwrap_or("[]");
                serde_json::from_str(conversation_json).unwrap_or_default()
            };

            messages.push(json!({
                "role": "user",
                "content": tool_results,
            }));

            let updated_conversation = serde_json::to_string(&messages).unwrap_or_default();
            params["conversation"] = json!(updated_conversation);
        }

        // Fsync sandbox files to TemperFS (best-effort)
        let file_manifest_id = fields
            .get("file_manifest_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let max_sync_file_bytes: u64 = ctx
            .config
            .get("max_sync_file_bytes")
            .and_then(|s| s.parse().ok())
            .unwrap_or(61440);
        let max_sync_files: usize = ctx
            .config
            .get("max_sync_files")
            .and_then(|s| s.parse().ok())
            .unwrap_or(64);
        let sync_exclude = ctx.config.get("sync_exclude").cloned().unwrap_or_default();

        if !file_manifest_id.is_empty() && !workspace_id.is_empty() && !sandbox_url.is_empty() {
            match sync_files_to_temperfs(
                &ctx,
                sandbox_url,
                &temper_api_url,
                tenant,
                workspace_id,
                file_manifest_id,
                workdir,
                max_sync_file_bytes,
                max_sync_files,
                &sync_exclude,
            ) {
                Ok(count) => ctx.log(
                    "info",
                    &format!("tool_runner: fsync complete ({count} files synced)"),
                ),
                Err(e) => ctx.log(
                    "warn",
                    &format!("tool_runner: fsync failed (non-fatal): {e}"),
                ),
            }
        }

        set_success_result("HandleToolResults", &params);

        Ok(())
    })();

    if let Err(e) = result {
        set_error_result(&e);
    }
    0
}

/// Execute a single tool call against the sandbox API.
/// Every sandbox target must implement the local sandbox HTTP API
/// (`/api/v1/files`, `/api/v1/processes`), whether the URL is local or remote.
fn execute_tool(
    ctx: &Context,
    sandbox_url: &str,
    workdir: &str,
    tool_name: &str,
    input: &Value,
) -> Result<String, String> {
    ensure_local_workdir(ctx, sandbox_url, workdir)?;

    match tool_name {
        "read" => {
            let path = input
                .get("path")
                .and_then(|v| v.as_str())
                .ok_or("read: missing 'path' parameter")?;

            let full_path = resolve_path(workdir, path);
            read_file_local(ctx, sandbox_url, &full_path)
        }
        "write" => {
            let path = input
                .get("path")
                .and_then(|v| v.as_str())
                .ok_or("write: missing 'path' parameter")?;
            let content = input
                .get("content")
                .and_then(|v| v.as_str())
                .ok_or("write: missing 'content' parameter")?;

            let full_path = resolve_path(workdir, path);
            write_file_local(ctx, sandbox_url, &full_path, content)
        }
        "edit" => {
            let path = input
                .get("path")
                .and_then(|v| v.as_str())
                .ok_or("edit: missing 'path' parameter")?;
            let old_string = input
                .get("old_string")
                .and_then(|v| v.as_str())
                .ok_or("edit: missing 'old_string' parameter")?;
            let new_string = input
                .get("new_string")
                .and_then(|v| v.as_str())
                .ok_or("edit: missing 'new_string' parameter")?;

            let full_path = resolve_path(workdir, path);
            // Read current file
            let current = read_file_local(ctx, sandbox_url, &full_path)?;

            if !current.contains(old_string) {
                return Err(format!("edit: old_string not found in {full_path}"));
            }
            let updated = current.replacen(old_string, new_string, 1);

            // Write updated file
            write_file_local(ctx, sandbox_url, &full_path, &updated)?;
            Ok(format!("File edited: {full_path}"))
        }
        "bash" => {
            let command = input
                .get("command")
                .and_then(|v| v.as_str())
                .ok_or("bash: missing 'command' parameter")?;

            run_bash_local(ctx, sandbox_url, command, workdir)
        }
        "datadog_query" => datadog::execute(ctx, input),
        "railway_api" => railway::execute(ctx, input),
        "vercel_api" => vercel::execute(ctx, input),
        unknown => Err(format!("unknown tool: {unknown}")),
    }
}

// --- Tensorlake sandbox API ---

/// Get the Tensorlake API key from integration config (empty string if not set).
fn tensorlake_api_key(ctx: &Context) -> String {
    ctx.config
        .get("tensorlake_api_key")
        .filter(|s| !s.is_empty() && !s.contains("{secret:"))
        .cloned()
        .unwrap_or_default()
}

/// Build sandbox request headers with optional Bearer auth.
fn sandbox_headers(api_key: &str, content_type: Option<&str>) -> Vec<(String, String)> {
    let mut headers = Vec::new();
    if !api_key.is_empty() {
        headers.push(("authorization".to_string(), format!("Bearer {api_key}")));
    }
    if let Some(ct) = content_type {
        headers.push(("content-type".to_string(), ct.to_string()));
    }
    headers
}

/// Read file via Tensorlake sandbox data plane API.
fn read_file_local(ctx: &Context, sandbox_url: &str, full_path: &str) -> Result<String, String> {
    let api_key = tensorlake_api_key(ctx);
    let url = format!("{sandbox_url}/api/v1/files?path={}", url_encode(full_path));
    let headers = sandbox_headers(&api_key, None);
    let resp = ctx.http_call("GET", &url, &headers, "")?;
    if resp.status >= 200 && resp.status < 300 {
        Ok(resp.body)
    } else {
        Err(format!("read failed (HTTP {}): {}", resp.status, resp.body))
    }
}

/// Write file via Tensorlake sandbox data plane API.
fn write_file_local(
    ctx: &Context,
    sandbox_url: &str,
    full_path: &str,
    content: &str,
) -> Result<String, String> {
    let api_key = tensorlake_api_key(ctx);
    let url = format!("{sandbox_url}/api/v1/files?path={}", url_encode(full_path));
    let headers = sandbox_headers(&api_key, Some("application/octet-stream"));
    let resp = ctx.http_call("PUT", &url, &headers, content)?;
    if resp.status >= 200 && resp.status < 300 {
        Ok(format!("File written: {full_path}"))
    } else {
        Err(format!(
            "write failed (HTTP {}): {}",
            resp.status, resp.body
        ))
    }
}

/// Delete file via Tensorlake sandbox data plane API (best-effort).
fn delete_file_sandbox(ctx: &Context, sandbox_url: &str, path: &str) {
    let api_key = tensorlake_api_key(ctx);
    let url = format!("{sandbox_url}/api/v1/files?path={}", url_encode(path));
    let headers = sandbox_headers(&api_key, None);
    let _ = ctx.http_call("DELETE", &url, &headers, "");
}

/// Generate a short hex run ID for output-redirection polling.
fn short_run_id() -> String {
    // Use a simple counter-like approach from entity state hash.
    // In WASM we don't have time or random — use a static counter.
    use core::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{n:08x}")
}

/// Run bash command via Tensorlake sandbox data plane API.
///
/// Tensorlake processes are async (POST to start, then poll). Since WASM has
/// no SSE support or sleep(), we use output-redirection: wrap the command to
/// redirect stdout/stderr to temp files, then poll for the exit code file.
pub(crate) fn run_bash_local(
    ctx: &Context,
    sandbox_url: &str,
    command: &str,
    workdir: &str,
) -> Result<String, String> {
    let api_key = tensorlake_api_key(ctx);
    let command = prepare_bash_command(command);
    let run_id = short_run_id();
    let out_path = format!("/tmp/.paw-out-{run_id}");
    let err_path = format!("/tmp/.paw-err-{run_id}");
    let rc_path = format!("/tmp/.paw-rc-{run_id}");

    // Wrap command: redirect stdout/stderr to files, write exit code to sentinel.
    let wrapped = format!(
        "({command}) > {out_path} 2> {err_path}; echo $? > {rc_path}",
    );

    let env = sandbox_env(ctx);
    let mut env_list: Vec<String> = Vec::new();
    for (k, v) in &env {
        env_list.push(format!("{k}={v}"));
    }

    // Start process via Tensorlake API.
    let start_url = format!("{sandbox_url}/api/v1/processes");
    let headers = sandbox_headers(&api_key, Some("application/json"));
    let body = json!({
        "command": "bash",
        "args": ["-c", &wrapped],
        "cwd": workdir,
        "env": env,
    });
    let resp = ctx.http_call("POST", &start_url, &headers, &body.to_string())?;
    if resp.status < 200 || resp.status >= 300 {
        return Err(format!(
            "process start failed (HTTP {}): {}",
            resp.status, resp.body
        ));
    }

    // Poll for exit code file. Network latency provides ~50-200ms natural backoff.
    let rc_url = format!("{sandbox_url}/api/v1/files?path={}", url_encode(&rc_path));
    let read_headers = sandbox_headers(&api_key, None);
    let mut exit_code: i64 = -1;
    let mut found = false;
    for attempt in 0..600 {
        let rc_resp = ctx.http_call("GET", &rc_url, &read_headers, "");
        match rc_resp {
            Ok(r) if r.status >= 200 && r.status < 300 => {
                exit_code = r.body.trim().parse::<i64>().unwrap_or(-1);
                found = true;
                break;
            }
            _ => {
                if attempt % 50 == 0 && attempt > 0 {
                    ctx.log(
                        "info",
                        &format!("run_bash: still waiting for process (attempt {attempt})..."),
                    );
                }
            }
        }
    }

    if !found {
        // Cleanup temp files even on timeout.
        delete_file_sandbox(ctx, sandbox_url, &out_path);
        delete_file_sandbox(ctx, sandbox_url, &err_path);
        delete_file_sandbox(ctx, sandbox_url, &rc_path);
        return Err("process timed out after ~300s".to_string());
    }

    // Read stdout and stderr.
    let stdout = read_file_local(ctx, sandbox_url, &out_path).unwrap_or_default();
    let stderr = read_file_local(ctx, sandbox_url, &err_path).unwrap_or_default();

    // Cleanup temp files (best effort).
    delete_file_sandbox(ctx, sandbox_url, &out_path);
    delete_file_sandbox(ctx, sandbox_url, &err_path);
    delete_file_sandbox(ctx, sandbox_url, &rc_path);

    // Format output same as before.
    let mut output = String::new();
    if !stdout.is_empty() {
        output.push_str(&stdout);
    }
    if !stderr.is_empty() {
        if !output.is_empty() {
            output.push('\n');
        }
        output.push_str("STDERR: ");
        output.push_str(&stderr);
    }
    if exit_code != 0 {
        output.push_str(&format!("\n(exit code: {exit_code})"));
    }
    Ok(output)
}

fn ensure_local_workdir(ctx: &Context, sandbox_url: &str, workdir: &str) -> Result<(), String> {
    if workdir.trim().is_empty() || workdir == "/" {
        return Ok(());
    }

    // Use run_bash_local which handles the Tensorlake async process pattern.
    let result = run_bash_local(
        ctx,
        sandbox_url,
        &format!("mkdir -p -- {}", shell_quote(workdir)),
        "/",
    )?;

    // Check if there was an error in the output.
    if result.contains("(exit code:") && !result.contains("(exit code: 0)") {
        return Err(format!("failed to prepare workdir {workdir}: {result}"));
    }

    Ok(())
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

/// Produce a Python string repr for embedding in generated scripts.
fn python_repr(value: &str) -> String {
    let escaped = value
        .replace('\\', "\\\\")
        .replace('\'', "\\'");
    format!("'{escaped}'")
}

fn truncate_tool_result(content: &str) -> String {
    if content.len() <= MAX_TOOL_RESULT_BYTES {
        return content.to_string();
    }

    let mut end = MAX_TOOL_RESULT_BYTES;
    while !content.is_char_boundary(end) {
        end -= 1;
    }

    format!(
        "{}\n\n[truncated tool output: kept {} of {} bytes]",
        &content[..end],
        end,
        content.len()
    )
}

fn compact_tool_results_marker(tool_results: &[Value]) -> String {
    let total_bytes: usize = tool_results
        .iter()
        .map(|result| {
            result
                .get("content")
                .and_then(Value::as_str)
                .map(str::len)
                .unwrap_or(0)
        })
        .sum();

    format!(
        "[stored {} tool result(s) in session tree; {} bytes retained outside entity state]",
        tool_results.len(),
        total_bytes
    )
}

fn sandbox_env(ctx: &Context) -> BTreeMap<String, String> {
    let mut env = BTreeMap::new();

    if let Ok(token) = ctx.get_secret("github_token") {
        if !token.trim().is_empty() {
            env.insert("GITHUB_TOKEN".to_string(), token.clone());
            env.insert("GH_TOKEN".to_string(), token);
            env.insert("GIT_TERMINAL_PROMPT".to_string(), "0".to_string());
        }
    }

    env
}

fn prepare_bash_command(command: &str) -> String {
    format!(
        r#"if [ -n "${{GITHUB_TOKEN:-}}" ]; then
  export GH_TOKEN="${{GH_TOKEN:-$GITHUB_TOKEN}}"
  export GIT_TERMINAL_PROMPT="${{GIT_TERMINAL_PROMPT:-0}}"
  export GIT_ASKPASS="${{TMPDIR:-/tmp}}/openpaw-git-askpass.sh"
  cat >"$GIT_ASKPASS" <<'EOF'
#!/bin/sh
case "$1" in
  *Username*) printf '%s\n' "x-access-token" ;;
  *Password*) printf '%s\n' "${{GITHUB_TOKEN:-}}" ;;
  *) printf '\n' ;;
esac
EOF
  chmod 700 "$GIT_ASKPASS"
fi
{command}"#
    )
}

/// Read conversation from TemperFS File entity.
fn read_conversation_from_temperfs(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    file_id: &str,
) -> Result<Vec<Value>, String> {
    let url = format!("{temper_api_url}/tdata/Files('{file_id}')/$value");
    let headers = file_headers(ctx, tenant, None, Some("application/json"));

    let resp = ctx
        .http_call("GET", &url, &headers, "")
        .map_err(|e| format!("TemperFS conversation read failed: {e}"))?;

    if resp.status != 200 {
        return Err(format!(
            "TemperFS conversation read failed (HTTP {}): {}",
            resp.status,
            &resp.body[..resp.body.len().min(200)]
        ));
    }

    let parsed: Value = serde_json::from_str(&resp.body)
        .map_err(|e| format!("TemperFS conversation parse failed: {e}"))?;

    Ok(parsed
        .get("messages")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default())
}

/// Resolve a path relative to the working directory.
fn resolve_path(workdir: &str, path: &str) -> String {
    if path.starts_with('/') {
        path.to_string()
    } else {
        format!("{}/{}", workdir.trim_end_matches('/'), path)
    }
}

/// Minimal URL encoding for path parameters.
pub(crate) fn url_encode(s: &str) -> String {
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

pub(crate) fn odata_headers(ctx: &Context, tenant: &str) -> Vec<(String, String)> {
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

pub(crate) fn file_headers(
    ctx: &Context,
    tenant: &str,
    content_type: Option<&str>,
    accept: Option<&str>,
) -> Vec<(String, String)> {
    let fields = ctx
        .entity_state
        .get("fields")
        .cloned()
        .unwrap_or_else(|| json!({}));
    runtime_headers(ctx, tenant, &fields, content_type, accept)
}

// --- Sandbox Fsync to TemperFS ---

/// File metadata from sandbox `find` + `stat`.
struct FileEntry {
    size_bytes: u64,
    mtime: u64,
}

/// Manifest entry stored in TemperFS.
struct ManifestEntry {
    file_id: String,
    size_bytes: u64,
    mtime: u64,
}

/// Enumerate all files in the sandbox workspace with a portable Python walk.
/// Returns a map of path → FileEntry with size and mtime.
fn enumerate_sandbox_files(
    ctx: &Context,
    sandbox_url: &str,
    workdir: &str,
    exclude: &str,
) -> Result<BTreeMap<String, FileEntry>, String> {
    // Write a Python enumeration script to the sandbox, then execute it.
    // Heredocs don't survive JSON serialization through the Tensorlake process API,
    // so we use write_file_local + run_bash_local instead.
    let script = format!(
        r#"import json, os
workdir = {workdir_repr}
exclude = {{part.strip() for part in {exclude_repr}.split(',') if part.strip()}}
for root, dirs, files in os.walk(workdir, topdown=True):
    dirs[:] = [d for d in dirs if not d.startswith('.') and d not in exclude]
    for name in files:
        if name.startswith('.') or name in exclude:
            continue
        path = os.path.join(root, name)
        rel = os.path.relpath(path, workdir)
        parts = [part for part in rel.split(os.sep) if part not in ('', '.')]
        if any(part.startswith('.') or part in exclude for part in parts):
            continue
        if os.path.islink(path) or not os.path.isfile(path):
            continue
        try:
            st = os.stat(path)
        except OSError:
            continue
        print(json.dumps({{"path": path, "size_bytes": int(st.st_size), "mtime": int(st.st_mtime)}}))
"#,
        workdir_repr = python_repr(workdir),
        exclude_repr = python_repr(exclude),
    );

    let script_path = "/tmp/.paw-enum.py";
    write_file_local(ctx, sandbox_url, script_path, &script)?;
    let output = run_bash_local(ctx, sandbox_url, &format!("python3 {script_path}"), workdir)?;
    delete_file_sandbox(ctx, sandbox_url, script_path);

    let mut files = BTreeMap::new();
    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("STDERR:") {
            continue;
        }
        let Ok(entry) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let Some(path) = entry.get("path").and_then(Value::as_str) else {
            continue;
        };
        let Some(size_bytes) = entry.get("size_bytes").and_then(Value::as_u64) else {
            continue;
        };
        let Some(mtime) = entry.get("mtime").and_then(Value::as_u64) else {
            continue;
        };
        files.insert(path.to_string(), FileEntry { size_bytes, mtime });
    }

    Ok(files)
}

/// Read the file manifest from TemperFS.
fn read_manifest(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    manifest_file_id: &str,
) -> Result<BTreeMap<String, ManifestEntry>, String> {
    let url = format!("{temper_api_url}/tdata/Files('{manifest_file_id}')/$value");
    let headers = file_headers(ctx, tenant, None, Some("application/json"));

    let resp = ctx.http_call("GET", &url, &headers, "")?;
    if resp.status != 200 {
        return Ok(BTreeMap::new());
    }

    let parsed: Value = serde_json::from_str(&resp.body).unwrap_or(json!({}));
    let files_obj = parsed.get("files").and_then(|v| v.as_object());

    let mut manifest = BTreeMap::new();
    if let Some(files) = files_obj {
        for (path, entry) in files {
            let file_id = entry
                .get("file_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let size_bytes = entry
                .get("size_bytes")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let mtime = entry.get("mtime").and_then(|v| v.as_u64()).unwrap_or(0);
            if !file_id.is_empty() {
                manifest.insert(
                    path.clone(),
                    ManifestEntry {
                        file_id,
                        size_bytes,
                        mtime,
                    },
                );
            }
        }
    }

    Ok(manifest)
}

/// Simple hash function for deterministic File entity IDs.
/// Returns first 16 hex chars of a djb2 hash.
fn simple_hash(input: &str) -> String {
    let mut hash: u64 = 5381;
    for b in input.bytes() {
        hash = hash.wrapping_mul(33).wrapping_add(b as u64);
    }
    format!("{hash:016x}")
}

/// Sync sandbox files to TemperFS. Returns the number of files synced.
fn sync_files_to_temperfs(
    ctx: &Context,
    sandbox_url: &str,
    temper_api_url: &str,
    tenant: &str,
    workspace_id: &str,
    manifest_file_id: &str,
    workdir: &str,
    max_file_bytes: u64,
    max_files: usize,
    exclude: &str,
) -> Result<usize, String> {
    ensure_local_workdir(ctx, sandbox_url, workdir)?;

    // 1. Enumerate current sandbox files with stat metadata
    let current_files = enumerate_sandbox_files(ctx, sandbox_url, workdir, exclude)?;
    ctx.log(
        "info",
        &format!(
            "tool_runner: fsync enumerated {} files",
            current_files.len()
        ),
    );

    // 2. Read previous manifest from TemperFS
    let old_manifest = read_manifest(ctx, temper_api_url, tenant, manifest_file_id)?;

    let headers = file_headers(ctx, tenant, Some("application/json"), None);

    let file_url = format!("{temper_api_url}/tdata/Files");
    let mut new_manifest: BTreeMap<String, Value> = old_manifest
        .iter()
        .map(|(path, entry)| {
            (
                path.clone(),
                json!({
                    "file_id": entry.file_id,
                    "size_bytes": entry.size_bytes,
                    "mtime": entry.mtime,
                }),
            )
        })
        .collect();
    let mut synced_count: usize = 0;
    let budget_limited = current_files.len() > max_files;
    if budget_limited {
        ctx.log(
            "warn",
            &format!(
                "tool_runner: fsync limiting uploads to first {max_files} of {} files",
                current_files.len()
            ),
        );
    }

    // 3. Sync new/modified files
    for (index, (path, entry)) in current_files.iter().enumerate() {
        if index >= max_files {
            break;
        }
        // Check if unchanged (size AND mtime match)
        if let Some(old_entry) = old_manifest.get(path) {
            if old_entry.size_bytes == entry.size_bytes && old_entry.mtime == entry.mtime {
                // Unchanged — carry forward manifest entry without reading file
                new_manifest.insert(
                    path.clone(),
                    json!({
                        "file_id": old_entry.file_id,
                        "size_bytes": old_entry.size_bytes,
                        "mtime": old_entry.mtime,
                    }),
                );
                continue;
            }
        }

        // File is new or modified — read from sandbox
        let content = read_file_local(ctx, sandbox_url, path);

        let content = match content {
            Ok(c) => c,
            Err(e) => {
                ctx.log(
                    "warn",
                    &format!("tool_runner: fsync skip {path}: read failed: {e}"),
                );
                continue;
            }
        };

        if content.len() as u64 > max_file_bytes {
            ctx.log(
                "warn",
                &format!(
                    "tool_runner: fsync skip {path}: {} bytes exceeds max {}",
                    content.len(),
                    max_file_bytes
                ),
            );
            continue;
        }

        // Deterministic File entity ID from workspace + path
        let file_entity_id = format!("wsf-{}", simple_hash(&format!("{workspace_id}:{path}")));

        // Create File entity (ignore 409 = already exists)
        let create_body = json!({
            "FileId": &file_entity_id,
            "workspace_id": workspace_id,
            "name": path.rsplit('/').next().unwrap_or("file"),
            "mime_type": "text/plain",
            "path": path,
        });
        let _ = ctx.http_call("POST", &file_url, &headers, &create_body.to_string());

        // Upload content (CAS dedup handles unchanged content)
        let value_url = format!("{temper_api_url}/tdata/Files('{file_entity_id}')/$value");
        match ctx.http_call("PUT", &value_url, &headers, &content) {
            Ok(resp) if resp.status >= 200 && resp.status < 300 => {
                synced_count += 1;
                new_manifest.insert(
                    path.clone(),
                    json!({
                        "file_id": file_entity_id,
                        "size_bytes": entry.size_bytes,
                        "mtime": entry.mtime,
                    }),
                );
            }
            Ok(resp) => {
                ctx.log(
                    "warn",
                    &format!(
                        "tool_runner: fsync upload failed for {path} (HTTP {})",
                        resp.status
                    ),
                );
            }
            Err(e) => {
                ctx.log(
                    "warn",
                    &format!("tool_runner: fsync upload failed for {path}: {e}"),
                );
            }
        }
    }

    // 4. Handle deletions — archive files that no longer exist in sandbox
    for (path, old_entry) in &old_manifest {
        if !current_files.contains_key(path) {
            let archive_url = format!(
                "{temper_api_url}/tdata/Files('{}')/Paw.FS.FileArchive",
                old_entry.file_id
            );
            match ctx.http_call("POST", &archive_url, &headers, "{}") {
                Ok(_) => ctx.log(
                    "info",
                    &format!("tool_runner: fsync archived deleted file {path}"),
                ),
                Err(e) => ctx.log(
                    "warn",
                    &format!("tool_runner: fsync archive failed for {path}: {e}"),
                ),
            }
        }
    }

    // 5. Write updated manifest to TemperFS
    let manifest_body = json!({ "files": new_manifest }).to_string();
    let manifest_url = format!("{temper_api_url}/tdata/Files('{manifest_file_id}')/$value");
    ctx.http_call("PUT", &manifest_url, &headers, &manifest_body)
        .map_err(|e| format!("manifest write failed: {e}"))?;

    Ok(synced_count)
}

// --- Entity tool dispatch ---

fn emit_progress_ignore(ctx: &Context, payload: Value) {
    let _ = (ctx, payload);
}

fn send_heartbeat(ctx: &Context, temper_api_url: &str, tenant: &str) -> Result<(), String> {
    let url = format!(
        "{temper_api_url}/tdata/Sessions('{}')/OpenPaw.Heartbeat",
        ctx.entity_id
    );
    let body = json!({ "last_heartbeat_at": "alive" });
    let _ = ctx.http_call("POST", &url, &odata_headers(ctx, tenant), &body.to_string())?;
    Ok(())
}

fn validate_tool_input(tool_name: &str, input: &Value) -> Result<(), String> {
    let object = input
        .as_object()
        .ok_or_else(|| format!("{tool_name}: input must be an object"))?;
    let required: &[&str] = match tool_name {
        "read" => &["path"],
        "write" => &["path", "content"],
        "edit" => &["path", "old_string", "new_string"],
        "bash" => &["command"],
        "save_memory" => &["key", "content"],
        "recall_memory" => &["query"],
        "spawn_session" => &["task"],
        "abort_session" => &["agent_id"],
        "steer_session" => &["agent_id", "message"],
        "read_entity" => &["file_id"],
        "file_upload" => &["name", "content"],
        "temper_create" => &["entity_set"],
        "temper_get" => &["entity_set", "entity_id"],
        "temper_list" => &["entity_set"],
        "temper_action" => &["entity_set", "entity_id", "action"],
        "run_coding_agent" => &["agent_type", "task"],
        "railway_api" => &["action"],
        "vercel_api" => &["action"],
        _ => &[],
    };
    for key in required {
        let Some(value) = object.get(*key) else {
            return Err(format!("{tool_name}: missing '{key}'"));
        };
        if value.is_null() || value.as_str().is_some_and(str::is_empty) {
            return Err(format!("{tool_name}: '{key}' must not be empty"));
        }
    }
    Ok(())
}

fn evaluate_before_hooks(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    soul_id: &str,
    hook_policy: &str,
    tool_name: &str,
    input: &Value,
    project_harness_id: &str,
) -> Result<Option<String>, String> {
    if hook_policy == "none" || soul_id.is_empty() {
        return Ok(None);
    }
    let hooks = load_matching_hooks(ctx, temper_api_url, tenant, soul_id, "before", tool_name)?;
    for hook in hooks {
        let action = entity_field_str(&hook, &["HookAction"]).unwrap_or("log");
        let name = entity_field_str(&hook, &["Name"]).unwrap_or("hook");
        let cedar_enabled = entity_field_str(&hook, &["CedarEnabled", "cedar_enabled"])
            .unwrap_or("false");

        if cedar_enabled == "true" {
            // Cedar-backed authorization hook
            let command_content = if tool_name == "bash" {
                input.get("command").and_then(|v| v.as_str()).unwrap_or("")
            } else {
                ""
            };

            // Check command against hook's command_pattern (simple contains check)
            let pattern =
                entity_field_str(&hook, &["CommandPattern", "command_pattern"]).unwrap_or("");
            if !pattern.is_empty() && !command_content.contains(pattern) {
                continue; // Pattern doesn't match, skip this hook
            }

            // Check project scope
            let hook_project =
                entity_field_str(&hook, &["ProjectScope", "project_scope"]).unwrap_or("");
            if !hook_project.is_empty() && hook_project != project_harness_id {
                continue; // Wrong project, skip
            }

            // Cedar policy matched — apply the hook action
            ctx.log(
                "info",
                &format!(
                    "tool_runner: cedar hook '{name}' matched tool '{tool_name}' (pattern='{pattern}')"
                ),
            );
            if action == "block" {
                return Ok(Some(format!(
                    "tool '{}' blocked by Cedar policy '{}' (command matched pattern '{}')",
                    tool_name, name, pattern
                )));
            }
            // For non-block cedar hooks (e.g. "log"), continue evaluation
            continue;
        }

        // Non-cedar hooks: original logic
        match action {
            "block" => {
                return Ok(Some(format!(
                    "tool blocked by hook '{name}' for tool '{tool_name}'"
                )));
            }
            "log" => ctx.log(
                "info",
                &format!("tool_runner: before hook '{name}' matched {tool_name}"),
            ),
            _ => {}
        }
    }
    Ok(None)
}

fn apply_after_hooks(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    soul_id: &str,
    hook_policy: &str,
    tool_name: &str,
    mut output: String,
) -> Result<String, String> {
    if hook_policy != "full_hooks" || soul_id.is_empty() {
        return Ok(output);
    }
    let hooks = load_matching_hooks(ctx, temper_api_url, tenant, soul_id, "after", tool_name)?;
    for hook in hooks {
        let action = entity_field_str(&hook, &["HookAction"]).unwrap_or("log");
        let name = entity_field_str(&hook, &["Name"]).unwrap_or("hook");
        match action {
            "modify" => {
                output = format!("[modified by hook:{name}]\n{output}");
            }
            "log" => ctx.log(
                "info",
                &format!("tool_runner: after hook '{name}' matched {tool_name}"),
            ),
            _ => {}
        }
    }
    Ok(output)
}

fn load_matching_hooks(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    soul_id: &str,
    hook_type: &str,
    tool_name: &str,
) -> Result<Vec<Value>, String> {
    let url = format!("{temper_api_url}/tdata/ToolHooks");
    let resp = ctx.http_call("GET", &url, &odata_headers(ctx, tenant), "")?;
    if resp.status != 200 {
        return Ok(Vec::new());
    }
    let parsed: Value = serde_json::from_str(&resp.body).unwrap_or_else(|_| json!({ "value": [] }));
    let hooks = parsed
        .get("value")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|hook| {
            entity_field_str(hook, &["Status"]) == Some("Active")
                && entity_field_str(hook, &["SoulId"]).unwrap_or("") == soul_id
                && entity_field_str(hook, &["HookType"]).unwrap_or("") == hook_type
                && hook_matches(
                    entity_field_str(hook, &["ToolPattern"]).unwrap_or(".*"),
                    tool_name,
                )
        })
        .collect::<Vec<_>>();
    Ok(hooks)
}

fn hook_matches(pattern: &str, tool_name: &str) -> bool {
    let pattern = pattern.trim();
    if pattern.is_empty() || pattern == ".*" || pattern == "*" {
        return true;
    }
    if pattern.contains('|') {
        return pattern.split('|').any(|part| part.trim() == tool_name);
    }
    pattern == tool_name
}

fn normalize_field_key(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_alphanumeric())
        .flat_map(|ch| ch.to_lowercase())
        .collect()
}

fn direct_field_value<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a Value> {
    let object = value.as_object()?;
    for key in keys {
        if let Some(found) = object.get(*key) {
            return Some(found);
        }
    }
    let normalized_keys = keys
        .iter()
        .map(|key| normalize_field_key(key))
        .collect::<Vec<_>>();
    object.iter().find_map(|(key, value)| {
        let normalized_key = normalize_field_key(key);
        normalized_keys
            .iter()
            .any(|candidate| candidate == &normalized_key)
            .then_some(value)
    })
}

fn direct_field_str<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a str> {
    direct_field_value(value, keys).and_then(Value::as_str)
}

pub(crate) fn entity_field_str<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a str> {
    direct_field_value(value, &["fields"])
        .and_then(|fields| direct_field_str(fields, keys))
        .or_else(|| direct_field_str(value, keys))
}

/// Read session JSONL from TemperFS.
fn read_session_from_temperfs(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    file_id: &str,
) -> Result<String, String> {
    let url = format!("{temper_api_url}/tdata/Files('{file_id}')/$value");
    let headers = file_headers(ctx, tenant, None, None);
    let resp = ctx.http_call("GET", &url, &headers, "")?;
    if resp.status == 200 {
        Ok(resp.body)
    } else if resp.status == 404 {
        Ok(String::new())
    } else {
        Err(format!(
            "TemperFS session read failed (HTTP {})",
            resp.status
        ))
    }
}

/// Write session JSONL to TemperFS.
fn write_session_to_temperfs(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    file_id: &str,
    jsonl: &str,
) -> Result<(), String> {
    let url = format!("{temper_api_url}/tdata/Files('{file_id}')/$value");
    let headers = file_headers(ctx, tenant, Some("text/plain"), None);
    let resp = ctx.http_call("PUT", &url, &headers, jsonl)?;
    if resp.status >= 200 && resp.status < 300 {
        Ok(())
    } else {
        Err(format!(
            "TemperFS session write failed (HTTP {})",
            resp.status
        ))
    }
}

fn create_tool_content_file(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    workspace_id: &str,
    entry_id: &str,
    content: &str,
) -> Result<String, String> {
    let file_name = format!("msg-{entry_id}.txt");
    create_content_file(ctx, temper_api_url, tenant, workspace_id, &file_name, content)
}
