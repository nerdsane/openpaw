//! Railway API tool ported from tool_runner/railway.rs.
//!
//! Dispatched as `temper.railway(...)` from Monty code.

use serde_json::{Value, json};
use temper_wasm_sdk::context::Context;

pub fn railway(ctx: &Context, args: &[Value]) -> Result<Value, String> {
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
        .ok_or("railway: 'action' is required")?;

    let token = ctx.config.get("railway_token").cloned().unwrap_or_default();
    if token.trim().is_empty() || token.contains("{secret:") {
        return Err("railway: missing RAILWAY_TOKEN; configure railway_token secret".into());
    }

    let headers = vec![
        ("authorization".to_string(), format!("Bearer {token}")),
        ("content-type".to_string(), "application/json".to_string()),
    ];
    let api_url = "https://backboard.railway.com/graphql/v2";

    let query = match action {
        "deployment_status" => {
            let project_id = input_or_config(ctx, &input, "project_id")?;
            json!({
                "query": "query($projectId: String!) { project(id: $projectId) { services { edges { node { id name serviceInstances { edges { node { domains { serviceDomains { domain } } latestDeployment { id status createdAt } } } } } } } } }",
                "variables": { "projectId": project_id },
            })
            .to_string()
        }
        "service_status" => {
            let service_id = input_or_config(ctx, &input, "service_id")?;
            json!({
                "query": "query($serviceId: String!) { service(id: $serviceId) { id name updatedAt serviceInstances { edges { node { latestDeployment { id status createdAt } } } } } }",
                "variables": { "serviceId": service_id },
            })
            .to_string()
        }
        "redeploy" => {
            let deployment_id = require_str(&input, "deployment_id")?;
            json!({
                "query": "mutation($deploymentId: String!) { deploymentRedeploy(id: $deploymentId) { id status } }",
                "variables": { "deploymentId": deployment_id },
            })
            .to_string()
        }
        "logs" => {
            let deployment_id = require_str(&input, "deployment_id")?;
            json!({
                "query": "query($deploymentId: String!) { deploymentLogs(deploymentId: $deploymentId, limit: 100) { message timestamp severity } }",
                "variables": { "deploymentId": deployment_id },
            })
            .to_string()
        }
        "variables" => {
            let project_id = input_or_config(ctx, &input, "project_id")?;
            let environment_id = input_or_config(ctx, &input, "environment_id")?;
            let mut variables = json!({
                "projectId": project_id,
                "environmentId": environment_id,
            });
            if let Some(service_id) = optional_input_or_config(ctx, &input, "service_id") {
                variables["serviceId"] = json!(service_id);
            }
            json!({
                "query": "query variables($projectId: String!, $environmentId: String!, $serviceId: String) { variables(projectId: $projectId, environmentId: $environmentId, serviceId: $serviceId) }",
                "variables": variables,
            })
            .to_string()
        }
        _ => {
            return Err(format!(
                "railway: unknown action '{action}'. Valid: deployment_status, service_status, redeploy, logs, variables"
            ));
        }
    };

    let resp = ctx.http_call("POST", api_url, &headers, &query)?;
    if resp.status >= 200 && resp.status < 300 {
        serde_json::from_str(&resp.body)
            .map_err(|e| format!("railway: failed to parse response: {e}"))
    } else {
        Err(format!(
            "railway {action}: HTTP {} -- {}",
            resp.status,
            &resp.body[..resp.body.len().min(500)]
        ))
    }
}

fn require_str<'a>(input: &'a Value, key: &str) -> Result<&'a str, String> {
    input
        .get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("railway: '{key}' is required"))
}

fn optional_input_or_config(ctx: &Context, input: &Value, key: &str) -> Option<String> {
    input
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            ctx.config
                .get(&format!("railway_{key}"))
                .map(String::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty() && !value.contains("{secret:"))
                .map(str::to_string)
        })
}

fn input_or_config(ctx: &Context, input: &Value, key: &str) -> Result<String, String> {
    optional_input_or_config(ctx, input, key).ok_or_else(|| format!("railway: '{key}' is required"))
}
