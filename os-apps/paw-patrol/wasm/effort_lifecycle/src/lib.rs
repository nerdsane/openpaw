//! Effort Lifecycle - chain-file guards for the entity loop (ARN-441 1b).
//!
//! Kernel guards cannot read files, so the design-chain doors are enforced here:
//! `Specify` and `Plan` each fire this module, which checks that the attached
//! reference (`spec_ref` / `plan_ref`) names a paw-fs `File` in the `Ready` state.
//! On a missing or not-yet-ready file this module ERRORS, and the transition's
//! `on_failure` (RejectSpec / RejectPlan) rolls the Effort back - the state machine
//! refuses on a missing chain file. This module only VALIDATES and returns; it never
//! dispatches transitions (one integration, one concern).

use temper_wasm_sdk::prelude::*;

const FILES_PATH: &str = "/tdata/Files";

#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        let base_url = resolve_api_url(&ctx);
        let headers = odata_headers(&ctx);
        let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));

        match ctx.trigger_action.as_str() {
            // intent.md is the first chain file - validated at birth (Seed); a
            // missing/not-Ready intent.md Abandons the just-born Effort (on_failure).
            "Seed" => require_ready_file(
                &ctx, &base_url, &headers, &fields, "intent_ref", "IntentRef", "intent.md",
            ),
            "Specify" => require_ready_file(
                &ctx, &base_url, &headers, &fields, "spec_ref", "SpecRef", "spec.md",
            ),
            "Plan" => require_ready_file(
                &ctx, &base_url, &headers, &fields, "plan_ref", "PlanRef", "plan.md",
            ),
            other => Err(format!("effort_lifecycle: unsupported trigger {other}")),
        }?;

        set_success_result("", &json!({ "status": "effort_chain_file_ready" }));
        Ok(())
    })();

    if let Err(error) = result {
        set_error_result(&error);
    }
    0
}

/// Refuse unless `<ref_field>` names a paw-fs File whose Status is `Ready`.
fn require_ready_file(
    ctx: &Context,
    base_url: &str,
    headers: &[(String, String)],
    fields: &Value,
    snake: &str,
    pascal: &str,
    label: &str,
) -> Result<(), String> {
    let effort_id = entity_id(ctx);
    let file_ref = string_field(fields, snake, pascal);
    if file_ref.trim().is_empty() {
        return Err(format!(
            "effort_lifecycle: Effort {effort_id} has no {label} reference ({snake} is empty)"
        ));
    }

    let status = get_status(ctx, base_url, headers, entity_set(FILES_PATH), &file_ref)?;
    if status != "Ready" {
        return Err(format!(
            "effort_lifecycle: {label} File {file_ref} for Effort {effort_id} is not Ready (status: {})",
            if status.is_empty() { "missing" } else { &status }
        ));
    }

    ctx.log(
        "info",
        &format!("effort_lifecycle: {label} File {file_ref} is Ready for Effort {effort_id}"),
    );
    Ok(())
}

fn entity_id(ctx: &Context) -> String {
    ctx.entity_state
        .get("entity_id")
        .and_then(Value::as_str)
        .unwrap_or(&ctx.entity_id)
        .to_string()
}

fn string_field(fields: &Value, snake: &str, pascal: &str) -> String {
    fields
        .get(snake)
        .and_then(Value::as_str)
        .or_else(|| fields.get(pascal).and_then(Value::as_str))
        .unwrap_or("")
        .to_string()
}

fn resolve_api_url(ctx: &Context) -> String {
    ctx.config
        .get("temper_api_url")
        .filter(|value| !value.is_empty() && !value.contains("{secret:"))
        .cloned()
        .unwrap_or_else(|| "http://127.0.0.1:3000".to_string())
}

fn odata_headers(ctx: &Context) -> Vec<(String, String)> {
    vec![
        ("content-type".to_string(), "application/json".to_string()),
        ("x-tenant-id".to_string(), ctx.tenant.clone()),
        ("x-temper-principal-kind".to_string(), "agent".to_string()),
        ("x-temper-principal-id".to_string(), ctx.entity_id.clone()),
        ("x-temper-agent-type".to_string(), "system".to_string()),
    ]
}

fn entity_set(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

fn get_status(
    ctx: &Context,
    base_url: &str,
    headers: &[(String, String)],
    entity_set: &str,
    entity_id: &str,
) -> Result<String, String> {
    let url = format!("{base_url}/tdata/{entity_set}('{entity_id}')");
    let resp = ctx.http_call("GET", &url, headers, "")?;
    if resp.status == 404 {
        return Ok(String::new());
    }
    if resp.status < 200 || resp.status >= 300 {
        return Err(format!(
            "get {entity_set}('{entity_id}') failed with HTTP {}: {}",
            resp.status,
            truncate(&resp.body, 300)
        ));
    }
    let body: Value = serde_json::from_str(&resp.body)
        .map_err(|err| format!("get {entity_set}('{entity_id}'): parse response: {err}"))?;
    Ok(status_from_response(&body))
}

fn status_from_response(value: &Value) -> String {
    value
        .get("Status")
        .or_else(|| value.get("status"))
        .or_else(|| value.pointer("/fields/Status"))
        .or_else(|| value.pointer("/fields/status"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn truncate(input: &str, max: usize) -> String {
    if input.len() <= max {
        input.to_string()
    } else {
        format!("{}[truncated]", input.chars().take(max).collect::<String>())
    }
}
