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
        let headers = odata_headers(&ctx, tenant);

        // Build action params from the normalized payload.
        let action_params = build_action_params(
            target_entity_type,
            target_action,
            normalized_payload,
            route_key,
        );

        // Dispatch the target action via OData POST
        let action_url = format!(
            "{}/tdata/{}s('{}')/{}",
            temper_api_url, target_entity_type, target_entity_id, target_action
        );

        ctx.log(
            "info",
            &format!(
                "process_webhook: dispatching {} on {}({})",
                target_action, target_entity_type, target_entity_id
            ),
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
fn odata_headers(ctx: &Context, tenant: &str) -> Vec<(String, String)> {
    vec![
        ("content-type".to_string(), "application/json".to_string()),
        ("x-tenant-id".to_string(), tenant.to_string()),
        ("x-temper-principal-kind".to_string(), "agent".to_string()),
        ("x-temper-principal-id".to_string(), ctx.entity_id.clone()),
        ("x-temper-agent-type".to_string(), "system".to_string()),
    ]
}

/// Build action parameters for the target entity action based on the normalized payload.
/// Patrol actions get their typed params; legacy alert actions keep alert_payload.
fn build_action_params(
    target_entity_type: &str,
    target_action: &str,
    normalized_payload: &str,
    route_key: &str,
) -> Value {
    let payload = parse_payload(normalized_payload);
    let action_name = target_action.rsplit('.').next().unwrap_or(target_action);

    match (target_entity_type, action_name) {
        ("WorkRequest" | "PatrolRequest", "Submit") => {
            build_patrol_request_submit_params(&payload, normalized_payload, route_key)
        }
        ("Signal", "Ingest") => build_signal_ingest_params(&payload, normalized_payload, route_key),
        (_, "Open") => {
            // AlertCycle.Open expects monitor_id and alert_payload
            let monitor_id =
                first_string(&payload, &["monitor_id", "dd_monitor_id"]).unwrap_or_default();

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

fn build_patrol_request_submit_params(
    payload: &Value,
    normalized_payload: &str,
    route_key: &str,
) -> Value {
    let fallback_source = fallback_source(payload, route_key);
    let request_text = first_string(
        payload,
        &[
            "request_text",
            "text",
            "message",
            "body",
            "description",
            "summary",
            "title",
        ],
    )
    .or_else(|| combine_title_and_body(payload))
    .unwrap_or_else(|| normalized_payload.to_string());

    let requester_id = first_string(
        payload,
        &[
            "requester_id",
            "requester",
            "user",
            "username",
            "author",
            "actor",
        ],
    )
    .or_else(|| string_at(payload, &["sender", "login"]))
    .or_else(|| string_at(payload, &["user", "login"]))
    .unwrap_or_else(|| format!("webhook:{route_key}"));

    json!({
        "source": fallback_source,
        "request_text": request_text,
        "requester_id": requester_id,
    })
}

fn build_signal_ingest_params(payload: &Value, normalized_payload: &str, route_key: &str) -> Value {
    let fallback_source = fallback_source(payload, route_key);
    let source_url = first_string(
        payload,
        &[
            "source_url",
            "url",
            "html_url",
            "web_url",
            "log_url",
            "trace_url",
            "permalink",
        ],
    )
    .or_else(|| string_at(payload, &["repository", "html_url"]))
    .unwrap_or_default();

    let severity = first_string(
        payload,
        &["severity", "priority", "status", "alert_type", "level"],
    )
    .unwrap_or_else(|| "unknown".to_string());

    json!({
        "source": fallback_source,
        "payload": normalized_payload,
        "source_url": source_url,
        "severity": severity,
    })
}

fn parse_payload(normalized_payload: &str) -> Value {
    serde_json::from_str(normalized_payload)
        .unwrap_or_else(|_| json!({ "text": normalized_payload }))
}

fn fallback_source(payload: &Value, route_key: &str) -> String {
    first_string(payload, &["source", "source_type", "provider"])
        .unwrap_or_else(|| route_key.trim_start_matches("patrol-").to_string())
}

fn combine_title_and_body(payload: &Value) -> Option<String> {
    let title = first_string(payload, &["title"])?;
    let body = first_string(payload, &["body", "description"]).unwrap_or_default();
    if body.is_empty() {
        Some(title)
    } else {
        Some(format!("{title}\n\n{body}"))
    }
}

fn first_string(payload: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .filter_map(|key| value_to_string(payload.get(*key)?))
        .find(|value| !value.trim().is_empty())
}

fn string_at(payload: &Value, path: &[&str]) -> Option<String> {
    let mut cursor = payload;
    for key in path {
        cursor = cursor.get(*key)?;
    }
    value_to_string(cursor).filter(|value| !value.trim().is_empty())
}

fn value_to_string(value: &Value) -> Option<String> {
    match value {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        Value::Null => None,
        Value::Array(_) | Value::Object(_) => Some(value.to_string()),
    }
}
