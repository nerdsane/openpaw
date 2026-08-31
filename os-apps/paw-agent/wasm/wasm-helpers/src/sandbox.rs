//! Sandbox provider abstraction — match-based dispatch for sandbox lifecycle,
//! file I/O, and command execution across multiple providers (Tensorlake, Modal).
//!
//! Follows the same pattern as the staged provider caller's provider selection:
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
    pub networking_type: String,
    pub allowed_hosts: Vec<String>,
    pub allow_mcp_servers: bool,
    pub allow_package_managers: bool,
    pub packages: Vec<SandboxPackage>,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            cpus: 2,
            memory_mb: 4096,
            timeout_seconds: 3600,
            internet_access: true,
            networking_type: String::new(),
            allowed_hosts: Vec::new(),
            allow_mcp_servers: false,
            allow_package_managers: false,
            packages: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SandboxPackage {
    pub manager: String,
    pub name: String,
    pub version: String,
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
            ctx.config.get("modal_token_id").cloned(),
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

pub fn sandbox_config_from_fields(fields: &Value) -> SandboxConfig {
    let networking_type = entity_field_str(
        fields,
        &["sandbox_networking_type", "SandboxNetworkingType"],
    )
    .unwrap_or("")
    .to_string();
    let allowed_hosts = entity_field_str(
        fields,
        &["sandbox_allowed_hosts_json", "SandboxAllowedHostsJson"],
    )
    .and_then(|raw| serde_json::from_str::<Vec<String>>(raw).ok())
    .unwrap_or_default();
    let allow_mcp_servers = entity_field_bool(
        fields,
        &["sandbox_allow_mcp_servers", "SandboxAllowMcpServers"],
    );
    let allow_package_managers = entity_field_bool(
        fields,
        &[
            "sandbox_allow_package_managers",
            "SandboxAllowPackageManagers",
        ],
    );
    let packages = entity_field_str(fields, &["sandbox_packages_json", "SandboxPackagesJson"])
        .and_then(|raw| serde_json::from_str::<Vec<Value>>(raw).ok())
        .unwrap_or_default()
        .into_iter()
        .map(|package| SandboxPackage {
            manager: package
                .get("manager")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            name: package
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
            version: package
                .get("version")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
        })
        .filter(|package| !package.manager.is_empty() && !package.name.is_empty())
        .collect::<Vec<_>>();

    SandboxConfig {
        internet_access: !matches!(
            networking_type.trim().to_ascii_lowercase().as_str(),
            "disabled"
        ),
        networking_type,
        allowed_hosts,
        allow_mcp_servers,
        allow_package_managers,
        packages,
        ..SandboxConfig::default()
    }
}

fn sandbox_policy_payload(config: &SandboxConfig) -> Value {
    json!({
        "networking_type": config.networking_type,
        "allowed_hosts": config.allowed_hosts,
        "allow_mcp_servers": config.allow_mcp_servers,
        "allow_package_managers": config.allow_package_managers,
        "packages": config
            .packages
            .iter()
            .map(|package| {
                json!({
                    "manager": package.manager,
                    "name": package.name,
                    "version": package.version,
                })
            })
            .collect::<Vec<_>>(),
    })
}

fn tensorlake_create_body(config: &SandboxConfig) -> Value {
    let mut body = json!({
        "resources": {
            "cpus": config.cpus,
            "memory_mb": config.memory_mb
        },
        "timeout_seconds": config.timeout_seconds,
        "internet_access": config.internet_access
    });

    body["network"] = json!({
        "allow_internet_access": config.internet_access,
        "allow_out": config.allowed_hosts,
    });

    body
}

fn modal_create_body(config: &SandboxConfig) -> Value {
    let mut body = json!({
        "cpus": config.cpus,
        "memory_mb": config.memory_mb,
        "timeout_seconds": config.timeout_seconds,
    });

    if let Some(object) = body.as_object_mut() {
        let policy = sandbox_policy_payload(config);
        if let Some(policy_object) = policy.as_object() {
            for (key, value) in policy_object {
                object.insert(key.clone(), value.clone());
            }
        }
    }

    body
}

fn sandbox_policy_file_contents(config: &SandboxConfig) -> String {
    serde_json::to_string_pretty(&sandbox_policy_payload(config))
        .unwrap_or_else(|_| "{}".to_string())
}

fn package_spec(package: &SandboxPackage, separator: &str) -> String {
    if package.version.trim().is_empty() {
        package.name.clone()
    } else {
        format!("{}{}{}", package.name, separator, package.version)
    }
}

fn sandbox_package_install_script(config: &SandboxConfig) -> Option<String> {
    if config.packages.is_empty() {
        return None;
    }

    let apt_packages = config
        .packages
        .iter()
        .filter(|package| package.manager.eq_ignore_ascii_case("apt"))
        .map(|package| package_spec(package, "="))
        .collect::<Vec<_>>();
    let pip_packages = config
        .packages
        .iter()
        .filter(|package| package.manager.eq_ignore_ascii_case("pip"))
        .map(|package| package_spec(package, "=="))
        .collect::<Vec<_>>();
    let npm_packages = config
        .packages
        .iter()
        .filter(|package| package.manager.eq_ignore_ascii_case("npm"))
        .map(|package| package_spec(package, "@"))
        .collect::<Vec<_>>();

    let mut commands = Vec::new();

    if !apt_packages.is_empty() {
        commands.push(format!(
            "apt-get update && apt-get install -y --no-install-recommends {}",
            apt_packages.join(" ")
        ));
    }

    if !pip_packages.is_empty() {
        commands.push(format!(
            "python3 -m ensurepip --upgrade >/dev/null 2>&1 || true\npython3 -m pip install --disable-pip-version-check --no-input {}",
            pip_packages.join(" ")
        ));
    }

    if !npm_packages.is_empty() {
        commands.push(format!(
            "command -v npm >/dev/null 2>&1 || (apt-get update && apt-get install -y --no-install-recommends nodejs npm)\nnpm install -g {}",
            npm_packages.join(" ")
        ));
    }

    if commands.is_empty() {
        return None;
    }

    Some(format!("set -e\n{}", commands.join("\n")))
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
    let result = match provider {
        "tensorlake" => tensorlake_create(ctx, &api_key, config),
        "modal" => modal_create(ctx, &api_key, config),
        other => Err(format!("unsupported sandbox provider: {other}")),
    };
    match &result {
        Ok(handle) => log_sandbox_observability(
            ctx,
            provider,
            "create",
            "success",
            &handle.sandbox_id,
            None,
            None,
            "",
        ),
        Err(_) => {
            log_sandbox_observability(ctx, provider, "create", "error", "", None, None, "");
        }
    }
    result
}

/// Copy a running sandbox via the provider's live-copy API, returning the new
/// sandbox's handle. The copy reproduces the source exactly (image, logins,
/// files) — the review panel uses this to spawn a per-review child of arni-big.
pub fn sandbox_copy(
    ctx: &Context,
    provider: &str,
    source_sandbox_id: &str,
    ready_timeout_secs: u64,
) -> Result<SandboxHandle, String> {
    let api_key = resolve_sandbox_api_key(ctx, provider)?;
    let result = match provider {
        "tensorlake" => tensorlake_copy(ctx, &api_key, source_sandbox_id, ready_timeout_secs),
        other => Err(format!("sandbox_copy unsupported for provider: {other}")),
    };
    match &result {
        Ok(h) => log_sandbox_observability(
            ctx, provider, "copy", "success", &h.sandbox_id, None, None, source_sandbox_id,
        ),
        Err(_) => log_sandbox_observability(
            ctx, provider, "copy", "error", "", None, None, source_sandbox_id,
        ),
    }
    result
}

/// Terminate a sandbox (best-effort teardown). Idempotent: an already-gone
/// sandbox (404) is treated as success, so reaping never fails on a race.
pub fn sandbox_terminate(ctx: &Context, provider: &str, sandbox_id: &str) -> Result<(), String> {
    let api_key = resolve_sandbox_api_key(ctx, provider)?;
    let result = match provider {
        "tensorlake" => tensorlake_terminate(ctx, &api_key, sandbox_id),
        other => Err(format!("sandbox_terminate unsupported for provider: {other}")),
    };
    match &result {
        Ok(()) => log_sandbox_observability(
            ctx, provider, "terminate", "success", sandbox_id, None, None, "",
        ),
        Err(_) => log_sandbox_observability(
            ctx, provider, "terminate", "error", sandbox_id, None, None, "",
        ),
    }
    result
}

/// Start a command ASYNCHRONOUSLY (ARN-443 D): POST the process and return the
/// run_id used to build the `/tmp/.paw-{out,err,rc}-<run_id>` capture files. Does
/// NOT poll — the caller drives completion via [`sandbox_exec_poll`] across
/// separate WASM invocations, so a command may outlive a single 120s invocation
/// (the synchronous [`sandbox_exec`] cannot).
/// `run_id` is supplied by the caller (not minted here) so it is STABLE across
/// retries of the same logical start: the launch is idempotent (guarded by an
/// `rc`-file check and a `flock`), so a retried `Run` trigger cannot spawn a
/// second process. Callers derive it deterministically from the Exec row id — one
/// exec per row, so per-dispatch randomness is neither needed nor wanted here.
pub fn sandbox_exec_start(
    ctx: &Context,
    handle: &SandboxHandle,
    command: &str,
    run_id: &str,
    tail_bytes: usize,
) -> Result<(), String> {
    let api_key = resolve_sandbox_api_key(ctx, &handle.provider)?;
    let result = match handle.provider.as_str() {
        "tensorlake" => {
            tensorlake_exec_start(ctx, &api_key, &handle.sandbox_url, command, run_id, tail_bytes)
        }
        other => Err(format!("async exec unsupported for provider: {other}")),
    };
    let outcome = if result.is_ok() { "started" } else { "error" };
    log_sandbox_observability(
        ctx, &handle.provider, "exec_start", outcome, &handle.sandbox_id, None, None, "",
    );
    result
}

/// A run_id derived deterministically from the entity id (no per-dispatch
/// randomness): stable across trigger retries so the idempotent launch dedups a
/// re-fired start. Unique per Exec row because the entity id is.
pub fn deterministic_run_id(entity_id: &str) -> String {
    capture_run_id(entity_id, 0)
}

/// Poll a command started by [`sandbox_exec_start`]. `Some(result)` once the rc
/// file exists (the command finished); `None` while it is still running.
///
/// `tail_bytes` bounds how much stdout/stderr is pulled OFF the sandbox: only the
/// last `tail_bytes` of each stream are fetched (HTTP Range), so a multi-gigabyte
/// output can never be read whole into WASM memory. The capture files are NOT
/// deleted here — they are ephemeral on the sandbox and die with it when the copy
/// is Destroyed, which avoids a delete-before-the-result-commits window (a crash
/// between delete and the terminal callback would otherwise lose the result).
pub fn sandbox_exec_poll(
    ctx: &Context,
    handle: &SandboxHandle,
    run_id: &str,
    tail_bytes: usize,
) -> Result<Option<ExecResult>, String> {
    let api_key = resolve_sandbox_api_key(ctx, &handle.provider)?;
    match handle.provider.as_str() {
        "tensorlake" => tensorlake_exec_poll(ctx, &api_key, &handle.sandbox_url, run_id, tail_bytes),
        other => Err(format!("async exec unsupported for provider: {other}")),
    }
}

/// The `/tmp/.paw-{out,err,rc}-<run_id>` capture file paths for a run.
fn paw_capture_files(run_id: &str) -> (String, String, String) {
    (
        format!("/tmp/.paw-out-{run_id}"),
        format!("/tmp/.paw-err-{run_id}"),
        format!("/tmp/.paw-rc-{run_id}"),
    )
}

/// Check if a sandbox is ready to accept commands.
pub fn sandbox_health_check(ctx: &Context, handle: &SandboxHandle) -> Result<bool, String> {
    let api_key = resolve_sandbox_api_key(ctx, &handle.provider)?;
    let result = match handle.provider.as_str() {
        "tensorlake" => tensorlake_health_check(ctx, &api_key, &handle.sandbox_url),
        "modal" => modal_health_check(ctx, &api_key, &handle.sandbox_id),
        other => Err(format!("unsupported sandbox provider: {other}")),
    };
    match &result {
        Ok(true) => log_sandbox_observability(
            ctx,
            &handle.provider,
            "health",
            "ready",
            &handle.sandbox_id,
            None,
            Some(200),
            "",
        ),
        Ok(false) => log_sandbox_observability(
            ctx,
            &handle.provider,
            "health",
            "not_ready",
            &handle.sandbox_id,
            None,
            None,
            "",
        ),
        Err(_) => log_sandbox_observability(
            ctx,
            &handle.provider,
            "health",
            "error",
            &handle.sandbox_id,
            None,
            None,
            "",
        ),
    }
    result
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
    let result = match handle.provider.as_str() {
        "tensorlake" => tensorlake_file_read(ctx, &api_key, &handle.sandbox_url, path),
        "modal" => modal_file_read(ctx, &api_key, &handle.sandbox_id, path),
        other => Err(format!("unsupported sandbox provider: {other}")),
    };
    log_sandbox_observability(
        ctx,
        &handle.provider,
        "read",
        if result.is_ok() { "success" } else { "error" },
        &handle.sandbox_id,
        None,
        None,
        path,
    );
    result
}

/// Write a file to the sandbox.
pub fn sandbox_file_write(
    ctx: &Context,
    handle: &SandboxHandle,
    path: &str,
    content: &str,
) -> Result<(), String> {
    let api_key = resolve_sandbox_api_key(ctx, &handle.provider)?;
    let result = match handle.provider.as_str() {
        "tensorlake" => tensorlake_file_write(ctx, &api_key, &handle.sandbox_url, path, content),
        "modal" => modal_file_write(ctx, &api_key, &handle.sandbox_id, path, content),
        other => Err(format!("unsupported sandbox provider: {other}")),
    };
    log_sandbox_observability(
        ctx,
        &handle.provider,
        "write",
        if result.is_ok() { "success" } else { "error" },
        &handle.sandbox_id,
        None,
        None,
        path,
    );
    result
}

/// Delete a file from the sandbox.
pub fn sandbox_file_delete(
    ctx: &Context,
    handle: &SandboxHandle,
    path: &str,
) -> Result<(), String> {
    let api_key = resolve_sandbox_api_key(ctx, &handle.provider)?;
    let result = match handle.provider.as_str() {
        "tensorlake" => tensorlake_file_delete(ctx, &api_key, &handle.sandbox_url, path),
        "modal" => modal_file_delete(ctx, &api_key, &handle.sandbox_id, path),
        other => Err(format!("unsupported sandbox provider: {other}")),
    };
    log_sandbox_observability(
        ctx,
        &handle.provider,
        "delete",
        if result.is_ok() { "success" } else { "error" },
        &handle.sandbox_id,
        None,
        None,
        path,
    );
    result
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
    let result = match handle.provider.as_str() {
        "tensorlake" => tensorlake_exec(ctx, &api_key, &handle.sandbox_url, command, workdir),
        "modal" => modal_exec(ctx, &api_key, &handle.sandbox_id, command, workdir),
        other => Err(format!("unsupported sandbox provider: {other}")),
    };
    let exit_code = result.as_ref().ok().map(|result| result.exit_code);
    let outcome = match exit_code {
        Some(0) => "success",
        Some(_) => "nonzero_exit",
        None => "error",
    };
    log_sandbox_observability(
        ctx,
        &handle.provider,
        "bash",
        outcome,
        &handle.sandbox_id,
        exit_code,
        None,
        workdir,
    );
    result
}

// ---------------------------------------------------------------------------
// Post-provisioning setup
// ---------------------------------------------------------------------------

/// Run post-provisioning setup (gh CLI etc.). Non-fatal — logs warnings on failure.
pub fn sandbox_setup(ctx: &Context, handle: &SandboxHandle) {
    if handle.sandbox_url.is_empty() {
        return;
    }

    let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));
    let config = sandbox_config_from_fields(&fields);
    let has_policy = !config.networking_type.trim().is_empty()
        || !config.allowed_hosts.is_empty()
        || config.allow_mcp_servers
        || config.allow_package_managers
        || !config.packages.is_empty();

    if has_policy {
        let policy_file = "/workspace/.temperpaw-sandbox-config.json";
        match sandbox_file_write(
            ctx,
            handle,
            policy_file,
            &sandbox_policy_file_contents(&config),
        ) {
            Ok(()) => ctx.log(
                "info",
                &format!("sandbox_setup: wrote sandbox policy file to {policy_file}"),
            ),
            Err(e) => ctx.log(
                "warn",
                &format!("sandbox_setup: failed to persist sandbox policy file: {e}"),
            ),
        }
    }

    if let Some(install_script) = sandbox_package_install_script(&config) {
        match sandbox_exec(ctx, handle, &install_script, "/") {
            Ok(result) if result.exit_code == 0 => ctx.log(
                "info",
                &format!(
                    "sandbox_setup: installed {} requested package(s)",
                    config.packages.len()
                ),
            ),
            Ok(result) => ctx.log(
                "warn",
                &format!(
                    "sandbox_setup: package installation exited {}: {}",
                    result.exit_code,
                    result.stderr.trim()
                ),
            ),
            Err(e) => ctx.log(
                "warn",
                &format!("sandbox_setup: package installation failed: {e}"),
            ),
        }
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
                &format!(
                    "sandbox_setup: gh CLI setup completed (exit {})",
                    result.exit_code
                ),
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

fn log_sandbox_observability(
    ctx: &Context,
    provider: &str,
    operation: &str,
    outcome: &str,
    sandbox_id: &str,
    exit_code: Option<i64>,
    status_code: Option<i64>,
    workdir: &str,
) {
    let fields = sandbox_observability_fields(
        provider,
        operation,
        outcome,
        sandbox_id,
        exit_code,
        status_code,
        workdir,
    );
    let _ = ctx.log_structured("info", "temperpaw.sandbox operation", &fields);
}

fn sandbox_observability_fields(
    provider: &str,
    operation: &str,
    outcome: &str,
    sandbox_id: &str,
    exit_code: Option<i64>,
    status_code: Option<i64>,
    workdir: &str,
) -> Value {
    json!({
        "observability_event": "temperpaw.sandbox",
        "sandbox_provider": provider,
        "sandbox_id": sandbox_id,
        "sandbox": {
            "operation": operation,
            "outcome": outcome,
            "backend": provider,
            "exit_code": exit_code.unwrap_or(-1),
            "status_code": status_code.unwrap_or(0),
            "workdir": workdir,
        }
    })
}

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

fn entity_field_bool(fields: &Value, keys: &[&str]) -> bool {
    for key in keys {
        if let Some(raw) = fields.get(*key) {
            if let Some(boolean) = raw.as_bool() {
                return boolean;
            }
            if let Some(text) = raw.as_str() {
                match text.trim().to_ascii_lowercase().as_str() {
                    "true" => return true,
                    "false" => return false,
                    _ => {}
                }
            }
        }
    }
    if let Some(nested) = fields.get("fields") {
        return entity_field_bool(nested, keys);
    }
    false
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

/// A capture id that is unique across concurrent exec invocations.
///
/// The temp files `/tmp/.paw-{out,err,rc}-<id>` must not collide between two
/// execs running on the same sandbox at once. Earlier schemes could not
/// guarantee this: a process-local `AtomicU32` reset to 0 on every fresh WASM
/// instance, and even entity id + host clock + counter collided for two execs
/// of the SAME entity in the same millisecond (both fresh instances, same clock,
/// counter 0) — and a long-running exec can overlap another dispatch for the
/// same entity. So uniqueness comes from a per-dispatch random u64 instead; the
/// entity id is kept only as a readable label. (ARN-401)
fn unique_run_id(ctx: &Context) -> Result<String, String> {
    Ok(capture_run_id(&ctx.entity_id, random_u64()?))
}

/// A random u64 for per-dispatch uniqueness — the sole uniqueness source for the
/// capture id (see `capture_run_id`). On wasm32-wasip1 this is WASI `random_get`
/// (the temper host wires `wasi_snapshot_preview1`); on native it is the OS RNG.
///
/// If the host RNG is unavailable we FAIL — never a fallback id. A non-random
/// fallback (a heap address, a clock) can repeat across calls or instances, which
/// is exactly the capture-file collision this whole change exists to prevent; and
/// `random_get` erroring is already an abnormal host state. An exec that cannot
/// obtain randomness must abort, not risk crossing another exec's output.
fn random_u64() -> Result<u64, String> {
    let mut b = [0u8; 8];
    fill_random(&mut b)?;
    Ok(u64::from_le_bytes(b))
}

/// Fill `buf` with random bytes.
///
/// On wasm this is a RAW WASI `random_get` import — NOT the `getrandom` crate.
/// The crate's `wasm32-unknown-unknown` support is a hard `compile_error!`, which
/// broke every wasm-helpers consumer still built for that target (unrelated
/// paw-agent/foresight/ingest modules) even though they never draw randomness.
/// A raw import is dead-code-eliminated when unused, so those modules compile
/// again, and the temper host provides `random_get` for wasip1 modules.
#[cfg(target_arch = "wasm32")]
fn fill_random(buf: &mut [u8]) -> Result<(), String> {
    #[link(wasm_import_module = "wasi_snapshot_preview1")]
    unsafe extern "C" {
        fn random_get(buf: *mut u8, buf_len: usize) -> u16;
    }
    let rc = unsafe { random_get(buf.as_mut_ptr(), buf.len()) };
    if rc == 0 {
        Ok(())
    } else {
        Err(format!("host RNG unavailable (random_get errno {rc})"))
    }
}

/// Native (unit tests only): a std-seeded fill. `random_u64` is not exercised by
/// the pure `capture_run_id` tests, so this only needs to be non-panicking.
#[cfg(not(target_arch = "wasm32"))]
fn fill_random(buf: &mut [u8]) -> Result<(), String> {
    use std::hash::{BuildHasher, Hasher};
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_usize(buf.as_ptr() as usize);
    let mut v = h.finish();
    for chunk in buf.chunks_mut(8) {
        let bytes = v.to_le_bytes();
        chunk.copy_from_slice(&bytes[..chunk.len()]);
        v = v.wrapping_mul(6364136223846793005).wrapping_add(1);
    }
    Ok(())
}

/// FNV-1a 32-bit hash — a small, dependency-free digest of the full entity id so
/// the bounded (truncated) entity segment stays distinguishing even for long ids.
fn fnv1a_32(bytes: &[u8]) -> u32 {
    let mut h: u32 = 0x811c_9dc5;
    for &b in bytes {
        h ^= b as u32;
        h = h.wrapping_mul(0x0100_0193);
    }
    h
}

/// Build the capture id for `/tmp/.paw-{out,err,rc}-<id>`. Pure (testable).
///
/// Uniqueness comes from `rand`, a random u64 drawn once per dispatch — so two
/// execs never share capture files even when they run for the SAME entity in the
/// same instant (the ARN-401 class of collision, which a wall-clock + per-
/// instance counter could not close: every trigger dispatch is a fresh WASM
/// instance, so the counter resets to 0). No wall-clock is read, keeping the
/// module free of ambient time (DST discipline).
///
/// The entity id is kept only as a human-readable, filename-safe label:
/// - encoded injectively (alphanumerics and `-` pass through; every other byte,
///   `_` included, becomes `_` + two hex digits — so `_` only ever marks an
///   escape and distinct ids never alias, e.g. `a/b` vs `a?b`);
/// - bounded to 32 encoded chars plus an 8-hex FNV hash of the FULL id, so the
///   filename can never exceed the filesystem's per-component limit while the
///   segment stays distinguishing;
/// - prefixed with `e` so an empty id (encoded segment "") cannot alias any
///   real id.
fn capture_run_id(entity_id: &str, rand: u64) -> String {
    let mut enc = String::with_capacity(entity_id.len());
    for b in entity_id.bytes() {
        if b.is_ascii_alphanumeric() || b == b'-' {
            enc.push(b as char);
        } else {
            enc.push('_');
            enc.push_str(&format!("{b:02x}"));
        }
    }
    let head: String = enc.chars().take(32).collect();
    let hash = fnv1a_32(entity_id.as_bytes());
    format!("e{head}-{hash:08x}-{rand:016x}")
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
    let body = tensorlake_create_body(config);

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

fn tensorlake_copy(
    ctx: &Context,
    api_key: &str,
    source_sandbox_id: &str,
    ready_timeout_secs: u64,
) -> Result<SandboxHandle, String> {
    // Server live-copy API: POST /sandboxes/{source}/copy.
    let copy_url = format!("https://api.tensorlake.ai/sandboxes/{source_sandbox_id}/copy");
    let headers = bearer_headers_json(api_key);
    let body = json!({ "times": 1, "timeout_seconds": ready_timeout_secs });
    let resp = ctx.http_call("POST", &copy_url, &headers, &body.to_string())?;
    if resp.status < 200 || resp.status >= 300 {
        return Err(format!(
            "Tensorlake sandbox copy of {source_sandbox_id} failed (HTTP {}): {}",
            resp.status,
            &resp.body[..resp.body.len().min(500)]
        ));
    }
    let parsed: Value = serde_json::from_str(&resp.body)
        .map_err(|e| format!("failed to parse Tensorlake copy response: {e}"))?;
    let sandbox_id = extract_copied_sandbox_id(&parsed).ok_or_else(|| {
        format!(
            "Tensorlake copy returned no sandbox id: {}",
            &resp.body[..resp.body.len().min(300)]
        )
    })?;
    Ok(SandboxHandle {
        sandbox_url: format!("https://{sandbox_id}.sandbox.tensorlake.ai"),
        sandbox_id,
        provider: "tensorlake".to_string(),
    })
}

/// Pull the new sandbox id out of a copy response, tolerating shapes: a bare
/// `{sandbox_id|id}`, or `{sandboxes:[{sandbox_id|id}|"<id>", ...]}` (the batch
/// form). Returns the first id found.
fn extract_copied_sandbox_id(parsed: &Value) -> Option<String> {
    if let Some(s) = parsed.get("sandbox_id").and_then(|v| v.as_str()) {
        return Some(s.to_string());
    }
    if let Some(s) = parsed.get("id").and_then(|v| v.as_str()) {
        return Some(s.to_string());
    }
    if let Some(arr) = parsed.get("sandboxes").and_then(|v| v.as_array()) {
        for e in arr {
            if let Some(s) = e
                .get("sandbox_id")
                .or_else(|| e.get("id"))
                .and_then(|v| v.as_str())
            {
                return Some(s.to_string());
            }
            if let Some(s) = e.as_str() {
                return Some(s.to_string());
            }
        }
    }
    None
}

/// ASYNC copy start (ARN-443, fix after C5), with concurrency guards (added after
/// the outgoing lead flagged that name discovery could adopt a live panel's copy).
///
/// The tensorlake live-copy API is SYNCHRONOUS — POST /sandboxes/{src}/copy blocks
/// until the copy is fully provisioned (minutes) then returns — so it cannot finish
/// inside one ~120s WASM invocation. This INITIATES the copy with a short wait (the
/// copy is created server-side regardless of whether the response arrives), then
/// DISCOVERS the created copy and returns its handle WITHOUT waiting for readiness;
/// the caller polls readiness from the Copying state.
///
/// Discovery is by the provider's fixed single-copy name `<source>-copy`, which the
/// panel's raw copies ALSO use — so two guards keep a governed copy from ever
/// claiming a panel's (or anyone's) sandbox, which would let the lease reaper kill a
/// running review:
///  (2) PRECONDITION — refuse if an un-suffixed `<source>-copy` already exists
///      (the provider's fixed naming means only ONE anonymous copy can be in
///      flight; a second POST 409s), so a pre-existing one is never ours.
///  (1) DISCOVERY FILTER — the adopted sandbox must have been created AFTER our
///      POST (creation-time window) AND must not already be referenced by another
///      live Computer row (`claimed_ids`, passed by the caller).
/// `claimed_ids` are the machine_ids of live Computer rows; `now_ms` is the caller's
/// clock just before the POST. Returns Err on a definitive failure OR a "retry
/// later" precondition (a copy already in flight).
pub fn sandbox_copy_start(
    ctx: &Context,
    provider: &str,
    source_sandbox_id: &str,
    wait_secs: u64,
    claimed_ids: &[String],
) -> Result<SandboxHandle, String> {
    let api_key = resolve_sandbox_api_key(ctx, provider)?;
    let handle = match provider {
        "tensorlake" => {
            tensorlake_copy_start(ctx, &api_key, source_sandbox_id, wait_secs, claimed_ids)
        }
        other => Err(format!("sandbox_copy_start unsupported for provider: {other}")),
    };
    let outcome = if handle.is_ok() { "started" } else { "error" };
    log_sandbox_observability(
        ctx, provider, "copy_start", outcome,
        handle.as_ref().map(|h| h.sandbox_id.as_str()).unwrap_or(""),
        None, None, source_sandbox_id,
    );
    handle
}

/// Tolerance (ms) on the "created after our POST" window — absorbs clock skew
/// between the host and the provider and list-propagation lag, so a fresh copy of
/// ours is never wrongly rejected while a clearly-older sandbox still is.
const COPY_DISCOVERY_WINDOW_TOLERANCE_MS: i64 = 60_000;

fn tensorlake_copy_start(
    ctx: &Context,
    api_key: &str,
    source_sandbox_id: &str,
    wait_secs: u64,
    claimed_ids: &[String],
) -> Result<SandboxHandle, String> {
    // Resolve the source name (the copy is named "<source-name>-copy").
    let rows_before = tensorlake_list_sandboxes(ctx, api_key)?;
    let source_name = rows_before
        .iter()
        .find(|r| r.id == source_sandbox_id)
        .map(|r| r.name.clone())
        .filter(|n| !n.is_empty())
        .ok_or_else(|| format!("source sandbox {source_sandbox_id} not found / unnamed"))?;
    let want = format!("{source_name}-copy");

    // (2) Precondition: refuse if an un-suffixed <source>-copy already exists —
    // only one anonymous copy can be in flight, and a pre-existing one is not ours
    // (it may be a live panel copy). Retry later.
    if rows_before
        .iter()
        .any(|r| r.name == want && r.id != source_sandbox_id && !is_terminated_status(&r.status))
    {
        return Err(format!(
            "a copy '{want}' is already in flight; retry later (only one anonymous copy per source)"
        ));
    }

    // Initiate. Short wait creates the copy server-side; a 4xx is a definitive
    // no-copy failure, a 5xx/timeout means it was created (discover it).
    let post_ms = Context::get_time_millis();
    let copy_url = format!("https://api.tensorlake.ai/sandboxes/{source_sandbox_id}/copy");
    let body = json!({ "times": 1, "timeout_seconds": wait_secs });
    match ctx.http_call("POST", &copy_url, &bearer_headers_json(api_key), &body.to_string()) {
        Ok(r) if r.status >= 400 && r.status < 500 => {
            return Err(format!(
                "Tensorlake copy of {source_sandbox_id} rejected (HTTP {}): {}",
                r.status,
                &r.body[..r.body.len().min(300)]
            ));
        }
        _ => { /* 2xx (created), 5xx, gateway timeout, or http error: discover */ }
    }

    // (1) Discovery filter: the copy must be un-suffixed <source>-copy, not
    // terminated, not the source, created at/after our POST (minus tolerance), and
    // NOT already claimed by another live Computer row.
    let rows_after = tensorlake_list_sandboxes(ctx, api_key)?;
    let min_created = post_ms - COPY_DISCOVERY_WINDOW_TOLERANCE_MS;
    let copy = rows_after.iter().find(|r| {
        r.name == want
            && r.id != source_sandbox_id
            && !is_terminated_status(&r.status)
            && !claimed_ids.iter().any(|c| c == &r.id)
            && (r.created_ms == 0 || r.created_ms >= min_created)
    });
    copy.map(|r| handle_for(r.id.clone())).ok_or_else(|| {
        format!("Tensorlake copy of {source_sandbox_id} was initiated but no fresh, unclaimed '{want}' was found")
    })
}

struct SandboxRow {
    id: String,
    name: String,
    status: String,
    created_ms: i64,
}

fn tensorlake_list_sandboxes(ctx: &Context, api_key: &str) -> Result<Vec<SandboxRow>, String> {
    let resp = ctx.http_call(
        "GET",
        "https://api.tensorlake.ai/sandboxes",
        &bearer_headers(api_key),
        "",
    )?;
    if resp.status < 200 || resp.status >= 300 {
        return Err(format!("Tensorlake list sandboxes failed (HTTP {})", resp.status));
    }
    let parsed: Value = serde_json::from_str(&resp.body)
        .map_err(|e| format!("failed to parse Tensorlake sandbox list: {e}"))?;
    let arr = parsed
        .as_array()
        .or_else(|| parsed.get("sandboxes").and_then(|v| v.as_array()));
    let mut out = Vec::new();
    if let Some(arr) = arr {
        for e in arr {
            let id = e
                .get("sandbox_id")
                .or_else(|| e.get("id"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if id.is_empty() {
                continue;
            }
            out.push(SandboxRow {
                id: id.to_string(),
                name: e.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                status: e.get("status").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                created_ms: parse_created_ms(e),
            });
        }
    }
    Ok(out)
}

/// Best-effort creation timestamp in ms. Tolerates epoch seconds, epoch ms, or an
/// RFC3339 string; returns 0 (unknown) if absent/unparseable, in which case the
/// window check is skipped for that row (the precondition + claimed-id guards still
/// apply).
fn parse_created_ms(row: &Value) -> i64 {
    for key in ["created_at", "createdAt", "created", "created_at_ms"] {
        match row.get(key) {
            Some(Value::Number(n)) => {
                if let Some(i) = n.as_i64() {
                    return if i < 100_000_000_000 { i * 1000 } else { i };
                }
            }
            Some(Value::String(sv)) => {
                if let Ok(i) = sv.parse::<i64>() {
                    return if i < 100_000_000_000 { i * 1000 } else { i };
                }
                // RFC3339 → epoch ms via a minimal parse is not available in this
                // no_std-ish wasm build; leave 0 (unknown) and rely on the other
                // guards rather than mis-parse a date.
                return 0;
            }
            _ => {}
        }
    }
    0
}

fn is_terminated_status(status: &str) -> bool {
    matches!(
        status.to_ascii_lowercase().as_str(),
        "terminated" | "destroyed" | "stopped" | "failed" | "deleted"
    )
}

fn handle_for(sandbox_id: String) -> SandboxHandle {
    SandboxHandle {
        sandbox_url: format!("https://{sandbox_id}.sandbox.tensorlake.ai"),
        sandbox_id,
        provider: "tensorlake".to_string(),
    }
}

fn tensorlake_terminate(ctx: &Context, api_key: &str, sandbox_id: &str) -> Result<(), String> {
    let url = format!("https://api.tensorlake.ai/sandboxes/{sandbox_id}");
    let resp = ctx.http_call("DELETE", &url, &bearer_headers(api_key), "")?;
    // 2xx = terminated; 404 = already gone (idempotent success for the reaper).
    if (resp.status >= 200 && resp.status < 300) || resp.status == 404 {
        Ok(())
    } else {
        Err(format!(
            "Tensorlake terminate of {sandbox_id} failed (HTTP {}): {}",
            resp.status,
            &resp.body[..resp.body.len().min(300)]
        ))
    }
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
    // Wall-clock captured at invocation ENTRY, so the poll budget below accounts
    // for the process-start latency, not just the time after it.
    let invocation_start = Context::get_time_millis();
    let run_id = unique_run_id(ctx)?;
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

    // Poll for the exit-code file (network latency provides natural backoff).
    // Bound by WALL TIME from invocation entry, not a fixed iteration count.
    // Budget against the 120s WASM invocation cap: a caller's command timeout must
    // be < 100s (computer_exec uses 90s), the poll runs to ~100s from entry so it
    // outlives that timeout and reaps the result, and the remaining ~20s covers
    // the stdout/stderr reads + the callback — all inside the 120s cap. Without
    // this the poll could give up early (fast GETs burning a fixed iteration
    // budget) and leave a live process behind a Failed row. The iteration cap is a
    // belt-and-suspenders guard against a stalled clock.
    let headers = bearer_headers(api_key);
    let rc_url = format!("{sandbox_url}/api/v1/files?path={}", url_encode(&rc_file));
    let poll_deadline = invocation_start + 100_000;
    let mut exit_code: i64 = -1;
    let mut found = false;
    for _ in 0..50_000 {
        if let Ok(r) = ctx.http_call("GET", &rc_url, &headers, "") {
            if r.status >= 200 && r.status < 300 {
                exit_code = r.body.trim().parse::<i64>().unwrap_or(-1);
                found = true;
                break;
            }
        }
        if Context::get_time_millis() >= poll_deadline {
            break;
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

/// Async exec START: POST the process, return its run_id. No poll.
fn tensorlake_exec_start(
    ctx: &Context,
    api_key: &str,
    sandbox_url: &str,
    command: &str,
    run_id: &str,
    tail_bytes: usize,
) -> Result<(), String> {
    let (out_file, err_file, rc_file) = paw_capture_files(run_id);
    let lock_file = format!("/tmp/.paw-lock-{run_id}");
    let out_tail = format!("{out_file}.tail");
    let err_tail = format!("{err_file}.tail");
    // Idempotent launch (rc-check + flock so a retried start never double-spawns),
    // then bound the output AT THE SOURCE: after the command, write `tail -c
    // {tail_bytes}` files and — on SUCCESS ONLY — delete the full (possibly huge)
    // out/err so a durable box does not accumulate them; on FAILURE the full files
    // are KEPT for debugging. The poll reads only the bounded `.tail` files, so a
    // gigabyte of output can never be pulled whole into WASM memory. rc is written
    // LAST, so when the poll sees it the tails are ready.
    let wrapped = format!(
        "if [ -f {rc_file} ]; then exit 0; fi; \
         exec 9>{lock_file} 2>/dev/null; \
         if ! flock -n 9; then exit 0; fi; \
         if [ -f {rc_file} ]; then exit 0; fi; \
         ({command}) > {out_file} 2> {err_file}; RC=$?; \
         tail -c {tail_bytes} {out_file} > {out_tail} 2>/dev/null; \
         tail -c {tail_bytes} {err_file} > {err_tail} 2>/dev/null; \
         if [ \"$RC\" = 0 ]; then rm -f {out_file} {err_file}; fi; \
         echo $RC > {rc_file}"
    );
    let body = json!({ "command": "/bin/bash", "args": ["-c", &wrapped] });
    let resp = ctx.http_call(
        "POST",
        &format!("{sandbox_url}/api/v1/processes"),
        &bearer_headers_json(api_key),
        &body.to_string(),
    )?;
    if resp.status >= 400 {
        return Err(format!("sandbox async start failed: {}", resp.body));
    }
    Ok(())
}

/// Async exec POLL: `Some(result)` once the rc file exists (finished), else
/// `None` (still running). On completion only the last `tail_bytes` of stdout and
/// stderr are pulled (HTTP Range) so a huge output cannot OOM the module; the
/// capture files are left in place (they die with the ephemeral sandbox, and not
/// deleting here removes the lose-the-result-on-crash window — see caller).
fn tensorlake_exec_poll(
    ctx: &Context,
    api_key: &str,
    sandbox_url: &str,
    run_id: &str,
    tail_bytes: usize,
) -> Result<Option<ExecResult>, String> {
    let (out_file, err_file, rc_file) = paw_capture_files(run_id);
    let rc = ctx.http_call(
        "GET",
        &format!("{sandbox_url}/api/v1/files?path={}", url_encode(&rc_file)),
        &bearer_headers(api_key),
        "",
    )?;
    if rc.status < 200 || rc.status >= 300 {
        return Ok(None); // rc file not written yet — still running
    }
    let exit_code = rc.body.trim().parse::<i64>().unwrap_or(-1);
    // Read only the bounded `.tail` files the wrapper produced (≤ tail_bytes each),
    // so output is capped at the SOURCE — no full-body read, no OOM.
    let stdout = read_capture_tail(ctx, api_key, sandbox_url, &format!("{out_file}.tail"), tail_bytes);
    let stderr = read_capture_tail(ctx, api_key, sandbox_url, &format!("{err_file}.tail"), tail_bytes);
    Ok(Some(ExecResult {
        stdout,
        stderr,
        exit_code,
    }))
}

/// Read a bounded `.tail` capture file (the wrapper already truncated it to
/// tail_bytes on the box). Only a 2xx body is returned — a 404 (no output) or a
/// 5xx error page becomes an EMPTY tail, never the error page itself. A defensive
/// re-truncate guards against a provider that returns more than asked. The exit
/// code is the source of truth; output is best-effort.
fn read_capture_tail(
    ctx: &Context,
    api_key: &str,
    sandbox_url: &str,
    path: &str,
    tail_bytes: usize,
) -> String {
    let resp = match ctx.http_call(
        "GET",
        &format!("{sandbox_url}/api/v1/files?path={}", url_encode(path)),
        &bearer_headers(api_key),
        "",
    ) {
        Ok(r) => r,
        Err(_) => return String::new(),
    };
    if resp.status < 200 || resp.status >= 300 {
        return String::new(); // 404 = no output; error page = not a tail
    }
    let body = resp.body;
    if body.len() <= tail_bytes {
        return body;
    }
    let mut start = body.len() - tail_bytes;
    while start < body.len() && !body.is_char_boundary(start) {
        start += 1;
    }
    body[start..].to_string()
}

// ===========================================================================
// Modal provider (calls REST bridge deployed on Modal)
// ===========================================================================

/// Resolve the Modal bridge base URL from config. This is the common prefix
/// of the per-endpoint URLs, e.g. `https://user--temperpaw-sandbox-bridge`.
/// Each endpoint appends its label suffix: `-create.modal.run`, `-exec.modal.run`, etc.
/// TemperPaw deploy should provision and persist `modal_bridge_url` automatically.
/// If it's missing at runtime, the deployment drifted or skipped the Modal bridge setup.
fn resolve_modal_base_url(config_value: Option<&String>) -> Result<String, String> {
    config_value
        .filter(|s| !s.is_empty() && !is_unresolved_secret(s))
        .cloned()
        .ok_or_else(|| {
            "Modal sandbox requires modal_bridge_url. TemperPaw deploy should provision MODAL_BRIDGE_URL automatically; redeploy or restore the platform secret if this deployment drifted.".to_string()
        })
}

fn modal_base_url(ctx: &Context) -> Result<String, String> {
    resolve_modal_base_url(ctx.config.get("modal_bridge_url"))
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
    let base = modal_base_url(ctx)?;
    let url = modal_url(&base, "create", api_key, "");
    let headers = vec![("content-type".to_string(), "application/json".to_string())];
    let body = modal_create_body(config);

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

fn modal_health_check(ctx: &Context, api_key: &str, sandbox_id: &str) -> Result<bool, String> {
    let base = modal_base_url(ctx)?;
    let params = format!("sandbox_id={}", url_encode(sandbox_id));
    let url = modal_url(&base, "health", api_key, &params);
    match ctx.http_call("GET", &url, &[], "") {
        Ok(r) if r.status >= 200 && r.status < 300 => {
            let parsed: Value = serde_json::from_str(&r.body).unwrap_or(json!({}));
            Ok(parsed
                .get("ready")
                .and_then(|v| v.as_bool())
                .unwrap_or(false))
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
    let base = modal_base_url(ctx)?;
    let params = format!(
        "sandbox_id={}&path={}",
        url_encode(sandbox_id),
        url_encode(path)
    );
    let url = modal_url(&base, "file-read", api_key, &params);
    let resp = ctx.http_call("GET", &url, &[], "")?;
    if resp.status >= 400 {
        return Err(format!("sandbox.read({path}): {}", resp.body));
    }
    let parsed: Value = serde_json::from_str(&resp.body).unwrap_or(json!({}));
    Ok(parsed
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string())
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

    let base = modal_base_url(ctx)?;
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
    let base = modal_base_url(ctx)?;
    let params = format!(
        "sandbox_id={}&path={}",
        url_encode(sandbox_id),
        url_encode(path)
    );
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
    let effective_workdir = if workdir.trim().is_empty() {
        "/"
    } else {
        workdir
    };
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
    let base = modal_base_url(ctx)?;
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
    fn capture_id_is_unique_per_dispatch_same_entity() {
        // THE ARN-401 CLASS: two execs for the SAME entity that used to collide
        // (same clock, both fresh instances so counter 0) must not share capture
        // files. Uniqueness now comes from the per-dispatch random u64.
        let a = capture_run_id("session-42", 0x1111_1111_1111_1111);
        let b = capture_run_id("session-42", 0x2222_2222_2222_2222);
        assert_ne!(a, b);
        assert_ne!(format!("/tmp/.paw-out-{a}"), format!("/tmp/.paw-out-{b}"));
    }

    #[test]
    fn capture_id_carries_a_readable_entity_label() {
        // Different entities are still distinguishable by the readable prefix
        // (the cross-entity ARN-401 case), even with the same random draw.
        let a = capture_run_id("rr-healthy-112", 7);
        let b = capture_run_id("rr-rollback-113", 7);
        assert_ne!(a, b);
        assert!(a.starts_with("err-healthy-112-"), "{a}");
        assert!(b.starts_with("err-rollback-113-"), "{b}");
    }

    #[test]
    fn capture_id_sanitizes_unsafe_entity_ids() {
        // Path separators / spaces / quotes cannot escape /tmp/.paw-* or break
        // the shell redirection.
        let id = capture_run_id("../../etc/passwd '; rm -rf ~", 0xdead_beef);
        assert!(!id.contains('/'), "{id}");
        assert!(!id.contains(' '), "{id}");
        assert!(!id.contains('\''), "{id}");
        assert!(!id.contains('.'), "{id}");
        assert!(id.ends_with("-00000000deadbeef"), "{id}");
    }

    #[test]
    fn capture_id_encoding_is_injective() {
        // Distinct entity ids that the old lossy sanitizer collapsed to the same
        // token (both `→ a_b`) get distinct readable segments (same random draw).
        let same = 5u64;
        assert_ne!(capture_run_id("a/b", same), capture_run_id("a?b", same));
        // `_` is escaped too, so it cannot alias an escaped byte.
        assert_ne!(capture_run_id("a_b", same), capture_run_id("a/b", same));
        assert_ne!(capture_run_id("a b", same), capture_run_id("a_b", same));
    }

    #[test]
    fn capture_id_is_length_bounded_and_empty_safe() {
        // A very long entity id is capped (32 encoded chars + 8-hex hash), so
        // the filename stays well under the filesystem per-component limit.
        let long = "x".repeat(500);
        let id = capture_run_id(&long, 1);
        // "e" + 32 + "-" + 8 + "-" + 16 == 59 chars; /tmp/.paw-out- prefix is 13.
        assert!(id.len() <= 60, "len {} id {}", id.len(), id);
        assert!(format!("/tmp/.paw-out-{id}").len() < 120);
        // The bound stays distinguishing via the full-id hash: two long ids that
        // share the first 32 encoded chars still differ.
        let a = "y".repeat(40) + "A";
        let b = "y".repeat(40) + "B";
        assert_ne!(capture_run_id(&a, 1), capture_run_id(&b, 1));
        // Empty id cannot alias a real id: it encodes to just the "e" prefix.
        assert!(capture_run_id("", 1).starts_with("e-"), "{}", capture_run_id("", 1));
        assert_ne!(capture_run_id("", 1), capture_run_id("e", 1));
    }

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
        assert_eq!(
            first_non_empty(&[Some("".into()), Some("def".into())]),
            "def"
        );
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

    #[test]
    fn test_resolve_modal_base_url_requires_explicit_value() {
        let configured = "https://user--temperpaw-sandbox-bridge".to_string();
        assert_eq!(
            resolve_modal_base_url(Some(&configured)).unwrap(),
            configured
        );

        let err = resolve_modal_base_url(None).unwrap_err();
        assert!(err.contains("modal_bridge_url"));
    }

    #[test]
    fn test_sandbox_config_from_fields_reads_managed_environment_settings() {
        let fields = json!({
            "SandboxNetworkingType": "Limited",
            "SandboxAllowedHostsJson": "[\"github.com\",\"api.anthropic.com\"]",
            "SandboxAllowMcpServers": true,
            "SandboxAllowPackageManagers": false,
            "SandboxPackagesJson": r#"[{"manager":"apt","name":"jq","version":"1.7"},{"manager":"pip","name":"rich","version":"13.9.4"}]"#,
        });

        let config = sandbox_config_from_fields(&fields);
        assert_eq!(config.networking_type, "Limited");
        assert_eq!(
            config.allowed_hosts,
            vec!["github.com".to_string(), "api.anthropic.com".to_string()]
        );
        assert!(config.allow_mcp_servers);
        assert!(!config.allow_package_managers);
        assert_eq!(
            config.packages,
            vec![
                SandboxPackage {
                    manager: "apt".to_string(),
                    name: "jq".to_string(),
                    version: "1.7".to_string(),
                },
                SandboxPackage {
                    manager: "pip".to_string(),
                    name: "rich".to_string(),
                    version: "13.9.4".to_string(),
                },
            ]
        );
    }

    #[test]
    fn test_tensorlake_create_body_includes_network_policy() {
        let config = SandboxConfig {
            cpus: 4,
            memory_mb: 8192,
            timeout_seconds: 7200,
            internet_access: true,
            networking_type: "Limited".to_string(),
            allowed_hosts: vec!["github.com".to_string(), "api.anthropic.com".to_string()],
            allow_mcp_servers: true,
            allow_package_managers: false,
            packages: vec![SandboxPackage {
                manager: "apt".to_string(),
                name: "jq".to_string(),
                version: "1.7".to_string(),
            }],
        };

        let body = tensorlake_create_body(&config);
        assert_eq!(body["resources"]["cpus"], json!(4));
        assert_eq!(body["resources"]["memory_mb"], json!(8192));
        assert_eq!(body["timeout_seconds"], json!(7200));
        assert_eq!(body["network"]["allow_internet_access"], json!(true));
        assert_eq!(
            body["network"]["allow_out"],
            json!(["github.com", "api.anthropic.com"])
        );
    }

    #[test]
    fn test_modal_create_body_includes_policy_fields() {
        let config = SandboxConfig {
            networking_type: "Limited".to_string(),
            allowed_hosts: vec!["github.com".to_string()],
            allow_mcp_servers: true,
            allow_package_managers: true,
            packages: vec![SandboxPackage {
                manager: "pip".to_string(),
                name: "rich".to_string(),
                version: "13.9.4".to_string(),
            }],
            ..SandboxConfig::default()
        };

        let body = modal_create_body(&config);
        assert_eq!(body["networking_type"], json!("Limited"));
        assert_eq!(body["allowed_hosts"], json!(["github.com"]));
        assert_eq!(body["allow_mcp_servers"], json!(true));
        assert_eq!(body["allow_package_managers"], json!(true));
        assert_eq!(
            body["packages"],
            json!([{
                "manager": "pip",
                "name": "rich",
                "version": "13.9.4",
            }])
        );
    }

    #[test]
    fn test_sandbox_package_install_script_groups_supported_package_managers() {
        let config = SandboxConfig {
            packages: vec![
                SandboxPackage {
                    manager: "apt".to_string(),
                    name: "jq".to_string(),
                    version: "1.7".to_string(),
                },
                SandboxPackage {
                    manager: "pip".to_string(),
                    name: "rich".to_string(),
                    version: "13.9.4".to_string(),
                },
                SandboxPackage {
                    manager: "npm".to_string(),
                    name: "typescript".to_string(),
                    version: "5.9.2".to_string(),
                },
            ],
            ..SandboxConfig::default()
        };

        let script =
            sandbox_package_install_script(&config).expect("expected package install script");
        assert!(script.contains("apt-get install -y --no-install-recommends jq=1.7"));
        assert!(script.contains(
            "python3 -m pip install --disable-pip-version-check --no-input rich==13.9.4"
        ));
        assert!(script.contains("npm install -g typescript@5.9.2"));
    }

    #[test]
    fn test_sandbox_observability_fields_expose_modal_bridge_context() {
        let fields = sandbox_observability_fields(
            "modal",
            "bash",
            "success",
            "sb-123",
            Some(0),
            Some(200),
            "/workspace/repo",
        );

        assert_eq!(fields["observability_event"], json!("temperpaw.sandbox"));
        assert_eq!(fields["sandbox_provider"], json!("modal"));
        assert_eq!(fields["sandbox_id"], json!("sb-123"));
        assert_eq!(fields["sandbox"]["operation"], json!("bash"));
        assert_eq!(fields["sandbox"]["outcome"], json!("success"));
        assert_eq!(fields["sandbox"]["backend"], json!("modal"));
        assert_eq!(fields["sandbox"]["exit_code"], json!(0));
        assert_eq!(fields["sandbox"]["status_code"], json!(200));
        assert_eq!(fields["sandbox"]["workdir"], json!("/workspace/repo"));
    }

    #[test]
    fn test_paw_agent_csdl_has_single_session_properties() {
        let csdl = include_str!("../../../specs/model.csdl.xml");
        for property in ["SessionMode", "PrePlanToolsEnabled", "ActivePlanId"] {
            let needle = format!("<Property Name=\"{property}\"");
            assert_eq!(
                csdl.matches(&needle).count(),
                1,
                "expected exactly one {property} property in paw-agent CSDL"
            );
        }
    }

    #[test]
    fn test_paw_agent_csdl_handle_tool_results_matches_runtime_params() {
        let csdl = include_str!("../../../specs/model.csdl.xml");
        let handle_tool_results = csdl
            .split("<Action Name=\"HandleToolResults\" IsBound=\"true\">")
            .nth(1)
            .and_then(|block| block.split("</Action>").next())
            .expect("HandleToolResults action block should exist");

        for parameter in [
            "sandbox_provider",
            "pending_tool_context",
            "pending_decision_id",
        ] {
            let needle = format!("<Parameter Name=\"{parameter}\"");
            assert!(
                handle_tool_results.contains(&needle),
                "expected HandleToolResults to expose {parameter}"
            );
        }
    }
    #[test]
    fn extract_copied_sandbox_id_tolerates_shapes() {
        use serde_json::json;
        assert_eq!(extract_copied_sandbox_id(&json!({"sandbox_id":"a1"})).as_deref(), Some("a1"));
        assert_eq!(extract_copied_sandbox_id(&json!({"id":"b2"})).as_deref(), Some("b2"));
        assert_eq!(extract_copied_sandbox_id(&json!({"sandboxes":[{"sandbox_id":"c3"}]})).as_deref(), Some("c3"));
        assert_eq!(extract_copied_sandbox_id(&json!({"sandboxes":["d4"]})).as_deref(), Some("d4"));
        assert_eq!(extract_copied_sandbox_id(&json!({"nope":1})), None);
    }

}
