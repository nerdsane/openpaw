use temper_wasm_sdk::prelude::*;

/// Vercel API tool — governed access to Vercel's REST API.
///
/// Operations: deployment_status, list_deployments, redeploy, build_logs, env_vars, promote.
/// Auth: VERCEL_TOKEN from secrets vault.
pub(crate) fn execute(ctx: &Context, input: &Value) -> Result<String, String> {
    let action = input
        .get("action")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or("vercel_api: 'action' is required")?;

    let token = ctx
        .get_secret("vercel_token")
        .map_err(|_| "vercel_api: missing VERCEL_TOKEN; configure vercel_token secret")?;
    if token.trim().is_empty() || token.contains("{secret:") {
        return Err("vercel_api: VERCEL_TOKEN is empty or unresolved".to_string());
    }

    let headers = vec![
        ("authorization".to_string(), format!("Bearer {token}")),
        ("accept".to_string(), "application/json".to_string()),
    ];
    let base = "https://api.vercel.com";

    match action {
        "deployment_status" => {
            let deployment_id = require_str(input, "deployment_id")?;
            let url = format!("{base}/v13/deployments/{deployment_id}");
            let resp = ctx.http_call("GET", &url, &headers, "")?;
            check_response("deployment_status", &resp)
        }
        "list_deployments" => {
            let project_id = require_str(input, "project_id")?;
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
            let deployment_id = require_str(input, "deployment_id")?;
            let target = input
                .get("target")
                .and_then(Value::as_str)
                .unwrap_or("production");
            let body = serde_json::json!({
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
            let deployment_id = require_str(input, "deployment_id")?;
            let url = format!("{base}/v2/deployments/{deployment_id}/events");
            let resp = ctx.http_call("GET", &url, &headers, "")?;
            check_response("build_logs", &resp)
        }
        "env_vars" => {
            let project_id = require_str(input, "project_id")?;
            let url = format!("{base}/v10/projects/{project_id}/env");
            let resp = ctx.http_call("GET", &url, &headers, "")?;
            check_response("env_vars", &resp)
        }
        "promote" => {
            let project_id = require_str(input, "project_id")?;
            let deployment_id = require_str(input, "deployment_id")?;
            let url = format!("{base}/v10/projects/{project_id}/promote/{deployment_id}");
            let resp = ctx.http_call("POST", &url, &headers, "")?;
            check_response("promote", &resp)
        }
        _ => Err(format!(
            "vercel_api: unknown action '{action}'. Valid: deployment_status, list_deployments, redeploy, build_logs, env_vars, promote"
        )),
    }
}

fn require_str<'a>(input: &'a Value, key: &str) -> Result<&'a str, String> {
    input
        .get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("vercel_api: '{key}' is required"))
}

fn check_response(action: &str, resp: &HttpResponse) -> Result<String, String> {
    if resp.status >= 200 && resp.status < 300 {
        Ok(resp.body.clone())
    } else {
        Err(format!(
            "vercel_api {action}: HTTP {} — {}",
            resp.status,
            &resp.body[..resp.body.len().min(500)]
        ))
    }
}
