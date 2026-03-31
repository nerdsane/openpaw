//! Capability Installer — WASM integration for CapabilityRequest entities.
//!
//! Triggered when a CapabilityRequest transitions to Installing (after approval).
//! Dispatches the actual installation based on capability_type:
//!   - os_app    → POST /api/apps/install
//!   - specs     → POST /api/specs/load-inline
//!   - wasm      → POST /api/wasm/modules/<name>
//!   - secret    → logs instruction for human provisioning
//!
//! Build: `cargo build --target wasm32-unknown-unknown --release`

use temper_wasm_sdk::prelude::*;

#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        ctx.log("info", "capability_installer: starting");

        let temper_api_url = ctx
            .config
            .get("temper_api_url")
            .cloned()
            .unwrap_or_else(|| "http://127.0.0.1:3000".to_string());
        let tenant = &ctx.tenant;

        let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));
        let capability_type = fields
            .get("capability_type")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let capability_name = fields
            .get("capability_name")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let payload = fields
            .get("payload")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        ctx.log(
            "info",
            &format!(
                "capability_installer: installing {capability_type} '{capability_name}'"
            ),
        );

        let headers = vec![
            ("Content-Type".to_string(), "application/json".to_string()),
            ("X-Tenant-Id".to_string(), tenant.to_string()),
        ];

        match capability_type {
            "os_app" => {
                // Install an OS app bundle
                let body = json!({
                    "tenant": tenant,
                    "app_name": capability_name,
                });
                let resp = ctx.http_call(
                    "POST",
                    &format!("{temper_api_url}/api/apps/install"),
                    &headers,
                    &body.to_string(),
                )?;
                if resp.status >= 400 {
                    return Err(format!(
                        "failed to install app '{}': {} {}",
                        capability_name, resp.status, resp.body
                    ));
                }
                ctx.log(
                    "info",
                    &format!("capability_installer: app '{capability_name}' installed"),
                );
            }

            "specs" => {
                // Load specs inline
                let specs: Value = if payload.is_empty() {
                    return Err("specs capability requires payload with spec content".into());
                } else {
                    serde_json::from_str(payload)
                        .map_err(|e| format!("invalid specs payload JSON: {e}"))?
                };
                let body = json!({
                    "tenant": tenant,
                    "specs": specs,
                });
                let resp = ctx.http_call(
                    "POST",
                    &format!("{temper_api_url}/api/specs/load-inline"),
                    &headers,
                    &body.to_string(),
                )?;
                if resp.status >= 400 {
                    return Err(format!(
                        "failed to load specs '{}': {} {}",
                        capability_name, resp.status, resp.body
                    ));
                }
                ctx.log(
                    "info",
                    &format!("capability_installer: specs '{capability_name}' loaded"),
                );
            }

            "wasm" => {
                // Upload a WASM module
                if payload.is_empty() {
                    return Err("wasm capability requires payload with base64 WASM bytes".into());
                }
                let body = json!({ "wasm_base64": payload });
                let resp = ctx.http_call(
                    "POST",
                    &format!("{temper_api_url}/api/wasm/modules/{capability_name}"),
                    &headers,
                    &body.to_string(),
                )?;
                if resp.status >= 400 {
                    return Err(format!(
                        "failed to upload WASM '{}': {} {}",
                        capability_name, resp.status, resp.body
                    ));
                }
                ctx.log(
                    "info",
                    &format!("capability_installer: WASM '{capability_name}' uploaded"),
                );
            }

            "secret" => {
                // Secrets require human provisioning — log the request
                ctx.log(
                    "info",
                    &format!(
                        "capability_installer: secret '{}' requested — requires human provisioning via Observe UI",
                        capability_name
                    ),
                );
            }

            _ => {
                return Err(format!(
                    "unknown capability_type: '{}'. Expected: os_app, specs, wasm, secret",
                    capability_type
                ));
            }
        }

        set_success_result("InstallComplete", &json!({}));
        Ok(())
    })();

    match result {
        Ok(()) => 0,
        Err(e) => {
            set_error_result(&e);
            1
        }
    }
}
