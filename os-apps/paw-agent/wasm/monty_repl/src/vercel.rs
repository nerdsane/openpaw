//! Vercel API tool ported from tool_runner/vercel.rs.
//!
//! Dispatched as `temper.vercel(...)` from Monty code.

use serde_json::{Value, json};
use temper_wasm_sdk::context::Context;

pub fn vercel(ctx: &Context, args: &[Value]) -> Result<Value, String> {
    let input = args
        .first()
        .filter(|v| v.is_object())
        .cloned()
        .unwrap_or(json!({}));

    let action = input
        .get("action")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or("vercel: 'action' is required")?;

    let token = ctx
        .config
        .get("vercel_token")
        .cloned()
        .unwrap_or_default();
    if token.trim().is_empty() || token.contains("{secret:") {
        return Err("vercel: missing VERCEL_TOKEN; configure vercel_token secret".into());
    }

    let headers = vec![
        ("authorization".to_string(), format!("Bearer {token}")),
        ("accept".to_string(), "application/json".to_string()),
    ];
    let base = "https://api.vercel.com";

    match action {
        "deployment_status" => {
            let deployment_id = require_str(&input, "deployment_id")?;
            let url = format!("{base}/v13/deployments/{deployment_id}");
            let resp = ctx.http_call("GET", &url, &headers, "")?;
            check_response("deployment_status", &resp)
        }
        "list_deployments" => {
            let project_id = require_str(&input, "project_id")?;
            let limit = input
                .get("limit")
                .and_then(Value::as_u64)
                .unwrap_or(10)
                .clamp(1, 100);
            let url = format!("{base}/v6/deployments?projectId={project_id}&limit={limit}");
            let resp = ctx.http_call("GET", &url, &headers, "")?;
            check_response("list_deployments", &resp)
        }
        "redeploy" => {
            let deployment_id = require_str(&input, "deployment_id")?;
            let target = input
                .get("target")
                .and_then(Value::as_str)
                .unwrap_or("production");
            let body = json!({
                "deploymentId": deployment_id,
                "target": target
            });
            let mut post_headers = headers.clone();
            post_headers.push(("content-type".to_string(), "application/json".to_string()));
            let url = format!("{base}/v13/deployments");
            let resp = ctx.http_call("POST", &url, &post_headers, &body.to_string())?;
            check_response("redeploy", &resp)
        }
        "build_logs" => {
            let deployment_id = require_str(&input, "deployment_id")?;
            let url = format!("{base}/v2/deployments/{deployment_id}/events");
            let resp = ctx.http_call("GET", &url, &headers, "")?;
            check_response("build_logs", &resp)
        }
        "env_vars" => {
            let project_id = require_str(&input, "project_id")?;
            let url = format!("{base}/v10/projects/{project_id}/env");
            let resp = ctx.http_call("GET", &url, &headers, "")?;
            check_response("env_vars", &resp)
        }
        "promote" => {
            let project_id = require_str(&input, "project_id")?;
            let deployment_id = require_str(&input, "deployment_id")?;
            let url = format!("{base}/v10/projects/{project_id}/promote/{deployment_id}");
            let resp = ctx.http_call("POST", &url, &headers, "")?;
            check_response("promote", &resp)
        }
        _ => Err(format!(
            "vercel: unknown action '{action}'. Valid: deployment_status, list_deployments, redeploy, build_logs, env_vars, promote"
        )),
    }
}

fn require_str<'a>(input: &'a Value, key: &str) -> Result<&'a str, String> {
    input
        .get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("vercel: '{key}' is required"))
}

fn check_response(action: &str, resp: &temper_wasm_sdk::context::HttpResponse) -> Result<Value, String> {
    if resp.status >= 200 && resp.status < 300 {
        serde_json::from_str(&resp.body)
            .map_err(|e| format!("vercel {action}: failed to parse response: {e}"))
    } else {
        Err(format!(
            "vercel {action}: HTTP {} -- {}",
            resp.status,
            &resp.body[..resp.body.len().min(500)]
        ))
    }
}
