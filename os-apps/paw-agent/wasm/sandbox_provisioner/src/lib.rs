//! Sandbox Provisioner — WASM module for provisioning Tensorlake sandboxes.
//!
//! Provisions a Tensorlake sandbox via REST API and returns the sandbox
//! connection details. Also creates a TemperFS Workspace and File for
//! conversation storage (content-addressable, versioned, Cedar-governed).
//!
//! Priority order:
//! 1. sandbox_url from entity state (set via Configure — testing override)
//! 2. sandbox_url from integration config (explicit override)
//! 3. Tensorlake REST API (production path)
//!
//! Build: `cargo build --target wasm32-unknown-unknown --release`

use temper_wasm_sdk::prelude::*;
use wasm_helpers::{
    resolve_temper_api_url, runtime_headers, runtime_headers_for_workspace,
    write_temperfs_value_with_retry,
};

/// Entry point.
#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        ctx.log("info", "sandbox_provisioner: starting");

        let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));

        let user_message = fields
            .get("user_message")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if user_message.is_empty() {
            return Err("agent not configured — user_message is empty".to_string());
        }

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

        // Create TemperFS Workspace + File for conversation storage.
        // Prefer per-run override from Configure state, then integration config.
        let temper_api_url = resolve_temper_api_url(&ctx, &fields);

        let entity_id = ctx
            .entity_state
            .get("entity_id")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        let tenant = &ctx.tenant;

        let fs_result =
            create_conversation_storage(&ctx, &temper_api_url, tenant, entity_id, user_message);

        let (
            workspace_id,
            conversation_file_id,
            file_manifest_id,
            session_file_id,
            session_leaf_id,
        ) = match fs_result {
            Ok((ws, conv, manifest, session_file_id, session_leaf_id)) => {
                (ws, conv, manifest, session_file_id, session_leaf_id)
            }
            Err(e) => {
                ctx.log(
                    "warn",
                    &format!(
                        "sandbox_provisioner: TemperFS bootstrap failed at {temper_api_url}/tdata (tenant={tenant}, agent={entity_id}): {e}. Ensure os-app 'temper-fs' is installed for this tenant and temper_api_url is correct. Falling back to inline."
                    ),
                );
                (
                    String::new(),
                    String::new(),
                    String::new(),
                    String::new(),
                    String::new(),
                )
            }
        };

        // Return sandbox + TemperFS details to the state machine
        set_success_result(
            "SandboxReady",
            &json!({
                "sandbox_url": sandbox_result.sandbox_url,
                "sandbox_id": sandbox_result.sandbox_id,
                "workspace_id": workspace_id,
                "conversation_file_id": conversation_file_id,
                "file_manifest_id": file_manifest_id,
                "session_file_id": session_file_id,
                "session_leaf_id": session_leaf_id,
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

fn agent_headers(
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

fn workspace_headers(
    ctx: &Context,
    tenant: &str,
    workspace_id: &str,
    content_type: Option<&str>,
    accept: Option<&str>,
) -> Vec<(String, String)> {
    runtime_headers_for_workspace(ctx, tenant, &json!({}), workspace_id, content_type, accept)
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

/// Create a TemperFS Workspace, conversation File, manifest File, and session file.
/// Returns (workspace_entity_id, conversation_file_id, manifest_file_id, session_file_id, session_leaf_id).
fn create_conversation_storage(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    agent_id: &str,
    user_message: &str,
) -> Result<(String, String, String, String, String), String> {
    let headers = agent_headers(ctx, tenant, Some("application/json"), None);

    // 1. Create Workspace
    let ws_body = json!({
        "WorkspaceId": format!("agent-{agent_id}"),
        "name": format!("Agent {agent_id} Workspace"),
        "owner_id": agent_id,
        "quota_bytes": "104857600"
    });

    let ws_url = format!("{temper_api_url}/tdata/Workspaces");
    let ws_resp = ctx.http_call("POST", &ws_url, &headers, &ws_body.to_string())?;

    if ws_resp.status < 200 || ws_resp.status >= 300 {
        return Err(format!(
            "Workspace creation failed (HTTP {}): {}",
            ws_resp.status,
            &ws_resp.body[..ws_resp.body.len().min(300)]
        ));
    }

    let ws_parsed: Value = serde_json::from_str(&ws_resp.body)
        .map_err(|e| format!("parse workspace response: {e}"))?;
    let workspace_id = ws_parsed
        .get("entity_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    ctx.log(
        "info",
        &format!("sandbox_provisioner: created workspace {workspace_id}"),
    );

    // 2. Create File for conversation
    let file_body = json!({
        "FileId": format!("conv-{agent_id}"),
        "workspace_id": workspace_id,
        "name": "conversation.json",
        "mime_type": "application/json",
        "path": "/conversation.json"
    });

    let file_url = format!("{temper_api_url}/tdata/Files");
    let file_headers = workspace_headers(ctx, tenant, &workspace_id, Some("application/json"), None);
    let file_resp = ctx.http_call("POST", &file_url, &file_headers, &file_body.to_string())?;

    if file_resp.status < 200 || file_resp.status >= 300 {
        return Err(format!(
            "File creation failed (HTTP {}): {}",
            file_resp.status,
            &file_resp.body[..file_resp.body.len().min(300)]
        ));
    }

    let file_parsed: Value =
        serde_json::from_str(&file_resp.body).map_err(|e| format!("parse file response: {e}"))?;
    let file_id = file_parsed
        .get("entity_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    ctx.log(
        "info",
        &format!("sandbox_provisioner: created conversation file {file_id}"),
    );

    // 3. Write initial empty conversation
    let init_conv = json!({"messages": []}).to_string();
    let value_url = format!("{temper_api_url}/tdata/Files('{file_id}')/$value");
    let value_headers =
        workspace_headers(ctx, tenant, &workspace_id, Some("application/json"), None);
    if let Err(err) = write_temperfs_value_with_retry(
        ctx,
        &value_url,
        &value_headers,
        &init_conv,
        "sandbox_provisioner: initial $value write failed",
    ) {
        ctx.log(
            "warn",
            &err,
        );
    }

    // 4. Create manifest File for sandbox fsync
    let manifest_body = json!({
        "FileId": format!("manifest-{agent_id}"),
        "workspace_id": workspace_id,
        "name": "file_manifest.json",
        "mime_type": "application/json",
        "path": "/file_manifest.json"
    });

    let manifest_resp = ctx.http_call("POST", &file_url, &file_headers, &manifest_body.to_string())?;

    if manifest_resp.status < 200 || manifest_resp.status >= 300 {
        return Err(format!(
            "Manifest File creation failed (HTTP {}): {}",
            manifest_resp.status,
            &manifest_resp.body[..manifest_resp.body.len().min(300)]
        ));
    }

    let manifest_parsed: Value = serde_json::from_str(&manifest_resp.body)
        .map_err(|e| format!("parse manifest response: {e}"))?;
    let manifest_id = manifest_parsed
        .get("entity_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    ctx.log(
        "info",
        &format!("sandbox_provisioner: created manifest file {manifest_id}"),
    );

    // 5. Write initial empty manifest
    let init_manifest = json!({"files": {}, "synced_at_turn": 0}).to_string();
    let manifest_value_url = format!("{temper_api_url}/tdata/Files('{manifest_id}')/$value");
    if let Err(err) = write_temperfs_value_with_retry(
        ctx,
        &manifest_value_url,
        &value_headers,
        &init_manifest,
        "sandbox_provisioner: initial manifest $value write failed",
    ) {
        ctx.log(
            "warn",
            &err,
        );
    }

    let (session_file_id, session_leaf_id) = create_session_tree(
        ctx,
        temper_api_url,
        tenant,
        &workspace_id,
        agent_id,
        user_message,
    );

    Ok((
        workspace_id,
        file_id,
        manifest_id,
        session_file_id,
        session_leaf_id,
    ))
}

/// Create a session tree JSONL file in TemperFS.
/// Returns (session_file_id, session_leaf_id). Non-fatal on failure.
fn create_session_tree(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    workspace_id: &str,
    agent_id: &str,
    user_message: &str,
) -> (String, String) {
    let headers = workspace_headers(ctx, tenant, workspace_id, Some("application/json"), None);

    // Create session JSONL file in TemperFS
    let session_file_body = json!({
        "FileId": format!("session-{agent_id}"),
        "workspace_id": workspace_id,
        "name": "session.jsonl",
        "mime_type": "text/plain",
        "path": "/session.jsonl"
    });
    let session_file_resp = match ctx.http_call(
        "POST",
        &format!("{temper_api_url}/tdata/Files"),
        &headers,
        &serde_json::to_string(&session_file_body).unwrap_or_default(),
    ) {
        Ok(resp) => resp,
        Err(e) => {
            ctx.log("warn", &format!("Failed to create session file: {e}"));
            return (String::new(), String::new());
        }
    };

    let session_file_id = if session_file_resp.status >= 200 && session_file_resp.status < 300 {
        let parsed: Value = serde_json::from_str(&session_file_resp.body).unwrap_or(json!({}));
        parsed
            .get("entity_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    } else {
        ctx.log(
            "warn",
            &format!(
                "Failed to create session file (HTTP {})",
                session_file_resp.status
            ),
        );
        return (String::new(), String::new());
    };

    if session_file_id.is_empty() {
        return (String::new(), String::new());
    }

    // Create a TemperFS file for the first user message content.
    let content_file_headers =
        workspace_headers(ctx, tenant, workspace_id, Some("application/json"), None);
    let content_file_body = json!({
        "workspace_id": workspace_id,
        "name": format!("msg-u-{agent_id}-0.txt"),
        "mime_type": "text/plain",
        "path": format!("/msg-u-{agent_id}-0.txt")
    });
    let content_file_resp = ctx.http_call(
        "POST",
        &format!("{temper_api_url}/tdata/Files"),
        &content_file_headers,
        &serde_json::to_string(&content_file_body).unwrap_or_default(),
    );
    let content_file_id = match content_file_resp {
        Ok(resp) if resp.status >= 200 && resp.status < 300 => {
            let parsed: Value = serde_json::from_str(&resp.body).unwrap_or(json!({}));
            let file_id = parsed
                .get("entity_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if !file_id.is_empty() {
                let content_write_url =
                    format!("{temper_api_url}/tdata/Files('{file_id}')/$value");
                let content_write_headers =
                    workspace_headers(ctx, tenant, workspace_id, Some("text/plain"), None);
                match ctx.http_call("PUT", &content_write_url, &content_write_headers, user_message)
                {
                    Ok(write_resp) if write_resp.status >= 200 && write_resp.status < 300 => {
                        Some(file_id)
                    }
                    _ => None,
                }
            } else {
                None
            }
        }
        _ => None,
    };

    // Initialize session file with JSONL header + first user message.
    let header_id = format!("h-{agent_id}");
    let header_entry = json!({
        "id": header_id,
        "parentId": null,
        "type": "header",
        "version": 1,
        "tokens": 0
    });
    let header_line = serde_json::to_string(&header_entry).unwrap_or_default();

    let session_leaf_id = format!("u-{agent_id}-0");
    let user_entry = if let Some(content_file_id) = content_file_id {
        json!({
            "id": session_leaf_id,
            "parentId": header_id,
            "type": "message",
            "role": "user",
            "content_file_id": content_file_id,
            "tokens": user_message.len() / 4
        })
    } else {
        json!({
            "id": session_leaf_id,
            "parentId": header_id,
            "type": "message",
            "role": "user",
            "content": user_message,
            "tokens": user_message.len() / 4
        })
    };
    let user_line = serde_json::to_string(&user_entry).unwrap_or_default();
    let initial_jsonl = format!("{header_line}\n{user_line}");

    let write_url = format!("{temper_api_url}/tdata/Files('{session_file_id}')/$value");
    let write_headers = workspace_headers(ctx, tenant, workspace_id, Some("text/plain"), None);
    match write_temperfs_value_with_retry(
        ctx,
        &write_url,
        &write_headers,
        &initial_jsonl,
        "Failed to write session file",
    ) {
        Ok(()) => {
            ctx.log("info", "sandbox_provisioner: session tree initialized");
        }
        Err(e) => {
            ctx.log("warn", &e);
        }
    }

    (session_file_id, session_leaf_id)
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
