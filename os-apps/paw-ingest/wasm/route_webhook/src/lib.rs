//! Route Webhook — WASM module for routing validated webhooks to target entities.
//!
//! Triggered by WebhookEvent.Validated action. Looks up the WebhookRoute to
//! determine the target entity type and action, optionally resolves monitors
//! and checks for duplicates, then creates the target entity.
//!
//! Build: `cargo build --target wasm32-unknown-unknown --release`

use temper_wasm_sdk::prelude::*;

/// Entry point.
#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        ctx.log("info", "route_webhook: starting");

        let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));

        // Read state set by prior actions
        let source_type = fields
            .get("source_type")
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

        if route_key.is_empty() {
            set_success_result("RouteFailed", &json!({
                "validation_error": "route_key missing from entity state"
            }));
            return Ok(());
        }

        let temper_api_url = resolve_api_url(&ctx);
        let tenant = &ctx.tenant;
        let headers = odata_headers(tenant);

        // Query WebhookRoute by route_key to get routing config
        let filter = format!(
            "route_key eq '{}' and Status eq 'Active'",
            route_key.replace('\'', "''")
        );
        let query_url = format!(
            "{}/tdata/WebhookRoutes?$filter={}",
            temper_api_url,
            urlencoded(&filter)
        );

        let resp = ctx.http_call("GET", &query_url, &headers, "")?;
        if resp.status < 200 || resp.status >= 300 {
            set_success_result("RouteFailed", &json!({
                "validation_error": format!("route lookup failed (HTTP {})", resp.status)
            }));
            return Ok(());
        }

        let body: Value = serde_json::from_str(&resp.body)
            .map_err(|e| format!("parse route response: {e}"))?;

        let routes = body
            .get("value")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        if routes.is_empty() {
            set_success_result("RouteFailed", &json!({
                "validation_error": "no matching route for routing"
            }));
            return Ok(());
        }

        let route = &routes[0];
        let route_fields = route.get("fields").cloned().unwrap_or(json!({}));

        let route_id = route
            .get("entity_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let target_entity_type = route_fields
            .get("target_entity_type")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let target_action = route_fields
            .get("target_action")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let monitor_resolution_enabled = route_fields
            .get("monitor_resolution_enabled")
            .and_then(|v| v.as_str())
            .unwrap_or("false");

        let dedup_enabled = route_fields
            .get("dedup_enabled")
            .and_then(|v| v.as_str())
            .unwrap_or("false");

        if target_entity_type.is_empty() || target_action.is_empty() {
            set_success_result("RouteFailed", &json!({
                "validation_error": "route missing target_entity_type or target_action"
            }));
            return Ok(());
        }

        ctx.log(
            "info",
            &format!(
                "route_webhook: route_id={}, target={}::{}, monitor_res={}, dedup={}",
                route_id, target_entity_type, target_action,
                monitor_resolution_enabled, dedup_enabled
            ),
        );

        // Parse the normalized payload for monitor/dedup logic
        let payload: Value = serde_json::from_str(normalized_payload).unwrap_or(json!({}));

        // Monitor resolution: if enabled and source is datadog, ensure Monitor entity exists
        if monitor_resolution_enabled == "true" && source_type == "datadog" {
            if let Err(e) = resolve_monitor(&ctx, &temper_api_url, &headers, &payload) {
                ctx.log(
                    "warn",
                    &format!("route_webhook: monitor resolution failed (non-fatal): {e}"),
                );
            }
        }

        // Dedup check: query recent target entities for duplicates
        if dedup_enabled == "true" {
            if let Some(err) = check_dedup(&ctx, &temper_api_url, &headers, target_entity_type, &payload) {
                set_success_result("RouteFailed", &json!({
                    "validation_error": err
                }));
                return Ok(());
            }
        }

        // Create the target entity (e.g., AlertCycle)
        let create_url = format!("{}/tdata/{}s", temper_api_url, target_entity_type);
        let create_body = json!({});
        let create_resp = ctx.http_call("POST", &create_url, &headers, &create_body.to_string())?;

        if create_resp.status < 200 || create_resp.status >= 300 {
            set_success_result("RouteFailed", &json!({
                "validation_error": format!(
                    "failed to create {} (HTTP {}): {}",
                    target_entity_type,
                    create_resp.status,
                    &create_resp.body[..create_resp.body.len().min(300)]
                )
            }));
            return Ok(());
        }

        let created: Value = serde_json::from_str(&create_resp.body)
            .map_err(|e| format!("parse created entity response: {e}"))?;

        let target_entity_id = created
            .get("entity_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        ctx.log(
            "info",
            &format!(
                "route_webhook: created {}({}) — will dispatch {}",
                target_entity_type, target_entity_id, target_action
            ),
        );

        set_success_result("Routed", &json!({
            "target_entity_type": target_entity_type,
            "target_entity_id": target_entity_id,
            "target_action": target_action,
            "webhook_route_id": route_id,
        }));

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

/// Minimal URL-encoding for OData filter values.
fn urlencoded(s: &str) -> String {
    s.replace(' ', "%20")
        .replace('\'', "%27")
        .replace('&', "%26")
        .replace('=', "%3D")
}

/// Resolve or create a Monitor entity for a Datadog alert.
/// Extracts dd_monitor_id from the payload, queries for an existing Monitor,
/// and creates + configures one if not found.
fn resolve_monitor(
    ctx: &Context,
    temper_api_url: &str,
    headers: &[(String, String)],
    payload: &Value,
) -> Result<(), String> {
    let monitor_id_raw = payload
        .get("monitor_id")
        .or_else(|| payload.get("dd_monitor_id"))
        .and_then(|v| v.as_str().map(|s| s.to_string()).or_else(|| Some(v.to_string())))
        .unwrap_or_default();

    if monitor_id_raw.is_empty() {
        return Ok(()); // No monitor ID in payload, skip
    }

    let filter = format!("dd_monitor_id eq '{}'", monitor_id_raw.replace('\'', "''"));
    let query_url = format!(
        "{}/tdata/Monitors?$filter={}",
        temper_api_url,
        urlencoded(&filter)
    );

    let resp = ctx.http_call("GET", &query_url, headers, "")?;

    if resp.status >= 200 && resp.status < 300 {
        let body: Value = serde_json::from_str(&resp.body).unwrap_or(json!({}));
        let monitors = body
            .get("value")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        if monitors.is_empty() {
            // Create a new Monitor entity
            ctx.log(
                "info",
                &format!("route_webhook: creating Monitor for dd_monitor_id={}", monitor_id_raw),
            );

            let create_body = json!({});
            let create_resp = ctx.http_call(
                "POST",
                &format!("{}/tdata/Monitors", temper_api_url),
                headers,
                &create_body.to_string(),
            )?;

            if create_resp.status >= 200 && create_resp.status < 300 {
                let created: Value =
                    serde_json::from_str(&create_resp.body).unwrap_or(json!({}));
                let monitor_entity_id = created
                    .get("entity_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                if !monitor_entity_id.is_empty() {
                    // Dispatch Configure action on the new Monitor
                    let action_url = format!(
                        "{}/tdata/Monitors('{}')/Configure",
                        temper_api_url, monitor_entity_id
                    );
                    let action_body = json!({
                        "dd_monitor_id": monitor_id_raw,
                    });
                    let _ = ctx.http_call("POST", &action_url, headers, &action_body.to_string());

                    // Dispatch Activate action
                    let activate_url = format!(
                        "{}/tdata/Monitors('{}')/Activate",
                        temper_api_url, monitor_entity_id
                    );
                    let _ = ctx.http_call("POST", &activate_url, headers, "{}");
                }
            }
        } else {
            ctx.log(
                "info",
                &format!("route_webhook: Monitor already exists for dd_monitor_id={}", monitor_id_raw),
            );
        }
    }

    Ok(())
}

/// Check for duplicate target entities. Returns Some(error_message) if duplicate found.
fn check_dedup(
    ctx: &Context,
    temper_api_url: &str,
    headers: &[(String, String)],
    target_entity_type: &str,
    payload: &Value,
) -> Option<String> {
    // Extract a dedup key from the payload (use alert_id or a hash of the payload)
    let dedup_key = payload
        .get("alert_id")
        .or_else(|| payload.get("id"))
        .and_then(|v| v.as_str().map(|s| s.to_string()).or_else(|| Some(v.to_string())))
        .unwrap_or_default();

    if dedup_key.is_empty() {
        return None; // No dedup key available, skip check
    }

    // Query recent entities of the target type with the same dedup key
    let filter = format!(
        "alert_id eq '{}'",
        dedup_key.replace('\'', "''")
    );
    let query_url = format!(
        "{}/tdata/{}s?$filter={}&$top=1",
        temper_api_url,
        target_entity_type,
        urlencoded(&filter)
    );

    ctx.log(
        "info",
        &format!("route_webhook: dedup check for {}={}", target_entity_type, dedup_key),
    );

    match ctx.http_call("GET", &query_url, headers, "") {
        Ok(resp) if resp.status >= 200 && resp.status < 300 => {
            let body: Value = serde_json::from_str(&resp.body).unwrap_or(json!({}));
            let existing = body
                .get("value")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();

            if !existing.is_empty() {
                Some("duplicate alert".to_string())
            } else {
                None
            }
        }
        _ => None, // On error, allow through rather than blocking
    }
}
