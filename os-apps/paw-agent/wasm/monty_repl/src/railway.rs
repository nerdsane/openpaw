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
            let project_id = require_str(&input, "project_id")?;
            format!(
                r#"{{"query":"query {{ project(id: \"{project_id}\") {{ services {{ edges {{ node {{ id name serviceInstances {{ edges {{ node {{ domains {{ serviceDomains {{ domain }} }} latestDeployment {{ id status createdAt }} }} }} }} }} }} }} }}"}}"#
            )
        }
        "service_status" => {
            let service_id = require_str(&input, "service_id")?;
            format!(
                r#"{{"query":"query {{ service(id: \"{service_id}\") {{ id name updatedAt serviceInstances {{ edges {{ node {{ latestDeployment {{ id status createdAt }} }} }} }} }} }}"}}"#
            )
        }
        "redeploy" => {
            let deployment_id = require_str(&input, "deployment_id")?;
            format!(
                r#"{{"query":"mutation {{ deploymentRedeploy(id: \"{deployment_id}\") {{ id status }} }}"}}"#
            )
        }
        "logs" => {
            let deployment_id = require_str(&input, "deployment_id")?;
            format!(
                r#"{{"query":"query {{ deploymentLogs(deploymentId: \"{deployment_id}\", limit: 100) {{ message timestamp severity }} }}"}}"#
            )
        }
        "variables" => {
            let service_id = require_str(&input, "service_id")?;
            let environment_id = input
                .get("environment_id")
                .and_then(Value::as_str)
                .unwrap_or("");
            if environment_id.is_empty() {
                format!(r#"{{"query":"query {{ variables(serviceId: \"{service_id}\") }}"}}"#)
            } else {
                format!(
                    r#"{{"query":"query {{ variables(serviceId: \"{service_id}\", environmentId: \"{environment_id}\") }}"}}"#
                )
            }
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
