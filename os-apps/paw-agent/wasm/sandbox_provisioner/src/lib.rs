//! Sandbox Provisioner — WASM module for provisioning Tensorlake sandboxes.
//!
//! Used by the Resume flow to provision a sandbox from a known URL.
//! For new sessions, sandbox provisioning is lazy (ADR-0022) — handled
//! by monty_repl/dispatch.rs on first sandbox tool call.
//!
//! Priority order:
//! 1. sandbox_url from entity state (set via Configure — testing override)
//! 2. sandbox_url from integration config (explicit override)
//! 3. Tensorlake REST API (production path)
//!
//! Build: `cargo build --target wasm32-unknown-unknown --release`

use temper_wasm_sdk::prelude::*;
use wasm_helpers::resolve_temper_api_url;

/// Entry point — used by the Resume flow to provision a sandbox for a restored session.
/// For new sessions, sandbox provisioning is lazy (ADR-0022).
#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        ctx.log("info", "sandbox_provisioner: starting (Resume flow)");

        let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));

        // Provision sandbox or schedule a later readiness check if the
        // Tensorlake microVM still needs time to boot.
        let sandbox_status = provision_sandbox(&ctx, &fields)?;
        let sandbox_result = match sandbox_status {
            SandboxStatus::Pending(sandbox_result) => {
                set_success_result(
                    "ProvisionPending",
                    &json!({
                        "sandbox_url": sandbox_result.sandbox_url,
                        "sandbox_id": sandbox_result.sandbox_id,
                    }),
                );
                return Ok(());
            }
            SandboxStatus::Ready(sandbox_result) => sandbox_result,
        };
        ctx.log(
            "info",
            &format!(
                "sandbox_provisioner: sandbox ready at {}",
                sandbox_result.sandbox_url
            ),
        );

        // Run post-provisioning setup: install tools declared in Computer spec
        // or project harness. For now, always install gh CLI if not present.
        run_sandbox_setup(&ctx, &sandbox_result.sandbox_url, &fields);

        // Return sandbox details to the state machine.
        // Workspace/conversation storage is handled by workspace_provisioner (ADR-0022).
        set_success_result(
            "SandboxReady",
            &json!({
                "sandbox_url": sandbox_result.sandbox_url,
                "sandbox_id": sandbox_result.sandbox_id,
            }),
        );

        Ok(())
    })();

    if let Err(e) = result {
        set_error_result(&e);
    }
    0
}

struct SandboxResult {
    sandbox_url: String,
    sandbox_id: String,
}

enum SandboxStatus {
    Pending(SandboxResult),
    Ready(SandboxResult),
}

/// Provision a sandbox. Priority order:
/// 1. sandbox_url from entity state (set via Configure action) or integration config
/// 2. Tensorlake REST API
/// 3. Fail with setup guidance
fn provision_sandbox(ctx: &Context, fields: &Value) -> Result<SandboxStatus, String> {
    // Retry path: Tensorlake sandbox was already created on a previous
    // invocation; only readiness checking remains.
    let existing_sandbox_url = fields
        .get("sandbox_url")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_owned);
    let existing_sandbox_id = fields
        .get("sandbox_id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_owned);
    if let (Some(sandbox_url), Some(sandbox_id)) = (existing_sandbox_url, existing_sandbox_id) {
        return check_sandbox_ready(ctx, fields, sandbox_url, sandbox_id);
    }

    // Priority 1: sandbox_url from Configure-time state or integration config.
    let static_url = fields
        .get("sandbox_url")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty() && !s.contains("{secret:"))
        .map(|s| s.to_string())
        .or_else(|| {
            ctx.config
                .get("sandbox_url")
                .filter(|s| !s.is_empty() && !s.contains("{secret:"))
                .cloned()
        })
        .or_else(|| {
            ctx.trigger_params
                .get("sandbox_url")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty() && !s.contains("{secret:"))
                .map(|s| s.to_string())
        });
    if let Some(url) = static_url {
        ctx.log(
            "info",
            &format!("sandbox_provisioner: using static sandbox_url: {url}"),
        );
        return Ok(SandboxStatus::Ready(SandboxResult {
            sandbox_url: url,
            sandbox_id: "static-sandbox".to_string(),
        }));
    }

    // Priority 2: Tensorlake REST API — create a Firecracker MicroVM sandbox.
    let api_key = ctx
        .config
        .get("tensorlake_api_key")
        .filter(|s| !s.is_empty() && !s.contains("{secret:"))
        .cloned();
    let api_key = match api_key {
        Some(key) => key,
        None => {
            return Err(
                "no sandbox_url configured and TL_API_KEY is not set — \
                 set TL_API_KEY in .env for Tensorlake sandbox provisioning"
                    .to_string(),
            );
        }
    };

    ctx.log("info", "sandbox_provisioner: provisioning via Tensorlake API");
    let create_url = "https://api.tensorlake.ai/sandboxes";
    let headers = vec![
        ("authorization".to_string(), format!("Bearer {api_key}")),
        ("content-type".to_string(), "application/json".to_string()),
    ];
    let body = json!({
        "resources": {
            "cpus": 2,
            "memory_mb": 4096
        },
        "timeout_seconds": 3600,
        "internet_access": true
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
    check_sandbox_ready(ctx, fields, sandbox_url, sandbox_id)
}

fn check_sandbox_ready(
    ctx: &Context,
    fields: &Value,
    sandbox_url: String,
    sandbox_id: String,
) -> Result<SandboxStatus, String> {
    let api_key = ctx
        .config
        .get("tensorlake_api_key")
        .filter(|s| !s.is_empty() && !s.contains("{secret:"))
        .cloned()
        .ok_or_else(|| {
            "TL_API_KEY is required to check Tensorlake sandbox readiness".to_string()
        })?;
    let health_headers = vec![("authorization".to_string(), format!("Bearer {api_key}"))];
    let health_url = format!("{sandbox_url}/api/v1/files/list?path=/");

    match ctx.http_call("GET", &health_url, &health_headers, "") {
        Ok(r) if r.status >= 200 && r.status < 300 => {
            ctx.log(
                "info",
                &format!(
                    "sandbox_provisioner: Tensorlake sandbox ready: id={sandbox_id}, url={sandbox_url}"
                ),
            );
            Ok(SandboxStatus::Ready(SandboxResult {
                sandbox_url,
                sandbox_id,
            }))
        }
        Ok(r) => {
            let check_count = fields
                .get("provision_check_count")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            let max_checks = fields
                .get("max_provision_checks")
                .and_then(|v| v.as_str())
                .and_then(|v| v.parse::<i64>().ok())
                .unwrap_or(24)
                .max(1);
            if check_count >= max_checks {
                Err(format!(
                    "Tensorlake sandbox {sandbox_id} did not become ready within {} retries (last HTTP {})",
                    max_checks, r.status
                ))
            } else {
                let temper_api_url = resolve_temper_api_url(ctx, fields);
                let retry_delay_seconds = 5;
                ctx.log(
                    "info",
                    &format!(
                        "sandbox_provisioner: sandbox {sandbox_id} not ready yet (HTTP {}), scheduling readiness check {}/{} in {}s via {}",
                        r.status,
                        check_count + 1,
                        max_checks,
                        retry_delay_seconds,
                        temper_api_url
                    ),
                );
                Ok(SandboxStatus::Pending(SandboxResult {
                    sandbox_url,
                    sandbox_id,
                }))
            }
        }
        Err(err) => {
            let check_count = fields
                .get("provision_check_count")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            let max_checks = fields
                .get("max_provision_checks")
                .and_then(|v| v.as_str())
                .and_then(|v| v.parse::<i64>().ok())
                .unwrap_or(24)
                .max(1);
            if check_count >= max_checks {
                Err(format!(
                    "Tensorlake sandbox {sandbox_id} did not become ready within {} retries: {}",
                    max_checks, err
                ))
            } else {
                ctx.log(
                    "info",
                    &format!(
                        "sandbox_provisioner: sandbox {sandbox_id} readiness check failed ({}), scheduling retry {}/{}",
                        err,
                        check_count + 1,
                        max_checks
                    ),
                );
                Ok(SandboxStatus::Pending(SandboxResult {
                    sandbox_url,
                    sandbox_id,
                }))
            }
        }
    }
}

/// Run post-provisioning setup on the sandbox.
///
/// Installs tools declared in the agent's project configuration.
/// Currently: always attempts to install `gh` CLI if available.
/// Non-fatal: logs warnings on failure but doesn't block provisioning.
fn run_sandbox_setup(ctx: &Context, sandbox_url: &str, fields: &Value) {
    if sandbox_url.is_empty() || sandbox_url == "static-sandbox" {
        return;
    }

    // Install gh CLI (GitHub CLI) — needed for governed PR operations
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

    let headers = vec![
        ("content-type".to_string(), "application/json".to_string()),
    ];
    let body = json!({
        "command": gh_setup,
        "timeout": 120
    });
    let url = format!("{sandbox_url}/commands");

    match ctx.http_call("POST", &url, &headers, &body.to_string()) {
        Ok(resp) if resp.status >= 200 && resp.status < 300 => {
            ctx.log("info", "sandbox_provisioner: gh CLI setup completed");
        }
        Ok(resp) => {
            ctx.log(
                "warn",
                &format!(
                    "sandbox_provisioner: gh CLI setup failed (HTTP {}): {}",
                    resp.status,
                    &resp.body[..resp.body.len().min(200)]
                ),
            );
        }
        Err(e) => {
            ctx.log(
                "warn",
                &format!("sandbox_provisioner: gh CLI setup request failed: {e}"),
            );
        }
    }

    // If tools_installed is set on the agent's fields, log what was requested
    let tools = fields
        .get("tools_installed")
        .or_else(|| fields.get("ToolsInstalled"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if !tools.is_empty() {
        ctx.log(
            "info",
            &format!("sandbox_provisioner: requested tools: {tools} (custom tool installation TBD)"),
        );
    }
}
