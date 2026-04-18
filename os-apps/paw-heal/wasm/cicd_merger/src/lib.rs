//! CICD Merger — WASM module for AlertCycle.BeginMerge and CheckMergeReady.
//!
//! Polls GitHub for PR merge readiness (check runs, combined status, mergeable
//! flag). When ready, squash-merges the PR. Self-loops via CheckMergeReady
//! until checks pass or max_checks is exhausted.
//!
//! Build: `cargo build --target wasm32-unknown-unknown --release`

use temper_wasm_sdk::prelude::*;

/// Entry point.
#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        ctx.log("info", "cicd_merger: starting");

        let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));

        // Read entity state
        let pr_url = fields
            .get("pr_url")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if pr_url.is_empty() {
            return Err("cicd_merger: pr_url is empty".to_string());
        }

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
            return Err("cicd_merger: github_token not configured".to_string());
        }

        // Parse GitHub PR URL: .../owner/repo/pull/number
        let (owner, repo, number) = parse_pr_url(pr_url)?;

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
            ("user-agent".to_string(), "temperpaw".to_string()),
        ];

        // GET the PR
        let pr_api_url = format!(
            "https://api.github.com/repos/{owner}/{repo}/pulls/{number}"
        );
        let pr_resp = ctx.http_call("GET", &pr_api_url, &gh_headers, "")?;
        if pr_resp.status < 200 || pr_resp.status >= 300 {
            return Err(format!(
                "cicd_merger: GET PR failed (HTTP {}): {}",
                pr_resp.status,
                &pr_resp.body[..pr_resp.body.len().min(300)]
            ));
        }
        let pr: Value = serde_json::from_str(&pr_resp.body)
            .map_err(|e| format!("cicd_merger: failed to parse PR JSON: {e}"))?;

        let mergeable = pr.get("mergeable").and_then(|v| v.as_bool());
        let mergeable_state = pr
            .get("mergeable_state")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let head_sha = pr
            .get("head")
            .and_then(|v| v.get("sha"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // Check: check-runs all green
        let checks_ok = if !head_sha.is_empty() {
            check_runs_green(&ctx, &gh_headers, &owner, &repo, head_sha)?
        } else {
            false
        };

        // Check: combined status is success
        let status_ok = if !head_sha.is_empty() {
            combined_status_green(&ctx, &gh_headers, &owner, &repo, head_sha)?
        } else {
            false
        };

        let mergeable_ok = mergeable == Some(true)
            && matches!(mergeable_state, "clean" | "unstable" | "has_hooks");

        if mergeable_ok && checks_ok && status_ok {
            // Ready to merge — squash merge
            ctx.log("info", "cicd_merger: PR is merge-ready, squash-merging");
            let merge_url = format!(
                "https://api.github.com/repos/{owner}/{repo}/pulls/{number}/merge"
            );
            let merge_body = json!({ "merge_method": "squash" });
            let merge_resp =
                ctx.http_call("PUT", &merge_url, &gh_headers, &merge_body.to_string())?;
            if merge_resp.status < 200 || merge_resp.status >= 300 {
                return Err(format!(
                    "cicd_merger: squash merge failed (HTTP {}): {}",
                    merge_resp.status,
                    &merge_resp.body[..merge_resp.body.len().min(300)]
                ));
            }
            let merge_parsed: Value = serde_json::from_str(&merge_resp.body)
                .map_err(|e| format!("cicd_merger: parse merge response: {e}"))?;
            let sha = merge_parsed
                .get("sha")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            ctx.log(
                "info",
                &format!("cicd_merger: merged successfully, sha={sha}"),
            );
            set_success_result("MergeComplete", &json!({ "merge_sha": sha }));
        } else if check_count < max_checks {
            // Not ready yet — self-loop
            ctx.log(
                "info",
                &format!(
                    "cicd_merger: not merge-ready (check {}/{}), retrying",
                    check_count + 1,
                    max_checks
                ),
            );
            set_success_result("CheckMergeReady", &json!({}));
        } else {
            // Exhausted retries
            ctx.log("info", "cicd_merger: max checks exhausted, giving up");
            set_success_result(
                "AlertPersists",
                &json!({ "diagnosis": "PR never became merge-ready after max checks" }),
            );
        }

        Ok(())
    })();

    if let Err(e) = result {
        set_error_result(&e);
    }
    0
}

/// Parse a GitHub PR URL into (owner, repo, number).
fn parse_pr_url(pr_url: &str) -> Result<(String, String, String), String> {
    let parts: Vec<&str> = pr_url.trim_end_matches('/').split('/').collect();
    if parts.len() < 7 || parts[parts.len() - 2] != "pull" {
        return Err(format!("cicd_merger: unsupported GitHub PR URL: {pr_url}"));
    }
    Ok((
        parts[parts.len() - 4].to_string(),
        parts[parts.len() - 3].to_string(),
        parts[parts.len() - 1].to_string(),
    ))
}

/// Check if all check-runs on a commit are green.
fn check_runs_green(
    ctx: &Context,
    headers: &[(String, String)],
    owner: &str,
    repo: &str,
    head_sha: &str,
) -> Result<bool, String> {
    let url = format!(
        "https://api.github.com/repos/{owner}/{repo}/commits/{head_sha}/check-runs?per_page=100"
    );
    let resp = ctx.http_call("GET", &url, headers, "")?;
    if resp.status < 200 || resp.status >= 300 {
        return Ok(false);
    }
    let body: Value =
        serde_json::from_str(&resp.body).map_err(|e| format!("parse check-runs: {e}"))?;
    let Some(runs) = body.get("check_runs").and_then(|v| v.as_array()) else {
        return Ok(true);
    };
    if runs.is_empty() {
        return Ok(true);
    }
    Ok(runs.iter().all(|run| {
        let status = run.get("status").and_then(|v| v.as_str()).unwrap_or("");
        let conclusion = run.get("conclusion").and_then(|v| v.as_str()).unwrap_or("");
        status == "completed" && matches!(conclusion, "success" | "neutral" | "skipped")
    }))
}

/// Check if the combined commit status is green.
fn combined_status_green(
    ctx: &Context,
    headers: &[(String, String)],
    owner: &str,
    repo: &str,
    head_sha: &str,
) -> Result<bool, String> {
    let url = format!(
        "https://api.github.com/repos/{owner}/{repo}/commits/{head_sha}/status"
    );
    let resp = ctx.http_call("GET", &url, headers, "")?;
    if resp.status < 200 || resp.status >= 300 {
        return Ok(false);
    }
    let body: Value =
        serde_json::from_str(&resp.body).map_err(|e| format!("parse combined status: {e}"))?;
    Ok(matches!(
        body.get("state").and_then(|v| v.as_str()).unwrap_or("success"),
        "success" | ""
    ))
}
