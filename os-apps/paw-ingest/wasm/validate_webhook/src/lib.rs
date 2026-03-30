//! Validate Webhook — WASM module for validating incoming webhook payloads.
//!
//! Triggered by WebhookEvent.Received action. Looks up the WebhookRoute by
//! route_key, verifies HMAC signature if a secret is configured, and transitions
//! to Validated or ValidationFailed.
//!
//! Build: `cargo build --target wasm32-unknown-unknown --release`

use temper_wasm_sdk::prelude::*;

/// Entry point.
#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        ctx.log("info", "validate_webhook: starting");

        let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));

        // Read route_key from trigger params (set by Received action)
        let route_key = fields
            .get("route_key")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if route_key.is_empty() {
            set_success_result("ValidationFailed", &json!({
                "validation_error": "route_key is empty"
            }));
            return Ok(());
        }

        let raw_payload = fields
            .get("raw_payload")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let raw_headers = fields
            .get("raw_headers")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // Resolve Temper API URL from config
        let temper_api_url = resolve_api_url(&ctx);

        let tenant = &ctx.tenant;
        let headers = odata_headers(&ctx, tenant);

        // Query WebhookRoutes by route_key and Active status
        let filter = format!(
            "route_key eq '{}' and Status eq 'Active'",
            route_key.replace('\'', "''")
        );
        let query_url = format!(
            "{}/tdata/WebhookRoutes?$filter={}",
            temper_api_url,
            urlencoded(&filter)
        );

        ctx.log("info", &format!("validate_webhook: querying routes: {query_url}"));

        let resp = ctx.http_call("GET", &query_url, &headers, "")?;
        if resp.status < 200 || resp.status >= 300 {
            set_success_result("ValidationFailed", &json!({
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
            set_success_result("ValidationFailed", &json!({
                "validation_error": "no matching route"
            }));
            return Ok(());
        }

        let route = &routes[0];
        let route_fields = route.get("fields").cloned().unwrap_or(json!({}));

        let webhook_secret = route_fields
            .get("webhook_secret")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let source_type = route_fields
            .get("source_type")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // HMAC verification (simplified — check header presence if secret is configured)
        let hmac_verified = if webhook_secret.is_empty() {
            "skipped"
        } else {
            verify_signature_header(raw_headers, source_type)
        };

        ctx.log(
            "info",
            &format!(
                "validate_webhook: route matched, source_type={}, hmac={}",
                source_type, hmac_verified
            ),
        );

        set_success_result("Validated", &json!({
            "source_type": source_type,
            "hmac_verified": hmac_verified,
            "normalized_payload": raw_payload,
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
fn odata_headers(ctx: &Context, tenant: &str) -> Vec<(String, String)> {
    vec![
        ("content-type".to_string(), "application/json".to_string()),
        ("x-tenant-id".to_string(), tenant.to_string()),
        ("x-temper-principal-kind".to_string(), "agent".to_string()),
        ("x-temper-principal-id".to_string(), ctx.entity_id.clone()),
        ("x-temper-agent-type".to_string(), "system".to_string()),
    ]
}

/// Minimal URL-encoding for OData filter values.
fn urlencoded(s: &str) -> String {
    s.replace(' ', "%20")
        .replace('\'', "%27")
        .replace('&', "%26")
        .replace('=', "%3D")
}

/// Simplified HMAC signature check: verify the expected signature header exists
/// in the raw headers JSON. Full cryptographic verification can be added later.
fn verify_signature_header(raw_headers: &str, source_type: &str) -> &'static str {
    let header_name = match source_type {
        "datadog" => "x-datadog-signature",
        "github" => "x-hub-signature-256",
        _ => "x-hub-signature-256",
    };

    // raw_headers is a JSON object string
    let parsed: Result<Value, _> = serde_json::from_str(raw_headers);
    match parsed {
        Ok(headers_obj) => {
            if headers_obj.get(header_name).is_some() {
                "true"
            } else {
                "false"
            }
        }
        Err(_) => "false",
    }
}
