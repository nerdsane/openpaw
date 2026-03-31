//! Deployment Tracker — WASM module for AlertCycle.MergeComplete and CheckDeployment.
//!
//! Polls the GitHub Deployments API for a successful deployment matching the
//! merge SHA. Self-loops via CheckDeployment until a success is found or
//! max_checks is exhausted.
//!
//! Build: `cargo build --target wasm32-unknown-unknown --release`

use temper_wasm_sdk::prelude::*;

/// Entry point.
#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        ctx.log("info", "deployment_tracker: starting");

        let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));

        // Read entity state
        let merge_sha = fields
            .get("merge_sha")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if merge_sha.is_empty() {
            return Err("deployment_tracker: merge_sha is empty".to_string());
        }

        let pr_url = fields
            .get("pr_url")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let check_count: u32 = fields
            .get("check_count")
            .and_then(|v| v.as_str())
            .unwrap_or("0")
            .parse()
            .unwrap_or(0);
        let max_checks: u32 = ctx
            .config
            .get("max_checks")
            .and_then(|v| v.parse().ok())
            .unwrap_or(40);
        let github_token = ctx
            .config
            .get("github_token")
            .filter(|s| !s.is_empty() && !s.contains("{secret:"))
            .cloned()
            .unwrap_or_default();

        if github_token.is_empty() {
            return Err("deployment_tracker: github_token not configured".to_string());
        }

        // Parse owner/repo from pr_url
        let (owner, repo) = if !pr_url.is_empty() {
            let parts: Vec<&str> = pr_url.trim_end_matches('/').split('/').collect();
            if parts.len() >= 7 {
                (
                    parts[parts.len() - 4].to_string(),
                    parts[parts.len() - 3].to_string(),
                )
            } else {
                return Err(format!(
                    "deployment_tracker: cannot parse owner/repo from pr_url: {pr_url}"
                ));
            }
        } else {
            return Err("deployment_tracker: pr_url is empty, cannot determine repo".to_string());
        };

        let gh_headers = vec![
            ("authorization".to_string(), format!("Bearer {github_token}")),
            (
                "accept".to_string(),
                "application/vnd.github+json".to_string(),
            ),
            (
                "x-github-api-version".to_string(),
                "2022-11-28".to_string(),
            ),
            ("user-agent".to_string(), "openpaw".to_string()),
        ];

        // GET deployments for the merge SHA
        let deployments_url = format!(
            "https://api.github.com/repos/{owner}/{repo}/deployments?sha={merge_sha}&per_page=20"
        );
        let dep_resp = ctx.http_call("GET", &deployments_url, &gh_headers, "")?;
        if dep_resp.status < 200 || dep_resp.status >= 300 {
            if check_count < max_checks {
                ctx.log(
                    "warn",
                    &format!(
                        "deployment_tracker: deployments API returned HTTP {}, retrying",
                        dep_resp.status
                    ),
                );
                set_success_result("CheckDeployment", &json!({}));
                return Ok(());
            }
            return Err(format!(
                "deployment_tracker: deployments API failed (HTTP {})",
                dep_resp.status
            ));
        }

        let deployments: Value = serde_json::from_str(&dep_resp.body)
            .map_err(|e| format!("deployment_tracker: parse deployments: {e}"))?;

        if let Some(items) = deployments.as_array() {
            for deployment in items {
                let statuses_url = deployment
                    .get("statuses_url")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if statuses_url.is_empty() {
                    continue;
                }

                let statuses_resp = ctx.http_call("GET", statuses_url, &gh_headers, "")?;
                if statuses_resp.status < 200 || statuses_resp.status >= 300 {
                    continue;
                }

                let statuses: Value = serde_json::from_str(&statuses_resp.body)
                    .unwrap_or(json!([]));
                if let Some(status_items) = statuses.as_array() {
                    for status in status_items {
                        let state = status
                            .get("state")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        if state == "success" {
                            // Found a successful deployment
                            let target_url = status
                                .get("environment_url")
                                .or_else(|| status.get("target_url"))
                                .or_else(|| status.get("log_url"))
                                .and_then(|v| v.as_str())
                                .unwrap_or(merge_sha);

                            ctx.log(
                                "info",
                                &format!(
                                    "deployment_tracker: deployment success, url={target_url}"
                                ),
                            );
                            set_success_result(
                                "DeployDetected",
                                &json!({ "deployment_url": target_url }),
                            );
                            return Ok(());
                        }
                    }
                }
            }
        }

        // No successful deployment found yet
        if check_count < max_checks {
            ctx.log(
                "info",
                &format!(
                    "deployment_tracker: no deployment success yet (check {}/{}), retrying",
                    check_count + 1,
                    max_checks
                ),
            );
            set_success_result("CheckDeployment", &json!({}));
        } else {
            ctx.log("info", "deployment_tracker: max checks exhausted");
            set_success_result(
                "AlertPersists",
                &json!({ "diagnosis": "deployment not detected after max checks" }),
            );
        }

        Ok(())
    })();

    if let Err(e) = result {
        set_error_result(&e);
    }
    0
}
