//! Workspace Restorer — WASM module for restoring sandbox files from TemperFS.
//!
//! Triggered by the `Resume` action's `restore_workspace` integration.
//! Reads the file manifest from TemperFS, downloads each file, and writes
//! it to the sandbox at the original path. Then returns `SandboxReady` so
//! the staged session turn flow can resume at context preparation.
//!
//! Build: `cargo build --target wasm32-unknown-unknown --release`

use temper_wasm_sdk::prelude::*;
use wasm_helpers::runtime_headers;

/// Entry point.
#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        ctx.log("info", "workspace_restorer: starting");

        let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));

        let sandbox_url = fields
            .get("sandbox_url")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if sandbox_url.is_empty() {
            return Err("sandbox_url is empty — cannot restore workspace".to_string());
        }

        let file_manifest_id = fields
            .get("file_manifest_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let sandbox_id = fields
            .get("sandbox_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let workspace_id = fields
            .get("workspace_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");
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

        // Build SandboxReady params to forward existing state
        let sandbox_ready_params = json!({
            "sandbox_url": sandbox_url,
            "sandbox_id": sandbox_id,
            "workspace_id": workspace_id,
            "conversation_file_id": conversation_file_id,
            "file_manifest_id": file_manifest_id,
            "session_file_id": session_file_id,
            "session_leaf_id": session_leaf_id,
        });

        if file_manifest_id.is_empty() {
            ctx.log(
                "warn",
                "workspace_restorer: no file_manifest_id, skipping restore",
            );
            set_success_result("SandboxReady", &sandbox_ready_params);
            return Ok(());
        }

        let temper_api_url = resolve_temper_api_url(&ctx, &fields);

        let tenant = &ctx.tenant;

        // Read manifest from TemperFS
        let manifest = read_manifest(&ctx, &temper_api_url, tenant, file_manifest_id)?;
        ctx.log(
            "info",
            &format!("workspace_restorer: manifest has {} files", manifest.len()),
        );

        let mut restored = 0usize;
        let mut failed = 0usize;

        for (path, file_id) in &manifest {
            // Read file content from TemperFS
            let content = match read_file_from_temperfs(&ctx, &temper_api_url, tenant, file_id) {
                Ok(c) => c,
                Err(e) => {
                    ctx.log("warn", &format!("workspace_restorer: skip {path}: {e}"));
                    failed += 1;
                    continue;
                }
            };

            // Write file to sandbox
            let write_result = write_file_local(&ctx, sandbox_url, path, &content);

            match write_result {
                Ok(_) => restored += 1,
                Err(e) => {
                    ctx.log(
                        "warn",
                        &format!("workspace_restorer: write failed for {path}: {e}"),
                    );
                    failed += 1;
                }
            }
        }

        ctx.log(
            "info",
            &format!("workspace_restorer: restored {restored} files, {failed} failed"),
        );

        // Dispatch SandboxReady so the normal staged Session turn loop resumes.
        set_success_result("SandboxReady", &sandbox_ready_params);

        Ok(())
    })();

    if let Err(e) = result {
        set_error_result(&e);
    }
    0
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

/// Read the file manifest from TemperFS.
/// Returns a map of sandbox path → TemperFS file entity ID.
fn read_manifest(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    manifest_file_id: &str,
) -> Result<Vec<(String, String)>, String> {
    let url = format!("{temper_api_url}/tdata/Files('{manifest_file_id}')/$value");
    let headers = agent_headers(ctx, tenant, None, Some("application/json"));

    let resp = ctx.http_call("GET", &url, &headers, "")?;
    if resp.status != 200 {
        return Err(format!("manifest read failed (HTTP {})", resp.status));
    }

    let parsed: Value =
        serde_json::from_str(&resp.body).map_err(|e| format!("manifest parse failed: {e}"))?;

    let files_obj = parsed
        .get("files")
        .and_then(|v| v.as_object())
        .ok_or("manifest missing 'files' object")?;

    let mut result = Vec::new();
    for (path, entry) in files_obj {
        if let Some(file_id) = entry.get("file_id").and_then(|v| v.as_str()) {
            result.push((path.clone(), file_id.to_string()));
        }
    }

    Ok(result)
}

/// Read file content from TemperFS File entity $value.
fn read_file_from_temperfs(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    file_id: &str,
) -> Result<String, String> {
    let url = format!("{temper_api_url}/tdata/Files('{file_id}')/$value");
    let headers = agent_headers(ctx, tenant, None, None);

    let resp = ctx.http_call("GET", &url, &headers, "")?;
    if resp.status != 200 {
        return Err(format!("file read failed (HTTP {})", resp.status));
    }

    Ok(resp.body)
}

/// Write file to sandbox via Tensorlake data plane API.
fn write_file_local(
    ctx: &Context,
    sandbox_url: &str,
    full_path: &str,
    content: &str,
) -> Result<(), String> {
    let api_key = ctx
        .config
        .get("tensorlake_api_key")
        .filter(|s| !s.is_empty() && !s.contains("{secret:"))
        .cloned()
        .unwrap_or_default();
    let url = format!("{sandbox_url}/api/v1/files?path={}", url_encode(full_path));
    let mut headers = vec![("content-type".to_string(), "application/octet-stream".to_string())];
    if !api_key.is_empty() {
        headers.push(("authorization".to_string(), format!("Bearer {api_key}")));
    }
    let resp = ctx.http_call("PUT", &url, &headers, content)?;
    if resp.status >= 200 && resp.status < 300 {
        Ok(())
    } else {
        Err(format!("write failed (HTTP {})", resp.status))
    }
}

/// Minimal URL encoding for path parameters.
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

fn resolve_temper_api_url(ctx: &Context, fields: &Value) -> String {
    fields
        .get("temper_api_url")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .or_else(|| {
            ctx.config
                .get("temper_api_url")
                .filter(|s| !s.is_empty())
                .cloned()
        })
        .unwrap_or_else(|| "http://127.0.0.1:3000".to_string())
}
