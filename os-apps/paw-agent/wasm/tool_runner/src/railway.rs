use temper_wasm_sdk::prelude::*;

/// Railway API tool — governed access to Railway's GraphQL API.
///
/// Operations: deployment_status, service_status, redeploy, logs, variables.
/// Auth: RAILWAY_TOKEN from secrets vault.
pub(crate) fn execute(ctx: &Context, input: &Value) -> Result<String, String> {
    let action = input
        .get("action")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or("railway_api: 'action' is required")?;

    let token = ctx
        .get_secret("railway_token")
        .map_err(|_| "railway_api: missing RAILWAY_TOKEN; configure railway_token secret")?;
    if token.trim().is_empty() || token.contains("{secret:") {
        return Err("railway_api: RAILWAY_TOKEN is empty or unresolved".to_string());
    }

    let headers = vec![
        ("authorization".to_string(), format!("Bearer {token}")),
        ("content-type".to_string(), "application/json".to_string()),
    ];
    let api_url = "https://backboard.railway.com/graphql/v2";

    match action {
        "deployment_status" => {
            let project_id = require_str(input, "project_id")?;
            let query = format!(
                r#"{{"query":"query {{ project(id: \"{project_id}\") {{ services {{ edges {{ node {{ id name serviceInstances {{ edges {{ node {{ domains {{ serviceDomains {{ domain }} }} latestDeployment {{ id status createdAt }} }} }} }} }} }} }} }}"}}"#
            );
            let resp = ctx.http_call("POST", api_url, &headers, &query)?;
            check_response("deployment_status", &resp)
        }
        "service_status" => {
            let service_id = require_str(input, "service_id")?;
            let query = format!(
                r#"{{"query":"query {{ service(id: \"{service_id}\") {{ id name updatedAt serviceInstances {{ edges {{ node {{ latestDeployment {{ id status createdAt }} }} }} }} }} }}"}}"#
            );
            let resp = ctx.http_call("POST", api_url, &headers, &query)?;
            check_response("service_status", &resp)
        }
        "redeploy" => {
            let deployment_id = require_str(input, "deployment_id")?;
            let query = format!(
                r#"{{"query":"mutation {{ deploymentRedeploy(id: \"{deployment_id}\") {{ id status }} }}"}}"#
            );
            let resp = ctx.http_call("POST", api_url, &headers, &query)?;
            check_response("redeploy", &resp)
        }
        "logs" => {
            let deployment_id = require_str(input, "deployment_id")?;
            let query = format!(
                r#"{{"query":"query {{ deploymentLogs(deploymentId: \"{deployment_id}\", limit: 100) {{ message timestamp severity }} }}"}}"#
            );
            let resp = ctx.http_call("POST", api_url, &headers, &query)?;
            check_response("logs", &resp)
        }
        "variables" => {
            let service_id = require_str(input, "service_id")?;
            let environment_id = input
                .get("environment_id")
                .and_then(Value::as_str)
                .unwrap_or("");
            let query = if environment_id.is_empty() {
                format!(
                    r#"{{"query":"query {{ variables(serviceId: \"{service_id}\") }}"}}"#
                )
            } else {
                format!(
                    r#"{{"query":"query {{ variables(serviceId: \"{service_id}\", environmentId: \"{environment_id}\") }}"}}"#
                )
            };
            let resp = ctx.http_call("POST", api_url, &headers, &query)?;
            check_response("variables", &resp)
        }
        _ => Err(format!(
            "railway_api: unknown action '{action}'. Valid: deployment_status, service_status, redeploy, logs, variables"
        )),
    }
}

fn require_str<'a>(input: &'a Value, key: &str) -> Result<&'a str, String> {
    input
        .get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("railway_api: '{key}' is required"))
}

fn check_response(action: &str, resp: &HttpResponse) -> Result<String, String> {
    if resp.status >= 200 && resp.status < 300 {
        Ok(resp.body.clone())
    } else {
        Err(format!(
            "railway_api {action}: HTTP {} — {}",
            resp.status,
            &resp.body[..resp.body.len().min(500)]
        ))
    }
}
