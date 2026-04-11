//! Sandbox provider abstraction — match-based dispatch for sandbox lifecycle,
//! file I/O, and command execution across multiple providers (Tensorlake, Modal).
//!
//! Follows the same pattern as `llm_caller`'s provider selection:
//! entity field → config → error. No dynamic dispatch (`dyn Trait`),
//! WASM-compatible throughout.

use serde_json::{Value, json};
use std::path::Path;
use temper_wasm_sdk::context::Context;

use crate::entity_field_str;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Handle to a provisioned sandbox. Carries provider so subsequent operations
/// route to the correct backend.
pub struct SandboxHandle {
    pub sandbox_url: String,
    pub sandbox_id: String,
    pub provider: String,
}

/// Resources requested when creating a sandbox.
pub struct SandboxConfig {
    pub cpus: u32,
    pub memory_mb: u32,
    pub timeout_seconds: u32,
    pub internet_access: bool,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            cpus: 2,
            memory_mb: 4096,
            timeout_seconds: 3600,
            internet_access: true,
        }
    }
}

/// Result of a bash command execution.
pub struct ExecResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i64,
}

// ---------------------------------------------------------------------------
// Provider resolution
// ---------------------------------------------------------------------------

/// Normalize provider name to canonical form.
pub fn normalize_sandbox_provider(provider: &str) -> String {
    match provider.trim().to_ascii_lowercase().as_str() {
        "tensorlake" | "tl" => "tensorlake".to_string(),
        "modal" => "modal".to_string(),
        other => other.to_string(),
    }
}

/// Determine which sandbox provider to use.
/// Priority: entity field → integration config → error (explicit selection required).
pub fn resolve_sandbox_provider(ctx: &Context, fields: &Value) -> Result<String, String> {
    // 1. Entity field (per-session override)
    if let Some(provider) = entity_field_str(fields, &["sandbox_provider", "SandboxProvider"])
        .filter(|s| !s.is_empty() && !is_unresolved_secret(s))
    {
        return Ok(normalize_sandbox_provider(provider));
    }

    // 2. Integration config
    if let Some(provider) = ctx
        .config
        .get("sandbox_provider")
        .filter(|s| !s.is_empty() && !is_unresolved_secret(s))
    {
        return Ok(normalize_sandbox_provider(provider));
    }

    Err(
        "no sandbox_provider configured — set SANDBOX_PROVIDER in .env (\"tensorlake\" or \"modal\")"
            .to_string(),
    )
}

/// Resolve the API key/token for the given provider.
pub fn resolve_sandbox_api_key(ctx: &Context, provider: &str) -> Result<String, String> {
    let key = match provider {
        "tensorlake" => first_non_empty(&[
            ctx.config.get("tensorlake_api_key").cloned(),
            ctx.config.get("sandbox_api_key").cloned(),
        ]),
        "modal" => first_non_empty(&[
            ctx.config.get("modal_api_token").cloned(),
            ctx.config.get("sandbox_api_key").cloned(),
        ]),
        other => return Err(format!("unsupported sandbox provider: {other}")),
    };

    if key.is_empty() || is_unresolved_secret(&key) {
        Err(format!(
            "no API key configured for sandbox provider '{provider}'"
        ))
    } else {
        Ok(key)
    }
}

// ---------------------------------------------------------------------------
// Sandbox lifecycle
// ---------------------------------------------------------------------------

/// Create a new sandbox via the provider's control plane.
pub fn sandbox_create(
    ctx: &Context,
    provider: &str,
    config: &SandboxConfig,
) -> Result<SandboxHandle, String> {
    let api_key = resolve_sandbox_api_key(ctx, provider)?;
    match provider {
        "tensorlake" => tensorlake_create(ctx, &api_key, config),
        "modal" => modal_create(ctx, &api_key, config),
        other => Err(format!("unsupported sandbox provider: {other}")),
    }
}

/// Check if a sandbox is ready to accept commands.
pub fn sandbox_health_check(ctx: &Context, handle: &SandboxHandle) -> Result<bool, String> {
    let api_key = resolve_sandbox_api_key(ctx, &handle.provider)?;
    match handle.provider.as_str() {
        "tensorlake" => tensorlake_health_check(ctx, &api_key, &handle.sandbox_url),
        "modal" => modal_health_check(ctx, &api_key, &handle.sandbox_id),
        other => Err(format!("unsupported sandbox provider: {other}")),
    }
}

// ---------------------------------------------------------------------------
// File I/O
// ---------------------------------------------------------------------------

/// Read a file from the sandbox.
pub fn sandbox_file_read(
    ctx: &Context,
    handle: &SandboxHandle,
    path: &str,
) -> Result<String, String> {
    let api_key = resolve_sandbox_api_key(ctx, &handle.provider)?;
    match handle.provider.as_str() {
        "tensorlake" => tensorlake_file_read(ctx, &api_key, &handle.sandbox_url, path),
        "modal" => modal_file_read(ctx, &api_key, &handle.sandbox_id, path),
        other => Err(format!("unsupported sandbox provider: {other}")),
    }
}

/// Write a file to the sandbox.
pub fn sandbox_file_write(
    ctx: &Context,
    handle: &SandboxHandle,
    path: &str,
    content: &str,
) -> Result<(), String> {
    let api_key = resolve_sandbox_api_key(ctx, &handle.provider)?;
    match handle.provider.as_str() {
        "tensorlake" => tensorlake_file_write(ctx, &api_key, &handle.sandbox_url, path, content),
        "modal" => modal_file_write(ctx, &api_key, &handle.sandbox_id, path, content),
        other => Err(format!("unsupported sandbox provider: {other}")),
    }
}

/// Delete a file from the sandbox.
pub fn sandbox_file_delete(
    ctx: &Context,
    handle: &SandboxHandle,
    path: &str,
) -> Result<(), String> {
    let api_key = resolve_sandbox_api_key(ctx, &handle.provider)?;
    match handle.provider.as_str() {
        "tensorlake" => tensorlake_file_delete(ctx, &api_key, &handle.sandbox_url, path),
        "modal" => modal_file_delete(ctx, &api_key, &handle.sandbox_id, path),
        other => Err(format!("unsupported sandbox provider: {other}")),
    }
}

// ---------------------------------------------------------------------------
// Command execution
// ---------------------------------------------------------------------------

/// Execute a bash command in the sandbox.
pub fn sandbox_exec(
    ctx: &Context,
    handle: &SandboxHandle,
    command: &str,
    workdir: &str,
) -> Result<ExecResult, String> {
    let api_key = resolve_sandbox_api_key(ctx, &handle.provider)?;
    match handle.provider.as_str() {
        "tensorlake" => tensorlake_exec(ctx, &api_key, &handle.sandbox_url, command, workdir),
        "modal" => modal_exec(ctx, &api_key, &handle.sandbox_id, command, workdir),
        other => Err(format!("unsupported sandbox provider: {other}")),
    }
}

// ---------------------------------------------------------------------------
// Post-provisioning setup
// ---------------------------------------------------------------------------

/// Run post-provisioning setup (gh CLI etc.). Non-fatal — logs warnings on failure.
pub fn sandbox_setup(ctx: &Context, handle: &SandboxHandle) {
    if handle.sandbox_url.is_empty() {
        return;
    }

    let gh_setup = r#"
if ! command -v gh &>/dev/null; then
  (type -p wget >/dev/null || (apt-get update && apt-get install wget -y)) && \
  mkdir -p -m 755 /etc/apt/keyrings && \
  out=$(mktemp) && wget -nv -O"$out" https://cli.github.com/packages/githubcli-archive-keyring.gpg && \
  cat "$out" | tee /etc/apt/keyrings/githubcli-archive-keyring.gpg > /dev/null && \
  chmod go+r /etc/apt/keyrings/githubcli-archive-keyring.gpg && \
  echo "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/githubcli-archive-keyring.gpg] https://cli.github.com/packages stable main" | tee /etc/apt/sources.list.d/github-cli.list > /dev/null && \
  apt-get update && apt-get install gh -y
fi
gh --version 2>/dev/null || echo 'gh: not installed'
"#;

    match sandbox_exec(ctx, handle, gh_setup, "/") {
        Ok(result) => {
            ctx.log(
                "info",
                &format!("sandbox_setup: gh CLI setup completed (exit {})", result.exit_code),
            );
        }
        Err(e) => {
            ctx.log("warn", &format!("sandbox_setup: gh CLI setup failed: {e}"));
        }
    }
}

// ---------------------------------------------------------------------------
// Shared utilities
// ---------------------------------------------------------------------------

/// URL-encode a string (path-safe: preserves `/`, `-`, `_`, `.`).
pub fn url_encode(s: &str) -> String {
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

fn is_unresolved_secret(value: &str) -> bool {
    value.contains("{secret:")
}

fn first_non_empty(values: &[Option<String>]) -> String {
    for v in values.iter().flatten() {
        let trimmed = v.trim();
        if !trimmed.is_empty() && !is_unresolved_secret(trimmed) {
            return trimmed.to_string();
        }
    }
    String::new()
}

fn bearer_headers(api_key: &str) -> Vec<(String, String)> {
    if api_key.is_empty() {
        vec![]
    } else {
        vec![("authorization".to_string(), format!("Bearer {api_key}"))]
    }
}

fn bearer_headers_json(api_key: &str) -> Vec<(String, String)> {
    let mut h = bearer_headers(api_key);
    h.push(("content-type".to_string(), "application/json".to_string()));
    h
}

/// Generate a unique run ID for output-redirection files.
fn unique_run_id() -> String {
    use core::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    format!("{:08x}", COUNTER.fetch_add(1, Ordering::Relaxed))
}

// ===========================================================================
// Tensorlake provider
// ===========================================================================

fn tensorlake_create(
    ctx: &Context,
    api_key: &str,
    config: &SandboxConfig,
) -> Result<SandboxHandle, String> {
    let create_url = "https://api.tensorlake.ai/sandboxes";
    let headers = bearer_headers_json(api_key);
    let body = json!({
        "resources": {
            "cpus": config.cpus,
            "memory_mb": config.memory_mb
        },
        "timeout_seconds": config.timeout_seconds,
        "internet_access": config.internet_access
    });

    let resp = ctx.http_call("POST", create_url, &headers, &body.to_string())?;
    if resp.status < 200 || resp.status >= 300 {
        return Err(format!(
            "Tensorlake sandbox creation failed (HTTP {}): {}",
            resp.status,
            &resp.body[..resp.body.len().min(500)]
        ));
    }

    let parsed: Value = serde_json::from_str(&resp.body)
        .map_err(|e| format!("failed to parse Tensorlake response: {e}"))?;
    let sandbox_id = parsed
        .get("sandbox_id")
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| {
            parsed
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("tensorlake-sandbox")
        })
        .to_string();
    let sandbox_url = format!("https://{sandbox_id}.sandbox.tensorlake.ai");

    Ok(SandboxHandle {
        sandbox_url,
        sandbox_id,
        provider: "tensorlake".to_string(),
    })
}

fn tensorlake_health_check(
    ctx: &Context,
    api_key: &str,
    sandbox_url: &str,
) -> Result<bool, String> {
    let health_url = format!("{sandbox_url}/api/v1/files/list?path=/");
    let headers = bearer_headers(api_key);
    match ctx.http_call("GET", &health_url, &headers, "") {
        Ok(r) if r.status >= 200 && r.status < 300 => Ok(true),
        Ok(_) => Ok(false),
        Err(e) => Err(format!("Tensorlake health check failed: {e}")),
    }
}

fn tensorlake_file_read(
    ctx: &Context,
    api_key: &str,
    sandbox_url: &str,
    path: &str,
) -> Result<String, String> {
    let url = format!("{sandbox_url}/api/v1/files?path={}", url_encode(path));
    let resp = ctx.http_call("GET", &url, &bearer_headers(api_key), "")?;
    if resp.status >= 400 {
        return Err(format!("sandbox.read({path}): {}", resp.body));
    }
    Ok(resp.body)
}

fn tensorlake_file_write(
    ctx: &Context,
    api_key: &str,
    sandbox_url: &str,
    path: &str,
    content: &str,
) -> Result<(), String> {
    let url = format!("{sandbox_url}/api/v1/files?path={}", url_encode(path));
    let resp = ctx.http_call("PUT", &url, &bearer_headers(api_key), content)?;
    if resp.status >= 400 {
        return Err(format!("sandbox.write({path}): {}", resp.body));
    }
    Ok(())
}

fn tensorlake_file_delete(
    ctx: &Context,
    api_key: &str,
    sandbox_url: &str,
    path: &str,
) -> Result<(), String> {
    let url = format!("{sandbox_url}/api/v1/files?path={}", url_encode(path));
    let _ = ctx.http_call("DELETE", &url, &bearer_headers(api_key), "");
    Ok(())
}

fn tensorlake_exec(
    ctx: &Context,
    api_key: &str,
    sandbox_url: &str,
    command: &str,
    _workdir: &str,
) -> Result<ExecResult, String> {
    let run_id = unique_run_id();
    let out_file = format!("/tmp/.paw-out-{run_id}");
    let err_file = format!("/tmp/.paw-err-{run_id}");
    let rc_file = format!("/tmp/.paw-rc-{run_id}");

    let wrapped = format!("({command}) > {out_file} 2> {err_file}; echo $? > {rc_file}");

    // Start process via Tensorlake data plane
    let body = json!({
        "command": "/bin/bash",
        "args": ["-c", &wrapped],
    });
    let resp = ctx.http_call(
        "POST",
        &format!("{sandbox_url}/api/v1/processes"),
        &bearer_headers_json(api_key),
        &body.to_string(),
    )?;
    if resp.status >= 400 {
        return Err(format!("sandbox.bash(): start failed: {}", resp.body));
    }

    // Poll for exit code file (network latency provides natural backoff)
    let headers = bearer_headers(api_key);
    let rc_url = format!("{sandbox_url}/api/v1/files?path={}", url_encode(&rc_file));
    let mut exit_code: i64 = -1;
    let mut found = false;
    for _ in 0..600 {
        if let Ok(r) = ctx.http_call("GET", &rc_url, &headers, "") {
            if r.status >= 200 && r.status < 300 {
                exit_code = r.body.trim().parse::<i64>().unwrap_or(-1);
                found = true;
                break;
            }
        }
    }

    if !found {
        return Err("sandbox.bash(): process timed out".to_string());
    }

    // Read stdout and stderr
    let stdout = ctx
        .http_call(
            "GET",
            &format!("{sandbox_url}/api/v1/files?path={}", url_encode(&out_file)),
            &headers,
            "",
        )
        .map(|r| r.body)
        .unwrap_or_default();
    let stderr = ctx
        .http_call(
            "GET",
            &format!("{sandbox_url}/api/v1/files?path={}", url_encode(&err_file)),
            &headers,
            "",
        )
        .map(|r| r.body)
        .unwrap_or_default();

    // Cleanup temp files (best effort)
    for f in [&out_file, &err_file, &rc_file] {
        let _ = ctx.http_call(
            "DELETE",
            &format!("{sandbox_url}/api/v1/files?path={}", url_encode(f)),
            &headers,
            "",
        );
    }

    Ok(ExecResult {
        stdout,
        stderr,
        exit_code,
    })
}

// ===========================================================================
// Modal provider (calls REST bridge deployed on Modal)
// ===========================================================================

/// Resolve the Modal bridge base URL from config. This is the common prefix
/// of the per-endpoint URLs, e.g. `https://user--openpaw-sandbox-bridge`.
/// Each endpoint appends its label suffix: `-create.modal.run`, `-exec.modal.run`, etc.
fn modal_base_url(ctx: &Context) -> String {
    ctx.config
        .get("modal_api_url")
        .filter(|s| !s.is_empty() && !is_unresolved_secret(s))
        .cloned()
        .unwrap_or_else(|| "https://openpaw-sandbox--bridge".to_string())
}

/// Build a Modal bridge endpoint URL with auth as query parameter.
fn modal_url(base: &str, endpoint: &str, api_key: &str, extra_params: &str) -> String {
    let auth = url_encode(&format!("Bearer {api_key}"));
    if extra_params.is_empty() {
        format!("{base}-{endpoint}.modal.run?authorization={auth}")
    } else {
        format!("{base}-{endpoint}.modal.run?{extra_params}&authorization={auth}")
    }
}

fn modal_create(
    ctx: &Context,
    api_key: &str,
    config: &SandboxConfig,
) -> Result<SandboxHandle, String> {
    let base = modal_base_url(ctx);
    let url = modal_url(&base, "create", api_key, "");
    let headers = vec![("content-type".to_string(), "application/json".to_string())];
    let body = json!({
        "cpus": config.cpus,
        "memory_mb": config.memory_mb,
        "timeout_seconds": config.timeout_seconds
    });

    let resp = ctx.http_call("POST", &url, &headers, &body.to_string())?;
    if resp.status < 200 || resp.status >= 300 {
        return Err(format!(
            "Modal sandbox creation failed (HTTP {}): {}",
            resp.status,
            &resp.body[..resp.body.len().min(500)]
        ));
    }

    let parsed: Value = serde_json::from_str(&resp.body)
        .map_err(|e| format!("failed to parse Modal bridge response: {e}"))?;
    let sandbox_id = parsed
        .get("sandbox_id")
        .and_then(|v| v.as_str())
        .unwrap_or("modal-sandbox")
        .to_string();

    Ok(SandboxHandle {
        sandbox_url: base.clone(),
        sandbox_id,
        provider: "modal".to_string(),
    })
}

fn modal_health_check(
    ctx: &Context,
    api_key: &str,
    sandbox_id: &str,
) -> Result<bool, String> {
    let base = modal_base_url(ctx);
    let params = format!("sandbox_id={}", url_encode(sandbox_id));
    let url = modal_url(&base, "health", api_key, &params);
    match ctx.http_call("GET", &url, &[], "") {
        Ok(r) if r.status >= 200 && r.status < 300 => {
            let parsed: Value = serde_json::from_str(&r.body).unwrap_or(json!({}));
            Ok(parsed.get("ready").and_then(|v| v.as_bool()).unwrap_or(false))
        }
        Ok(_) => Ok(false),
        Err(e) => Err(format!("Modal health check failed: {e}")),
    }
}

fn modal_file_read(
    ctx: &Context,
    api_key: &str,
    sandbox_id: &str,
    path: &str,
) -> Result<String, String> {
    let base = modal_base_url(ctx);
    let params = format!("sandbox_id={}&path={}", url_encode(sandbox_id), url_encode(path));
    let url = modal_url(&base, "file-read", api_key, &params);
    let resp = ctx.http_call("GET", &url, &[], "")?;
    if resp.status >= 400 {
        return Err(format!("sandbox.read({path}): {}", resp.body));
    }
    let parsed: Value = serde_json::from_str(&resp.body).unwrap_or(json!({}));
    Ok(parsed.get("content").and_then(|v| v.as_str()).unwrap_or("").to_string())
}

fn modal_file_write(
    ctx: &Context,
    api_key: &str,
    sandbox_id: &str,
    path: &str,
    content: &str,
) -> Result<(), String> {
    if let Some(parent) = Path::new(path)
        .parent()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty() && *value != ".")
    {
        modal_ensure_dir(ctx, api_key, sandbox_id, parent).map_err(|e| {
            format!("sandbox.write({path}): failed to create parent directory {parent}: {e}")
        })?;
    }

    let base = modal_base_url(ctx);
    let url = modal_url(&base, "file-write", api_key, "");
    let headers = vec![("content-type".to_string(), "application/json".to_string())];
    let body = json!({
        "sandbox_id": sandbox_id,
        "path": path,
        "content": content
    });
    let resp = ctx.http_call("POST", &url, &headers, &body.to_string())?;
    if resp.status >= 400 {
        return Err(format!("sandbox.write({path}): {}", resp.body));
    }
    Ok(())
}

fn modal_file_delete(
    ctx: &Context,
    api_key: &str,
    sandbox_id: &str,
    path: &str,
) -> Result<(), String> {
    let base = modal_base_url(ctx);
    let params = format!("sandbox_id={}&path={}", url_encode(sandbox_id), url_encode(path));
    let url = modal_url(&base, "file-delete", api_key, &params);
    let _ = ctx.http_call("DELETE", &url, &[], "");
    Ok(())
}

fn modal_exec(
    ctx: &Context,
    api_key: &str,
    sandbox_id: &str,
    command: &str,
    workdir: &str,
) -> Result<ExecResult, String> {
    let effective_workdir = if workdir.trim().is_empty() { "/" } else { workdir };
    modal_ensure_dir(ctx, api_key, sandbox_id, effective_workdir)
        .map_err(|e| format!("sandbox.exec: failed to prepare workdir {effective_workdir}: {e}"))?;

    let prepared_command = format!(
        "cd {} && {}",
        shell_single_quote(effective_workdir),
        command
    );

    modal_exec_raw(ctx, api_key, sandbox_id, &prepared_command, "/")
}

fn modal_exec_raw(
    ctx: &Context,
    api_key: &str,
    sandbox_id: &str,
    command: &str,
    workdir: &str,
) -> Result<ExecResult, String> {
    let base = modal_base_url(ctx);
    let url = modal_url(&base, "exec", api_key, "");
    let headers = vec![("content-type".to_string(), "application/json".to_string())];
    let body = json!({
        "sandbox_id": sandbox_id,
        "command": command,
        "workdir": workdir
    });

    let resp = ctx.http_call("POST", &url, &headers, &body.to_string())?;
    if resp.status < 200 || resp.status >= 300 {
        return Err(format!(
            "Modal exec failed (HTTP {}): {}",
            resp.status,
            &resp.body[..resp.body.len().min(500)]
        ));
    }

    let parsed: Value = serde_json::from_str(&resp.body)
        .map_err(|e| format!("failed to parse Modal exec response: {e}"))?;

    Ok(ExecResult {
        stdout: parsed
            .get("stdout")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        stderr: parsed
            .get("stderr")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        exit_code: parsed
            .get("exit_code")
            .and_then(|v| v.as_i64())
            .unwrap_or(-1),
    })
}

fn modal_ensure_dir(
    ctx: &Context,
    api_key: &str,
    sandbox_id: &str,
    dir: &str,
) -> Result<(), String> {
    let result = modal_exec_raw(
        ctx,
        api_key,
        sandbox_id,
        &format!("mkdir -p {}", shell_single_quote(dir)),
        "/",
    )?;
    if result.exit_code != 0 {
        let stderr = result.stderr.trim();
        let detail = if stderr.is_empty() {
            format!("exit {}", result.exit_code)
        } else {
            format!("exit {}: {}", result.exit_code, stderr)
        };
        return Err(detail);
    }
    Ok(())
}

fn shell_single_quote(value: &str) -> String {
    if value.is_empty() {
        return "''".to_string();
    }
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_sandbox_provider() {
        assert_eq!(normalize_sandbox_provider("tensorlake"), "tensorlake");
        assert_eq!(normalize_sandbox_provider("TensorLake"), "tensorlake");
        assert_eq!(normalize_sandbox_provider("tl"), "tensorlake");
        assert_eq!(normalize_sandbox_provider("TL"), "tensorlake");
        assert_eq!(normalize_sandbox_provider("modal"), "modal");
        assert_eq!(normalize_sandbox_provider("Modal"), "modal");
        assert_eq!(normalize_sandbox_provider(" modal "), "modal");
        assert_eq!(normalize_sandbox_provider("unknown"), "unknown");
    }

    #[test]
    fn test_url_encode() {
        assert_eq!(url_encode("/tmp/foo.txt"), "/tmp/foo.txt");
        assert_eq!(url_encode("/tmp/foo bar.txt"), "/tmp/foo%20bar.txt");
        assert_eq!(url_encode("hello world"), "hello%20world");
        assert_eq!(url_encode("a=b&c=d"), "a%3Db%26c%3Dd");
    }

    #[test]
    fn test_is_unresolved_secret() {
        assert!(is_unresolved_secret("{secret:tensorlake_api_key}"));
        assert!(!is_unresolved_secret("sk-abc123"));
        assert!(!is_unresolved_secret(""));
    }

    #[test]
    fn test_first_non_empty() {
        assert_eq!(first_non_empty(&[None, Some("abc".into())]), "abc");
        assert_eq!(first_non_empty(&[Some("".into()), Some("def".into())]), "def");
        assert_eq!(
            first_non_empty(&[Some("{secret:key}".into()), Some("real".into())]),
            "real"
        );
        assert_eq!(first_non_empty(&[None, None]), "");
    }

    #[test]
    fn test_shell_single_quote() {
        assert_eq!(shell_single_quote("/workspace"), "'/workspace'");
        assert_eq!(shell_single_quote(""), "''");
        assert_eq!(shell_single_quote("it's fine"), "'it'\"'\"'s fine'");
    }
}
