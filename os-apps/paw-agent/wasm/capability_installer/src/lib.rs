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
        // Try both snake_case (IOA field names) and PascalCase (OData convention)
        let capability_type = fields.get("capability_type")
            .or_else(|| fields.get("CapabilityType"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let capability_name = fields.get("capability_name")
            .or_else(|| fields.get("CapabilityName"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let payload = fields.get("payload")
            .or_else(|| fields.get("Payload"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        ctx.log(
            "info",
            &format!(
                "capability_installer: installing {capability_type} '{capability_name}'"
            ),
        );

        // Use admin principal for installation — the human already approved via Approve action.
        let headers = vec![
            ("Content-Type".to_string(), "application/json".to_string()),
            ("X-Tenant-Id".to_string(), tenant.to_string()),
            ("X-Temper-Principal-Kind".to_string(), "admin".to_string()),
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

            "cedar_policy" => {
                // Create a Cedar policy entry
                if payload.is_empty() {
                    return Err("cedar_policy capability requires payload with Cedar policy text".into());
                }
                let body = json!({
                    "policy_id": capability_name,
                    "cedar_text": payload,
                });
                let resp = ctx.http_call(
                    "POST",
                    &format!("{temper_api_url}/api/tenants/{tenant}/policies/create"),
                    &headers,
                    &body.to_string(),
                )?;
                if resp.status >= 400 {
                    return Err(format!(
                        "failed to create Cedar policy '{}': {} {}",
                        capability_name, resp.status, resp.body
                    ));
                }
                ctx.log(
                    "info",
                    &format!("capability_installer: Cedar policy '{capability_name}' created"),
                );
            }

            "app" => {
                // Bundled app: specs + WASM modules + Cedar policies in one payload.
                // Payload is JSON: { "specs": {...}, "wasm_modules": {...}, "policies": {...} }
                if payload.is_empty() {
                    return Err("app capability requires payload with specs/wasm_modules/policies".into());
                }
                let bundle: Value = serde_json::from_str(payload)
                    .map_err(|e| format!("invalid app bundle JSON: {e}"))?;

                // 1. Specs
                if let Some(specs) = bundle.get("specs").filter(|v| v.is_object()) {
                    let body = json!({ "tenant": tenant, "specs": specs });
                    let resp = ctx.http_call(
                        "POST",
                        &format!("{temper_api_url}/api/specs/load-inline"),
                        &headers,
                        &body.to_string(),
                    )?;
                    if resp.status >= 400 {
                        return Err(format!(
                            "app bundle: failed to load specs: {} {}",
                            resp.status, resp.body
                        ));
                    }
                    ctx.log("info", "capability_installer: app bundle specs loaded");
                }

                // 2. WASM modules
                if let Some(modules) = bundle.get("wasm_modules").and_then(|v| v.as_object()) {
                    for (name, b64) in modules {
                        let wasm_b64 = b64.as_str().unwrap_or("");
                        let body = json!({ "wasm_base64": wasm_b64 });
                        let resp = ctx.http_call(
                            "POST",
                            &format!("{temper_api_url}/api/wasm/modules/{name}"),
                            &headers,
                            &body.to_string(),
                        )?;
                        if resp.status >= 400 {
                            return Err(format!(
                                "app bundle: failed to upload WASM '{}': {} {}",
                                name, resp.status, resp.body
                            ));
                        }
                        ctx.log("info", &format!("capability_installer: app bundle WASM '{name}' uploaded"));
                    }
                }

                // 3. Cedar policies
                if let Some(policies) = bundle.get("policies").and_then(|v| v.as_object()) {
                    for (pid, text) in policies {
                        let cedar_text = text.as_str().unwrap_or("");
                        let body = json!({ "policy_id": pid, "cedar_text": cedar_text });
                        let resp = ctx.http_call(
                            "POST",
                            &format!("{temper_api_url}/api/tenants/{tenant}/policies/create"),
                            &headers,
                            &body.to_string(),
                        )?;
                        if resp.status >= 400 {
                            return Err(format!(
                                "app bundle: failed to create policy '{}': {} {}",
                                pid, resp.status, resp.body
                            ));
                        }
                        ctx.log("info", &format!("capability_installer: app bundle policy '{pid}' created"));
                    }
                }

                ctx.log("info", &format!("capability_installer: app bundle '{capability_name}' installed"));
            }

            _ => {
                return Err(format!(
                    "unknown capability_type: '{}'. Expected: os_app, specs, wasm, secret, cedar_policy, app",
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
