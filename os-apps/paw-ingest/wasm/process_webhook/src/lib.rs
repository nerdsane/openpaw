//! Process Webhook — WASM module for dispatching the routed action on the target entity.
//!
//! Triggered by WebhookEvent.Routed action. Reads the target entity details
//! from entity state and dispatches the configured action (e.g., AlertCycle.Open)
//! via the Temper OData API.
//!
//! Build: `cargo build --target wasm32-unknown-unknown --release`

use temper_wasm_sdk::prelude::*;

/// Entry point.
#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        ctx.log("info", "process_webhook: starting");

        let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));

        // Read routing state set by the Routed action
        let target_entity_type = fields
            .get("target_entity_type")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let target_entity_id = fields
            .get("target_entity_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let target_action = fields
            .get("target_action")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let normalized_payload = fields
            .get("normalized_payload")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let route_key = fields
            .get("route_key")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if target_entity_type.is_empty() || target_entity_id.is_empty() || target_action.is_empty()
        {
            return Err(
                "missing target_entity_type, target_entity_id, or target_action".to_string(),
            );
        }

        let temper_api_url = resolve_api_url(&ctx);
        let tenant = &ctx.tenant;
        let headers = odata_headers(tenant);

        // Build action params from the normalized payload
        let action_params = build_action_params(target_action, normalized_payload, route_key);

        // Dispatch the target action via OData POST
        let action_url = format!(
            "{}/tdata/{}s('{}')/{}",
            temper_api_url, target_entity_type, target_entity_id, target_action
        );

        ctx.log(
            "info",
            &format!("process_webhook: dispatching {} on {}({})", target_action, target_entity_type, target_entity_id),
        );

        let resp = ctx.http_call("POST", &action_url, &headers, &action_params.to_string())?;

        if resp.status < 200 || resp.status >= 300 {
            return Err(format!(
                "action dispatch failed (HTTP {}): {}",
                resp.status,
                &resp.body[..resp.body.len().min(500)]
            ));
        }

        ctx.log(
            "info",
            &format!(
                "process_webhook: successfully dispatched {}.{} on entity {}",
                target_entity_type, target_action, target_entity_id
            ),
        );

        set_success_result("Processed", &json!({}));

        Ok(())
    })();

    if let Err(e) = result {
        set_error_result(&e);
    }
    0
}

/// Resolve the Temper API URL from integration config or fall back to localhost.
fn resolve_api_url(ctx: &Context) -> String {
    ctx.config
        .get("temper_api_url")
        .filter(|s| !s.is_empty() && !s.contains("{secret:"))
        .cloned()
        .unwrap_or_else(|| "http://127.0.0.1:3000".to_string())
}

/// Build standard OData request headers.
fn odata_headers(tenant: &str) -> Vec<(String, String)> {
    vec![
        ("content-type".to_string(), "application/json".to_string()),
        ("x-tenant-id".to_string(), tenant.to_string()),
        ("x-temper-principal-kind".to_string(), "admin".to_string()),
    ]
}

/// Build action parameters for the target entity action based on the normalized payload.
/// For AlertCycle.Open: extract monitor_id and pass the alert payload.
/// For other actions: pass the full normalized payload as alert_payload.
fn build_action_params(target_action: &str, normalized_payload: &str, _route_key: &str) -> Value {
    let payload: Value = serde_json::from_str(normalized_payload).unwrap_or(json!({}));

    match target_action {
        "Open" => {
            // AlertCycle.Open expects monitor_id and alert_payload
            let monitor_id = payload
                .get("monitor_id")
                .or_else(|| payload.get("dd_monitor_id"))
                .and_then(|v| v.as_str().map(|s| s.to_string()).or_else(|| Some(v.to_string())))
                .unwrap_or_default();

            json!({
                "monitor_id": monitor_id,
                "alert_payload": normalized_payload,
            })
        }
        _ => {
            // Generic fallback: pass the full payload
            json!({
                "alert_payload": normalized_payload,
            })
        }
    }
}
