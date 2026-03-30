//! Webhook ingestion routes for Open Paw.
//!
//! `POST /webhooks/ingest` accepts external alerting and GitHub webhook payloads,
//! then re-enters the platform through its own OData API. This keeps webhook-driven
//! workflows governed by the same entity actions used everywhere else.

use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use axum::Router;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::time::sleep;

type HmacSha256 = Hmac<Sha256>;

const DEFAULT_AGENT_MODEL: &str = "claude-sonnet-4-20250514";
const DEFAULT_SRE_WORKDIR: &str = "/tmp/openpaw-sre-webhook";
const DEFAULT_DEVELOPER_WORKDIR: &str = "/tmp/openpaw-self-heal";

#[derive(Clone, Debug)]
pub struct WebhookState {
    odata: ODataClient,
    webhook_secret: Option<String>,
    github_token: Option<String>,
    dd_api_key: Option<String>,
    dd_app_key: Option<String>,
    dd_site: String,
}

impl WebhookState {
    pub fn new(
        base_url: String,
        tenant: String,
        api_key: Option<String>,
        webhook_secret: Option<String>,
        github_token: Option<String>,
        dd_api_key: Option<String>,
        dd_app_key: Option<String>,
        dd_site: String,
    ) -> Self {
        Self {
            odata: ODataClient::new(reqwest::Client::new(), base_url, tenant, api_key),
            webhook_secret,
            github_token,
            dd_api_key,
            dd_app_key,
            dd_site,
        }
    }
}

pub fn build_webhook_router(state: WebhookState) -> Router {
    Router::new()
        .route("/webhooks/ingest", post(ingest_webhook))
        .with_state(state)
}

#[derive(Debug, Deserialize)]
struct WebhookPayload {
    source: String,
    event_type: String,
    payload: Value,
}

#[derive(Debug, Serialize)]
struct WebhookIngestResponse {
    accepted: bool,
    outcome: String,
    message: String,
    duplicate: bool,
    monitor_id: Option<String>,
    alert_cycle_id: Option<String>,
    sre_agent_id: Option<String>,
    work_cycle_id: Option<String>,
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }

    fn unauthorized(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            message: message.into(),
        }
    }

    fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: message.into(),
        }
    }

    fn internal(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: message.into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            axum::Json(json!({
                "accepted": false,
                "error": self.message,
            })),
        )
            .into_response()
    }
}

#[derive(Clone, Debug)]
struct ODataClient {
    http: reqwest::Client,
    base_url: String,
    tenant: String,
    api_key: Option<String>,
}

impl ODataClient {
    fn new(
        http: reqwest::Client,
        base_url: String,
        tenant: String,
        api_key: Option<String>,
    ) -> Self {
        Self {
            http,
            base_url: base_url.trim_end_matches('/').to_string(),
            tenant,
            api_key,
        }
    }

    fn build_request(&self, method: reqwest::Method, url: &str) -> reqwest::RequestBuilder {
        let mut req = self
            .http
            .request(method, url)
            .header("x-tenant-id", &self.tenant)
            .header("accept", "application/json");
        if let Some(api_key) = &self.api_key {
            req = req.header("authorization", format!("Bearer {api_key}"));
        } else {
            req = req.header("x-temper-principal-kind", "admin");
        }
        req
    }

    async fn get_json(&self, url: &str) -> Result<Value> {
        let resp = self
            .build_request(reqwest::Method::GET, url)
            .send()
            .await
            .with_context(|| format!("GET {url} failed"))?;
        let status = resp.status();
        let text = resp.text().await.context("failed reading GET body")?;
        if !status.is_success() {
            bail!("GET {url} returned {status}: {text}");
        }
        if text.trim().is_empty() {
            return Ok(Value::Null);
        }
        serde_json::from_str(&text).context("failed to parse GET JSON body")
    }

    async fn post_json(&self, url: &str, body: Value) -> Result<Value> {
        let resp = self
            .build_request(reqwest::Method::POST, url)
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .with_context(|| format!("POST {url} failed"))?;
        let status = resp.status();
        let text = resp.text().await.context("failed reading POST body")?;
        if !status.is_success() {
            bail!("POST {url} returned {status}: {text}");
        }
        if text.trim().is_empty() {
            return Ok(Value::Null);
        }
        serde_json::from_str(&text).context("failed to parse POST JSON body")
    }

    async fn create_entity(&self, entity_set: &str, body: Value) -> Result<Value> {
        self.post_json(&format!("{}/tdata/{entity_set}", self.base_url), body)
            .await
    }

    async fn get_entity(&self, entity_set: &str, entity_id: &str) -> Result<Value> {
        self.get_json(&format!(
            "{}/tdata/{}('{}')",
            self.base_url, entity_set, entity_id
        ))
        .await
    }

    async fn dispatch_action(
        &self,
        entity_set: &str,
        entity_id: &str,
        action_name: &str,
        body: Value,
    ) -> Result<Value> {
        self.post_json(
            &format!(
                "{}/tdata/{}('{}')/{}",
                self.base_url, entity_set, entity_id, action_name
            ),
            body,
        )
        .await
    }

    async fn query_entities(
        &self,
        entity_set: &str,
        filter: Option<&str>,
        orderby: Option<&str>,
        top: Option<usize>,
    ) -> Result<Vec<Value>> {
        let mut params = Vec::new();
        if let Some(filter) = filter.filter(|value| !value.is_empty()) {
            params.push(format!("$filter={}", urlencoding::encode(filter)));
        }
        if let Some(orderby) = orderby.filter(|value| !value.is_empty()) {
            params.push(format!("$orderby={}", urlencoding::encode(orderby)));
        }
        if let Some(top) = top {
            params.push(format!("$top={top}"));
        }
        let suffix = if params.is_empty() {
            String::new()
        } else {
            format!("?{}", params.join("&"))
        };
        let body = self
            .get_json(&format!("{}/tdata/{}{}", self.base_url, entity_set, suffix))
            .await?;
        Ok(body
            .get("value")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default())
    }

    async fn wait_for_agent_terminal(&self, agent_id: &str, timeout: Duration) -> Result<Value> {
        let mut remaining_ms = timeout.as_millis().max(1_000) as u64;
        loop {
            let chunk_ms = remaining_ms.min(300_000);
            let wait_url = format!(
                "{}/observe/entities/Agent/{agent_id}/wait?statuses=Completed,Failed,Cancelled&timeout_ms={chunk_ms}&poll_ms=250",
                self.base_url
            );
            let payload = self.get_json(&wait_url).await?;
            let status = entity_status(&payload);
            if matches!(
                status.as_deref(),
                Some("Completed" | "Failed" | "Cancelled")
            ) {
                return Ok(payload);
            }
            let timed_out = payload
                .get("timed_out")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            if !timed_out {
                return Ok(payload);
            }
            if remaining_ms <= chunk_ms {
                return Ok(payload);
            }
            remaining_ms -= chunk_ms;
        }
    }
}

#[derive(Clone, Debug)]
struct ReportTarget {
    channel_entity_id: String,
    thread_id: String,
}

#[derive(Clone, Debug)]
struct AlertContext {
    project_harness_id: Option<String>,
    repo_url: Option<String>,
    developer_sandbox_url: Option<String>,
    developer_workdir: String,
    severity: Option<String>,
    summary: Option<String>,
    failure: Option<String>,
    report_target: Option<ReportTarget>,
}

async fn ingest_webhook(
    State(state): State<WebhookState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<axum::Json<WebhookIngestResponse>, ApiError> {
    if let Some(secret) = state.webhook_secret.as_deref() {
        verify_signature(secret, &headers, &body)
            .map_err(|error| ApiError::unauthorized(error.to_string()))?;
    }

    let envelope = normalize_webhook_envelope(&headers, &body)?;

    let response = match envelope.event_type.as_str() {
        "pull_request.merged" => handle_github_merge(&state, &envelope).await,
        _ => handle_alert_ingest(&state, &envelope).await,
    }
    .map_err(map_handler_error)?;

    Ok(axum::Json(response))
}

async fn handle_alert_ingest(
    state: &WebhookState,
    envelope: &WebhookPayload,
) -> Result<WebhookIngestResponse> {
    let monitor_key = extract_monitor_key(&envelope.payload).unwrap_or_else(|| {
        let digest = Sha256::digest(canonical_json_string(&envelope.payload));
        format!(
            "{}:{}:{}",
            envelope.source,
            envelope.event_type,
            &hex::encode(digest)[..12]
        )
    });
    let canonical_payload = canonical_json_string(&envelope.payload);
    let monitor = resolve_or_create_monitor(state, &monitor_key, &envelope.payload).await?;
    let monitor_id = entity_id(&monitor)
        .context("monitor create/query did not return an entity id")?
        .to_string();

    if is_datadog_recovery_payload(envelope) {
        let outcome = resolve_recovered_alert_cycle(state, &monitor_id, &envelope.payload).await?;
        return Ok(WebhookIngestResponse {
            accepted: true,
            outcome,
            message: "Datadog recovery payload processed".to_string(),
            duplicate: false,
            monitor_id: Some(monitor_id),
            alert_cycle_id: None,
            sre_agent_id: None,
            work_cycle_id: None,
        });
    }

    if let Some(existing) =
        find_duplicate_alert_cycle(state, &monitor_id, &canonical_payload).await?
    {
        let alert_cycle_id = entity_id(&existing)
            .context("duplicate alert cycle missing entity id")?
            .to_string();
        let sre_agent_id =
            entity_field_str(&existing, &["sre_agent_id", "SreAgentId"]).map(ToOwned::to_owned);
        return Ok(WebhookIngestResponse {
            accepted: true,
            outcome: "duplicate_alert".to_string(),
            message: "Duplicate alert payload ignored".to_string(),
            duplicate: true,
            monitor_id: Some(monitor_id),
            alert_cycle_id: Some(alert_cycle_id),
            sre_agent_id,
            work_cycle_id: None,
        });
    }

    let alert_cycle = state.odata.create_entity("AlertCycles", json!({})).await?;
    let alert_cycle_id = entity_id(&alert_cycle)
        .context("alert cycle creation did not return an entity id")?
        .to_string();
    let alert_context = resolve_alert_context(state, &envelope.payload).await?;
    let sre_agent_id = match spawn_sre_agent(
        state,
        &monitor_id,
        &alert_cycle_id,
        &canonical_payload,
        &alert_context,
    )
    .await
    {
        Ok(agent_id) => agent_id,
        Err(error) => {
            tracing::warn!(%error, monitor_id, alert_cycle_id, "webhook alert will remain open without auto-spawned SRE");
            None
        }
    };

    state
        .odata
        .dispatch_action(
            "Monitors",
            &monitor_id,
            "OpenPaw.Heal.AlertFired",
            json!({ "last_alert_payload": canonical_payload.clone() }),
        )
        .await?;

    state
        .odata
        .dispatch_action(
            "AlertCycles",
            &alert_cycle_id,
            "OpenPaw.Heal.Open",
            json!({
                "monitor_id": monitor_id,
                "alert_payload": canonical_payload,
                "sre_agent_id": sre_agent_id.clone().unwrap_or_default(),
            }),
        )
        .await?;

    if let Some(sre_agent_id) = sre_agent_id.clone() {
        state
            .odata
            .dispatch_action("Agents", &sre_agent_id, "OpenPaw.Provision", json!({}))
            .await?;
        spawn_sre_completion_watcher(
            state.clone(),
            alert_cycle_id.clone(),
            sre_agent_id.clone(),
            alert_context.project_harness_id.clone(),
            alert_context.repo_url.clone(),
            alert_context.report_target.clone(),
        );
    }

    Ok(WebhookIngestResponse {
        accepted: true,
        outcome: "alert_opened".to_string(),
        message: "AlertCycle opened from webhook payload".to_string(),
        duplicate: false,
        monitor_id: Some(monitor_id),
        alert_cycle_id: Some(alert_cycle_id),
        sre_agent_id,
        work_cycle_id: None,
    })
}

async fn handle_github_merge(
    state: &WebhookState,
    envelope: &WebhookPayload,
) -> Result<WebhookIngestResponse> {
    let work_cycle_id =
        if let Some(id) = extract_string(&envelope.payload, &["work_cycle_id", "workCycleId"]) {
            id.to_string()
        } else {
            let pr_url = extract_pr_url(&envelope.payload)
                .context("missing pr_url/html_url in merged PR payload")?;
            let filter = format!("pr_url eq '{}'", escape_odata_string(&pr_url));
            let work_cycle = state
                .odata
                .query_entities(
                    "WorkCycles",
                    Some(&filter),
                    Some("sequence_nr desc"),
                    Some(1),
                )
                .await?
                .into_iter()
                .next()
                .context("no WorkCycle matched the merged PR payload")?;
            entity_id(&work_cycle)
                .context("matched WorkCycle missing entity id")?
                .to_string()
        };

    let work_cycle = state.odata.get_entity("WorkCycles", &work_cycle_id).await?;
    let work_cycle_status = entity_status(&work_cycle).unwrap_or_default();
    let pr_url = extract_pr_url(&envelope.payload).unwrap_or_else(|| {
        entity_field_str(&work_cycle, &["pr_url", "PrUrl"])
            .unwrap_or("")
            .to_string()
    });

    let outcome = match work_cycle_status.as_str() {
        "Reviewing" => {
            let approver = extract_string(
                &envelope.payload,
                &["merged_by.login", "sender.login", "merged_by", "sender"],
            )
            .unwrap_or("github-merge-webhook");
            state
                .odata
                .dispatch_action(
                    "WorkCycles",
                    &work_cycle_id,
                    "OpenPaw.Harness.Approve",
                    json!({
                        "approver_id": approver,
                        "pr_url": pr_url,
                    }),
                )
                .await?;
            "work_cycle_completed"
        }
        "Complete" => "work_cycle_already_complete",
        _ => "work_cycle_ignored",
    };

    let mut related_alert_cycle_id = None;
    if !pr_url.is_empty() && !extract_string(&envelope.payload, &["pull_request.merge_commit_sha", "merge_commit_sha"]).unwrap_or("").is_empty() {
        let merge_sha = extract_string(&envelope.payload, &["pull_request.merge_commit_sha", "merge_commit_sha"])
            .unwrap_or("")
            .to_string();
        let filter = format!(
            "pr_url eq '{}' and (Status eq 'Fixed' or Status eq 'Merging')",
            escape_odata_string(&pr_url)
        );
        let related_cycles = state
            .odata
            .query_entities("AlertCycles", Some(&filter), Some("sequence_nr desc"), Some(10))
            .await
            .unwrap_or_default();
        for cycle in related_cycles {
            let Some(alert_cycle_id) = entity_id(&cycle).map(ToOwned::to_owned) else {
                continue;
            };
            related_alert_cycle_id.get_or_insert_with(|| alert_cycle_id.clone());
            if matches!(entity_status(&cycle).as_deref(), Some("Fixed")) {
                let _ = state
                    .odata
                    .dispatch_action(
                        "AlertCycles",
                        &alert_cycle_id,
                        "OpenPaw.Heal.BeginMerge",
                        json!({ "pr_url": pr_url }),
                    )
                    .await;
            }
            let _ = state
                .odata
                .dispatch_action(
                    "AlertCycles",
                    &alert_cycle_id,
                    "OpenPaw.Heal.MergeComplete",
                    json!({ "merge_sha": merge_sha.clone() }),
                )
                .await;
            let state_clone = state.clone();
            let alert_cycle_id_clone = alert_cycle_id.clone();
            let pr_url_clone = pr_url.clone();
            let merge_sha_clone = merge_sha.clone();
            tokio::spawn(async move {
                if let Err(error) = track_deployment_and_verify(
                    &state_clone,
                    &alert_cycle_id_clone,
                    &pr_url_clone,
                    &merge_sha_clone,
                )
                .await
                {
                    tracing::warn!(%error, alert_cycle_id = alert_cycle_id_clone, "failed to track deployment after GitHub merge webhook");
                }
            });
        }
    }

    Ok(WebhookIngestResponse {
        accepted: true,
        outcome: outcome.to_string(),
        message: format!("GitHub merged PR processed for WorkCycle {work_cycle_id}"),
        duplicate: false,
        monitor_id: None,
        alert_cycle_id: related_alert_cycle_id,
        sre_agent_id: None,
        work_cycle_id: Some(work_cycle_id),
    })
}

#[derive(Clone, Debug)]
struct GitHubPrRef {
    owner: String,
    repo: String,
    number: String,
}

async fn maybe_start_cicd_closure(state: &WebhookState, alert_cycle_id: &str) -> Result<()> {
    let alert_cycle = state
        .odata
        .get_entity("AlertCycles", alert_cycle_id)
        .await?;
    if !matches!(entity_status(&alert_cycle).as_deref(), Some("Fixed")) {
        return Ok(());
    }
    let Some(pr_url) = entity_field_str(&alert_cycle, &["pr_url", "PrUrl"]) else {
        return Ok(());
    };
    if pr_url.is_empty() {
        return Ok(());
    }
    start_cicd_closure(state, alert_cycle_id, pr_url).await
}

async fn start_cicd_closure(
    state: &WebhookState,
    alert_cycle_id: &str,
    pr_url: &str,
) -> Result<()> {
    let Some(github_token) = state
        .github_token
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    else {
        tracing::warn!(alert_cycle_id, pr_url, "skipping CI/CD closure because GITHUB_TOKEN is not configured");
        return Ok(());
    };
    let pr_ref = parse_github_pr_url(pr_url)?;
    state
        .odata
        .dispatch_action(
            "AlertCycles",
            alert_cycle_id,
            "OpenPaw.Heal.BeginMerge",
            json!({ "pr_url": pr_url }),
        )
        .await?;

    let github = reqwest::Client::new();
    if let Err(error) = wait_for_pr_ready_for_merge(&github, github_token, &pr_ref).await {
        state
            .odata
            .dispatch_action(
                "AlertCycles",
                alert_cycle_id,
                "OpenPaw.Heal.AlertPersists",
                json!({
                    "diagnosis": format!("PR {pr_url} never became merge-ready: {error:#}")
                }),
            )
            .await?;
        return Ok(());
    }

    let merge_sha = match squash_merge_pull_request(&github, github_token, &pr_ref).await {
        Ok(merge_sha) => merge_sha,
        Err(error) => {
            state
                .odata
                .dispatch_action(
                    "AlertCycles",
                    alert_cycle_id,
                    "OpenPaw.Heal.AlertPersists",
                    json!({
                        "diagnosis": format!("Failed to squash-merge PR {pr_url}: {error:#}")
                    }),
                )
                .await?;
            return Ok(());
        }
    };

    state
        .odata
        .dispatch_action(
            "AlertCycles",
            alert_cycle_id,
            "OpenPaw.Heal.MergeComplete",
            json!({ "merge_sha": merge_sha.clone() }),
        )
        .await?;

    track_deployment_and_verify(state, alert_cycle_id, pr_url, &merge_sha).await
}

async fn track_deployment_and_verify(
    state: &WebhookState,
    alert_cycle_id: &str,
    pr_url: &str,
    merge_sha: &str,
) -> Result<()> {
    let Some(github_token) = state
        .github_token
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    else {
        tracing::warn!(alert_cycle_id, merge_sha, "skipping deployment tracking because GITHUB_TOKEN is not configured");
        return Ok(());
    };
    let pr_ref = parse_github_pr_url(pr_url)?;
    let github = reqwest::Client::new();
    match wait_for_successful_deployment(&github, github_token, &pr_ref, merge_sha).await {
        Ok(deployment_url) => {
            state
                .odata
                .dispatch_action(
                    "AlertCycles",
                    alert_cycle_id,
                    "OpenPaw.Heal.DeployDetected",
                    json!({ "deployment_url": deployment_url }),
                )
                .await?;
            let alert_cycle = state
                .odata
                .get_entity("AlertCycles", alert_cycle_id)
                .await?;
            let monitor_id = entity_field_str(&alert_cycle, &["monitor_id", "MonitorId"])
                .unwrap_or("")
                .to_string();
            verify_alert_resolved_via_dd(state, alert_cycle_id, &monitor_id).await?;
        }
        Err(error) => {
            state
                .odata
                .dispatch_action(
                    "AlertCycles",
                    alert_cycle_id,
                    "OpenPaw.Heal.AlertPersists",
                    json!({
                        "diagnosis": format!("Deployment tracking failed for merge {merge_sha}: {error:#}")
                    }),
                )
                .await?;
        }
    }
    Ok(())
}

fn parse_github_pr_url(pr_url: &str) -> Result<GitHubPrRef> {
    let parts = pr_url
        .trim_end_matches('/')
        .split('/')
        .collect::<Vec<_>>();
    if parts.len() < 7 || parts[parts.len() - 2] != "pull" {
        bail!("unsupported GitHub PR URL: {pr_url}");
    }
    Ok(GitHubPrRef {
        owner: parts[parts.len() - 4].to_string(),
        repo: parts[parts.len() - 3].to_string(),
        number: parts[parts.len() - 1].to_string(),
    })
}

async fn wait_for_pr_ready_for_merge(
    github: &reqwest::Client,
    github_token: &str,
    pr_ref: &GitHubPrRef,
) -> Result<()> {
    for _ in 0..40 {
        let pr = github_get_json(
            github,
            github_token,
            &format!(
                "https://api.github.com/repos/{}/{}/pulls/{}",
                pr_ref.owner, pr_ref.repo, pr_ref.number
            ),
        )
        .await?;
        let mergeable = pr.get("mergeable").and_then(Value::as_bool);
        let mergeable_state = pr
            .get("mergeable_state")
            .and_then(Value::as_str)
            .unwrap_or("");
        let head_sha = pr
            .get("head")
            .and_then(|value| value.get("sha"))
            .and_then(Value::as_str)
            .unwrap_or("");
        if head_sha.is_empty() || mergeable.is_none() {
            sleep(Duration::from_secs(5)).await;
            continue;
        }

        let checks_ok = github_check_runs_green(github, github_token, pr_ref, head_sha).await?;
        let status_ok = github_combined_status_green(github, github_token, pr_ref, head_sha).await?;
        let mergeable_ok = mergeable == Some(true)
            && matches!(mergeable_state, "clean" | "unstable" | "has_hooks");
        if mergeable_ok && checks_ok && status_ok {
            return Ok(());
        }
        sleep(Duration::from_secs(15)).await;
    }
    bail!("timed out waiting for PR checks to pass")
}

async fn github_check_runs_green(
    github: &reqwest::Client,
    github_token: &str,
    pr_ref: &GitHubPrRef,
    head_sha: &str,
) -> Result<bool> {
    let body = github_get_json(
        github,
        github_token,
        &format!(
            "https://api.github.com/repos/{}/{}/commits/{}/check-runs?per_page=100",
            pr_ref.owner, pr_ref.repo, head_sha
        ),
    )
    .await?;
    let Some(check_runs) = body.get("check_runs").and_then(Value::as_array) else {
        return Ok(true);
    };
    if check_runs.is_empty() {
        return Ok(true);
    }
    Ok(check_runs.iter().all(|run| {
        let status = run.get("status").and_then(Value::as_str).unwrap_or("");
        let conclusion = run.get("conclusion").and_then(Value::as_str).unwrap_or("");
        status == "completed" && matches!(conclusion, "success" | "neutral" | "skipped")
    }))
}

async fn github_combined_status_green(
    github: &reqwest::Client,
    github_token: &str,
    pr_ref: &GitHubPrRef,
    head_sha: &str,
) -> Result<bool> {
    let body = github_get_json(
        github,
        github_token,
        &format!(
            "https://api.github.com/repos/{}/{}/commits/{}/status",
            pr_ref.owner, pr_ref.repo, head_sha
        ),
    )
    .await?;
    Ok(matches!(
        body.get("state").and_then(Value::as_str).unwrap_or("success"),
        "success" | ""
    ))
}

async fn squash_merge_pull_request(
    github: &reqwest::Client,
    github_token: &str,
    pr_ref: &GitHubPrRef,
) -> Result<String> {
    let response = github
        .put(format!(
            "https://api.github.com/repos/{}/{}/pulls/{}/merge",
            pr_ref.owner, pr_ref.repo, pr_ref.number
        ))
        .header("authorization", format!("Bearer {github_token}"))
        .header("accept", "application/vnd.github+json")
        .header("x-github-api-version", "2022-11-28")
        .header("user-agent", "openpaw")
        .json(&json!({ "merge_method": "squash" }))
        .send()
        .await
        .context("failed to call GitHub merge API")?;
    let status = response.status();
    let text = response.text().await.context("failed to read GitHub merge response")?;
    if !status.is_success() {
        bail!("GitHub merge API returned {status}: {text}");
    }
    let body: Value = serde_json::from_str(&text).context("failed to parse GitHub merge JSON")?;
    body.get("sha")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .context("GitHub merge response did not include sha")
}

async fn wait_for_successful_deployment(
    github: &reqwest::Client,
    github_token: &str,
    pr_ref: &GitHubPrRef,
    merge_sha: &str,
) -> Result<String> {
    let mut last_failure = None::<String>;
    for _ in 0..40 {
        let deployments = github_get_json(
            github,
            github_token,
            &format!(
                "https://api.github.com/repos/{}/{}/deployments?sha={}&per_page=20",
                pr_ref.owner, pr_ref.repo, merge_sha
            ),
        )
        .await?;
        let Some(items) = deployments.as_array() else {
            sleep(Duration::from_secs(15)).await;
            continue;
        };
        for deployment in items {
            let Some(statuses_url) = deployment
                .get("statuses_url")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
            else {
                continue;
            };
            let statuses = github_get_json(github, github_token, statuses_url).await?;
            let Some(status_items) = statuses.as_array() else {
                continue;
            };
            for status in status_items {
                match status.get("state").and_then(Value::as_str).unwrap_or("") {
                    "success" => {
                        return Ok(
                            extract_string(status, &["environment_url", "target_url", "log_url"])
                                .unwrap_or(merge_sha)
                                .to_string(),
                        );
                    }
                    "failure" | "error" | "inactive" => {
                        last_failure = Some(status.to_string());
                    }
                    _ => {}
                }
            }
        }
        sleep(Duration::from_secs(15)).await;
    }
    bail!(
        "timed out waiting for successful deployment status{}",
        last_failure
            .as_deref()
            .map(|value| format!("; last failure={value}"))
            .unwrap_or_default()
    )
}

async fn verify_alert_resolved_via_dd(
    state: &WebhookState,
    alert_cycle_id: &str,
    monitor_id: &str,
) -> Result<()> {
    if monitor_id.is_empty() {
        state
            .odata
            .dispatch_action(
                "AlertCycles",
                alert_cycle_id,
                "OpenPaw.Heal.AlertResolved",
                json!({
                    "diagnosis": "No monitor_id on AlertCycle after deployment; resolved without Datadog verification"
                }),
            )
            .await?;
        return Ok(());
    }

    let monitor = state.odata.get_entity("Monitors", monitor_id).await?;
    let dd_monitor_id = entity_field_str(&monitor, &["dd_monitor_id", "DdMonitorId"])
        .unwrap_or("")
        .to_string();
    if dd_monitor_id.is_empty() {
        state
            .odata
            .dispatch_action(
                "AlertCycles",
                alert_cycle_id,
                "OpenPaw.Heal.AlertResolved",
                json!({
                    "diagnosis": format!("Monitor {monitor_id} has no Datadog monitor id; resolved after deployment")
                }),
            )
            .await?;
        return Ok(());
    }

    let Some(dd_api_key) = state
        .dd_api_key
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    else {
        state
            .odata
            .dispatch_action(
                "AlertCycles",
                alert_cycle_id,
                "OpenPaw.Heal.AlertPersists",
                json!({
                    "diagnosis": format!("Datadog verification unavailable for monitor {dd_monitor_id}: DD_API_KEY is missing")
                }),
            )
            .await?;
        return Ok(());
    };
    let Some(dd_app_key) = state
        .dd_app_key
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    else {
        state
            .odata
            .dispatch_action(
                "AlertCycles",
                alert_cycle_id,
                "OpenPaw.Heal.AlertPersists",
                json!({
                    "diagnosis": format!("Datadog verification unavailable for monitor {dd_monitor_id}: DD_APP_KEY is missing")
                }),
            )
            .await?;
        return Ok(());
    };

    sleep(Duration::from_secs(120)).await;

    let datadog = reqwest::Client::new();
    for _ in 0..5 {
        let response = datadog
            .get(format!(
                "https://api.{}/api/v1/monitor/{}",
                state.dd_site, dd_monitor_id
            ))
            .header("DD-API-KEY", dd_api_key)
            .header("DD-APPLICATION-KEY", dd_app_key)
            .header("accept", "application/json")
            .send()
            .await;
        match response {
            Ok(response) if response.status().is_success() => {
                let body: Value = response
                    .json()
                    .await
                    .context("failed to parse Datadog monitor response")?;
                let overall_state = body
                    .get("overall_state")
                    .and_then(Value::as_str)
                    .unwrap_or("Unknown");
                let action = if matches!(overall_state, "OK" | "No Data") {
                    "OpenPaw.Heal.AlertResolved"
                } else {
                    "OpenPaw.Heal.AlertPersists"
                };
                let diagnosis = format!(
                    "Datadog monitor {dd_monitor_id} post-deploy state={overall_state}"
                );
                state
                    .odata
                    .dispatch_action(
                        "AlertCycles",
                        alert_cycle_id,
                        action,
                        json!({ "diagnosis": diagnosis }),
                    )
                    .await?;
                return Ok(());
            }
            Ok(_) | Err(_) => sleep(Duration::from_secs(30)).await,
        }
    }

    state
        .odata
        .dispatch_action(
            "AlertCycles",
            alert_cycle_id,
            "OpenPaw.Heal.AlertPersists",
            json!({
                "diagnosis": format!("Datadog verification failed repeatedly for monitor {dd_monitor_id} after deployment")
            }),
        )
        .await?;
    Ok(())
}

async fn github_get_json(
    github: &reqwest::Client,
    github_token: &str,
    url: &str,
) -> Result<Value> {
    let response = github
        .get(url)
        .header("authorization", format!("Bearer {github_token}"))
        .header("accept", "application/vnd.github+json")
        .header("x-github-api-version", "2022-11-28")
        .header("user-agent", "openpaw")
        .send()
        .await
        .with_context(|| format!("GET {url} failed"))?;
    let status = response.status();
    let text = response.text().await.context("failed reading GitHub response body")?;
    if !status.is_success() {
        bail!("GET {url} returned {status}: {text}");
    }
    if text.trim().is_empty() {
        return Ok(Value::Null);
    }
    serde_json::from_str(&text).context("failed to parse GitHub JSON response")
}

async fn resolve_or_create_monitor(
    state: &WebhookState,
    monitor_key: &str,
    payload: &Value,
) -> Result<Value> {
    let filter = format!("dd_monitor_id eq '{}'", escape_odata_string(monitor_key));
    if let Some(existing) = state
        .odata
        .query_entities("Monitors", Some(&filter), Some("sequence_nr desc"), Some(1))
        .await?
        .into_iter()
        .next()
    {
        let monitor_id = entity_id(&existing)
            .context("existing monitor missing entity id")?
            .to_string();
        let status = entity_status(&existing).unwrap_or_default();
        if status == "Created" || status == "Paused" {
            state
                .odata
                .dispatch_action(
                    "Monitors",
                    &monitor_id,
                    "OpenPaw.Heal.Configure",
                    json!({
                        "dd_query": extract_string(payload, &["dd_query", "query"]).unwrap_or(monitor_key),
                        "threshold": extract_string(payload, &["threshold"]).unwrap_or("1"),
                        "dd_monitor_id": monitor_key,
                    }),
                )
                .await?;
            state
                .odata
                .dispatch_action("Monitors", &monitor_id, "OpenPaw.Heal.Activate", json!({}))
                .await?;
            return state.odata.get_entity("Monitors", &monitor_id).await;
        }
        return Ok(existing);
    }

    let monitor = state.odata.create_entity("Monitors", json!({})).await?;
    let monitor_id = entity_id(&monitor)
        .context("monitor creation did not return an entity id")?
        .to_string();
    state
        .odata
        .dispatch_action(
            "Monitors",
            &monitor_id,
            "OpenPaw.Heal.Configure",
            json!({
                "dd_query": extract_string(payload, &["dd_query", "query"]).unwrap_or(monitor_key),
                "threshold": extract_string(payload, &["threshold"]).unwrap_or("1"),
                "dd_monitor_id": monitor_key,
            }),
        )
        .await?;
    state
        .odata
        .dispatch_action("Monitors", &monitor_id, "OpenPaw.Heal.Activate", json!({}))
        .await?;
    state.odata.get_entity("Monitors", &monitor_id).await
}

async fn find_duplicate_alert_cycle(
    state: &WebhookState,
    monitor_id: &str,
    canonical_payload: &str,
) -> Result<Option<Value>> {
    let filter = format!("monitor_id eq '{}'", escape_odata_string(monitor_id));
    let candidates = state
        .odata
        .query_entities(
            "AlertCycles",
            Some(&filter),
            Some("sequence_nr desc"),
            Some(25),
        )
        .await?;
    Ok(candidates.into_iter().find(|candidate| {
        entity_field_str(candidate, &["alert_payload", "AlertPayload"])
            .map(|value| value == canonical_payload)
            .unwrap_or(false)
    }))
}

async fn resolve_alert_context(state: &WebhookState, payload: &Value) -> Result<AlertContext> {
    let mut project_harness_id =
        extract_string(payload, &["project_harness_id", "projectHarnessId"]).map(ToOwned::to_owned);
    let mut repo_url = extract_string(
        payload,
        &[
            "repo_url",
            "repository.clone_url",
            "repository.html_url",
            "repository.url",
        ],
    )
    .map(ToOwned::to_owned);

    if let Some(harness_id) = project_harness_id.as_deref() {
        if let Ok(harness) = state.odata.get_entity("ProjectHarnesses", harness_id).await {
            if repo_url.is_none() {
                repo_url =
                    entity_field_str(&harness, &["repo_url", "RepoUrl"]).map(ToOwned::to_owned);
            }
        }
    } else if let Some(found_harness) = find_project_harness(state, repo_url.as_deref()).await? {
        if let Some(found_id) = entity_id(&found_harness) {
            project_harness_id = Some(found_id.to_string());
        }
        if repo_url.is_none() {
            repo_url =
                entity_field_str(&found_harness, &["repo_url", "RepoUrl"]).map(ToOwned::to_owned);
        }
    }

    let report_target = resolve_report_target(state, payload).await?;

    Ok(AlertContext {
        project_harness_id,
        repo_url,
        developer_sandbox_url: extract_string(payload, &["developer_sandbox_url", "sandbox_url"])
            .map(ToOwned::to_owned),
        developer_workdir: extract_string(payload, &["developer_workdir", "workdir"])
            .unwrap_or(DEFAULT_DEVELOPER_WORKDIR)
            .to_string(),
        severity: extract_string(payload, &["severity", "priority"]).map(ToOwned::to_owned),
        summary: extract_string(payload, &["summary", "title", "name", "event_title"])
            .map(ToOwned::to_owned),
        failure: extract_string(
            payload,
            &[
                "reproduction.failure",
                "failure",
                "error.message",
                "body",
                "message",
                "text",
            ],
        )
        .map(ToOwned::to_owned),
        report_target,
    })
}

async fn find_project_harness(
    state: &WebhookState,
    repo_url: Option<&str>,
) -> Result<Option<Value>> {
    if let Some(repo_url) = repo_url.filter(|value| !value.is_empty()) {
        let filter = format!("repo_url eq '{}'", escape_odata_string(repo_url));
        if let Some(harness) = state
            .odata
            .query_entities(
                "ProjectHarnesses",
                Some(&filter),
                Some("sequence_nr desc"),
                Some(1),
            )
            .await?
            .into_iter()
            .next()
        {
            return Ok(Some(harness));
        }
    }

    Ok(state
        .odata
        .query_entities(
            "ProjectHarnesses",
            Some("Status eq 'Active'"),
            Some("sequence_nr desc"),
            Some(1),
        )
        .await?
        .into_iter()
        .next())
}

async fn spawn_sre_agent(
    state: &WebhookState,
    monitor_id: &str,
    alert_cycle_id: &str,
    canonical_payload: &str,
    context: &AlertContext,
) -> Result<Option<String>> {
    let Some(project_harness_id) = context.project_harness_id.as_deref() else {
        return Ok(None);
    };

    let active_sre = state
        .odata
        .query_entities(
            "Souls",
            Some("Name eq 'SRE' and Status eq 'Active'"),
            Some("sequence_nr desc"),
            Some(1),
        )
        .await?;
    if active_sre.is_empty() {
        bail!("SRE soul is not active yet");
    }

    let agent = state.odata.create_entity("Agents", json!({})).await?;
    let agent_id = entity_id(&agent)
        .context("agent creation did not return an entity id")?
        .to_string();

    let sre_message = build_sre_message(
        project_harness_id,
        monitor_id,
        alert_cycle_id,
        canonical_payload,
        context,
    );
    state
        .odata
        .dispatch_action(
            "Agents",
            &agent_id,
            "OpenPaw.Configure",
            json!({
                "model": DEFAULT_AGENT_MODEL,
                "provider": "anthropic",
                "max_turns": "80",
                "tools_enabled": "temper_get,temper_list,temper_action,temper_create,spawn_agent,read_entity",
                "workdir": DEFAULT_SRE_WORKDIR,
                "soul_id": "SRE",
                "temper_api_url": state.odata.base_url.clone(),
                "user_message": sre_message,
            }),
        )
        .await?;

    Ok(Some(agent_id))
}

fn spawn_sre_completion_watcher(
    state: WebhookState,
    alert_cycle_id: String,
    sre_agent_id: String,
    project_harness_id: Option<String>,
    repo_url: Option<String>,
    report_target: Option<ReportTarget>,
) {
    tokio::spawn(async move {
        let run = async {
            let sre_terminal = state
                .odata
                .wait_for_agent_terminal(&sre_agent_id, Duration::from_secs(20 * 60))
                .await?;
            let sre_status = entity_status(&sre_terminal);
            converge_alert_cycle_after_sre_terminal(
                &state,
                &alert_cycle_id,
                &sre_agent_id,
                &sre_terminal,
            )
            .await?;
            if matches!(sre_status.as_deref(), Some("Completed")) {
                maybe_start_cicd_closure(&state, &alert_cycle_id).await?;
            }
            let Some(report_target) = report_target.as_ref() else {
                return Ok::<(), anyhow::Error>(());
            };
            let alert_cycle = wait_for_alert_cycle_terminal(&state, &alert_cycle_id).await?;
            let work_cycle = if let Some(project_harness_id) = project_harness_id.as_deref() {
                latest_work_cycle(&state, project_harness_id).await?
            } else {
                None
            };
            let issue = find_latest_issue_for_alert(&state, &alert_cycle_id).await?;
            let content = build_proactive_summary(
                &alert_cycle_id,
                &sre_agent_id,
                repo_url.as_deref(),
                &alert_cycle,
                work_cycle.as_ref(),
                issue.as_ref(),
            );
            state
                .odata
                .dispatch_action(
                    "Channels",
                    &report_target.channel_entity_id,
                    "Paw.Channel.SendReply",
                    json!({
                        "thread_id": report_target.thread_id,
                        "content": content,
                        "agent_entity_id": sre_agent_id,
                    }),
                )
                .await?;
            Ok::<(), anyhow::Error>(())
        };

        if let Err(error) = run.await {
            tracing::warn!(%error, alert_cycle_id, sre_agent_id, "failed to converge sre-driven alert cycle");
        }
    });
}

async fn converge_alert_cycle_after_sre_terminal(
    state: &WebhookState,
    alert_cycle_id: &str,
    sre_agent_id: &str,
    sre_terminal: &Value,
) -> Result<()> {
    let sre_status = entity_status(sre_terminal);
    if !matches!(sre_status.as_deref(), Some("Failed" | "Cancelled")) {
        return Ok(());
    }

    let alert_cycle = state
        .odata
        .get_entity("AlertCycles", alert_cycle_id)
        .await?;
    if !matches!(entity_status(&alert_cycle).as_deref(), Some("Triaging")) {
        return Ok(());
    }

    let diagnosis = build_sre_failure_diagnosis(sre_terminal, sre_agent_id);
    state
        .odata
        .dispatch_action(
            "AlertCycles",
            alert_cycle_id,
            "OpenPaw.Heal.Escalate",
            json!({ "diagnosis": diagnosis }),
        )
        .await?;
    Ok(())
}

async fn wait_for_alert_cycle_terminal(
    state: &WebhookState,
    alert_cycle_id: &str,
) -> Result<Value> {
    for _ in 0..120 {
        let alert_cycle = state
            .odata
            .get_entity("AlertCycles", alert_cycle_id)
            .await?;
        match entity_status(&alert_cycle).as_deref() {
            Some("Resolved" | "Tuned" | "Failed") => return Ok(alert_cycle),
            Some("Fixed")
                if entity_field_str(&alert_cycle, &["pr_url", "PrUrl"])
                    .unwrap_or("")
                    .is_empty() =>
            {
                return Ok(alert_cycle)
            }
            _ => sleep(Duration::from_secs(5)).await,
        }
    }
    state.odata.get_entity("AlertCycles", alert_cycle_id).await
}

fn build_sre_failure_diagnosis(sre_terminal: &Value, sre_agent_id: &str) -> String {
    let sre_status = entity_status(sre_terminal).unwrap_or_else(|| "Unknown".to_string());
    let error = entity_field_str(
        sre_terminal,
        &["error_message", "ErrorMessage", "error", "Error"],
    )
    .unwrap_or("SRE agent terminated before it could classify or remediate the alert.");
    format!("SRE agent {sre_agent_id} ended in {sre_status}: {error}")
}

async fn latest_work_cycle(
    state: &WebhookState,
    project_harness_id: &str,
) -> Result<Option<Value>> {
    let filter = format!(
        "project_harness_id eq '{}'",
        escape_odata_string(project_harness_id)
    );
    Ok(state
        .odata
        .query_entities(
            "WorkCycles",
            Some(&filter),
            Some("sequence_nr desc"),
            Some(1),
        )
        .await?
        .into_iter()
        .next())
}

async fn find_latest_issue_for_alert(
    state: &WebhookState,
    alert_cycle_id: &str,
) -> Result<Option<Value>> {
    let issues = state
        .odata
        .query_entities("Issues", None, Some("sequence_nr desc"), Some(25))
        .await?;
    Ok(issues.into_iter().find(|issue| {
        entity_field_str(issue, &["Description", "description"])
            .map(|description| description.contains(alert_cycle_id))
            .unwrap_or(false)
    }))
}

async fn resolve_report_target(
    state: &WebhookState,
    payload: &Value,
) -> Result<Option<ReportTarget>> {
    if let (Some(channel_entity_id), Some(thread_id)) = (
        extract_string(payload, &["reply_channel_entity_id", "channel_entity_id"]),
        extract_string(payload, &["reply_thread_id", "thread_id"]),
    ) {
        return Ok(Some(ReportTarget {
            channel_entity_id: channel_entity_id.to_string(),
            thread_id: thread_id.to_string(),
        }));
    }

    if let (Some(external_channel_id), Some(thread_id)) = (
        extract_string(payload, &["reply_channel_id", "channel_id"]),
        extract_string(payload, &["reply_thread_id", "thread_id"]),
    ) {
        if let Some(channel_entity_id) =
            resolve_channel_entity_id(state, external_channel_id).await?
        {
            return Ok(Some(ReportTarget {
                channel_entity_id,
                thread_id: thread_id.to_string(),
            }));
        }
    }

    let session = state
        .odata
        .query_entities(
            "ChannelSessions",
            Some("Status eq 'Active'"),
            Some("sequence_nr desc"),
            Some(1),
        )
        .await?
        .into_iter()
        .next();
    let Some(session) = session else {
        return Ok(None);
    };
    let Some(thread_id) = entity_field_str(&session, &["thread_id", "ThreadId"]) else {
        return Ok(None);
    };
    let Some(external_channel_id) = entity_field_str(&session, &["channel_id", "ChannelId"]) else {
        return Ok(None);
    };
    let Some(channel_entity_id) = resolve_channel_entity_id(state, external_channel_id).await?
    else {
        return Ok(None);
    };
    Ok(Some(ReportTarget {
        channel_entity_id,
        thread_id: thread_id.to_string(),
    }))
}

async fn resolve_channel_entity_id(
    state: &WebhookState,
    external_channel_id: &str,
) -> Result<Option<String>> {
    let filter = format!(
        "channel_id eq '{}' and Status eq 'Connected'",
        escape_odata_string(external_channel_id)
    );
    Ok(state
        .odata
        .query_entities("Channels", Some(&filter), Some("sequence_nr desc"), Some(1))
        .await?
        .into_iter()
        .next()
        .and_then(|channel| entity_id(&channel).map(ToOwned::to_owned)))
}

fn build_sre_message(
    project_harness_id: &str,
    monitor_id: &str,
    alert_cycle_placeholder: &str,
    canonical_payload: &str,
    context: &AlertContext,
) -> String {
    let repo_url = context.repo_url.as_deref().unwrap_or("unknown-repository");
    let developer_sandbox_instructions = if let Some(sandbox_url) = context
        .developer_sandbox_url
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        format!("- pass `sandbox_url = {sandbox_url}` through to the Developer child agent")
    } else {
        "- do not invent a sandbox URL; let platform provisioning use the configured default sandbox".to_string()
    };
    let priority_hint = match context.severity.as_deref() {
        Some("critical" | "sev0" | "sev1") => "1",
        Some("high" | "error") => "2",
        Some("medium" | "warn" | "warning") => "3",
        _ => "2",
    };
    format!(
        "You are handling a real webhook-driven self-heal remediation.\n\n\
Workflow entity IDs:\n\
- ProjectHarness: {project_harness_id}\n\
- Monitor: {monitor_id}\n\
- AlertCycle: {alert_cycle_placeholder}\n\
- Repository: {repo_url}\n\n\
Alert context:\n\
- Severity: {severity}\n\
- Summary: {summary}\n\
- Failure signal: {failure}\n\
- Raw alert payload JSON: {canonical_payload}\n\n\
Treat this as a real issue unless the payload clearly proves it is monitor noise.\n\n\
Required workflow:\n\
1. Read the ProjectHarness, Monitor, and AlertCycle first.\n\
2. Record a concrete diagnosis in your own reasoning based on the failing symptom.\n\
3. For a confirmed real issue, create or reuse exactly one PM Issue before spawning a Developer:\n\
   - look for an existing non-final Issue only if it already covers this exact Monitor ID\n\
   - do not reuse an Issue that references a different Monitor ID even if the diagnosis text looks similar\n\
   - if none exists, create one new Issue entity\n\
   - set a description that includes the Monitor ID, AlertCycle ID, and later the WorkCycle ID once it exists\n\
   - set priority level {priority_hint}\n\
   - move the Issue into Triage so it is visible as active work\n\
4. Create or reuse exactly one WorkCycle tied to the ProjectHarness for the remediation.\n\
5. Spawn exactly one Developer child agent with:\n\
   - `soul_id = Developer`\n\
   - tools including `read,write,edit,bash,temper_get,temper_list,temper_action,read_entity`\n\
   - `workdir = {developer_workdir}`\n\
   - `max_turns = 80`\n\
   - `background = false`\n\
   - {developer_sandbox_instructions}\n\
   - do not spawn a replacement developer unless the first child reaches a terminal failed state\n\
6. Give the Developer a precise remediation task:\n\
- clone {repo_url}\n\
- reproduce the issue from the alert payload with a bounded command such as `timeout 120 npm ci` when the failing command is `npm ci`\n\
- after the first failed reproduction, move directly to the smallest safe fix; do not spend turns on lockfile greps, git history, or broad package surveys when the missing packages are already listed in the alert\n\
- if the issue looks like dependency or lockfile drift and a full install hangs, times out, or is killed, do not loop on it; instead remove `node_modules` and use a bounded lockfile refresh such as `timeout 120 npm install --package-lock-only --ignore-scripts --no-fund --no-audit`\n\
- when the alert already identifies the missing packages, treat that as the working diagnosis and repair the lockfile directly on the next step; prefer the bounded lockfile refresh command above or a bounded install of the named packages, and do not spend turns on `git log`, lockfile grep, broad package surveys, or history archaeology unless that direct repair attempt fails\n\
   - run at least one concrete validation command\n\
   - commit, push, and open a PR if code changed\n\
   - use the GitHub REST API via `curl` if `gh` is unavailable\n\
7. After the child finishes, read back the updated WorkCycle and PM Issue.\n\
8. Close the loop:\n\
   - success: `WorkCycle.BeginTesting`, `WorkCycle.PassTests`, `WorkCycle.Approve`, `AlertCycle.HealComplete`\n\
   - monitor noise: `Monitor.Tune` then `AlertCycle.TuneComplete`\n\
   - failed remediation: `WorkCycle.Fail` then `AlertCycle.Escalate`\n\
9. If you created or updated a PM Issue, make sure its description reflects the final WorkCycle ID, AlertCycle ID, diagnosis, and PR URL.\n\n\
Your final response must include these exact keys on separate lines:\n\
ALERT_CYCLE_STATUS=<status>\n\
WORK_CYCLE_STATUS=<status or empty>\n\
PR_URL=<url or empty>\n\
DEVELOPER_AGENT_ID=<id or empty>\n\
ISSUE_ID=<id or empty>\n",
        severity = context.severity.as_deref().unwrap_or("unknown"),
        summary = context.summary.as_deref().unwrap_or("n/a"),
        failure = context.failure.as_deref().unwrap_or("n/a"),
        developer_workdir = context.developer_workdir,
    )
}

fn build_proactive_summary(
    alert_cycle_id: &str,
    sre_agent_id: &str,
    repo_url: Option<&str>,
    alert_cycle: &Value,
    work_cycle: Option<&Value>,
    issue: Option<&Value>,
) -> String {
    let alert_status = entity_status(alert_cycle).unwrap_or_else(|| "Unknown".to_string());
    let diagnosis = entity_field_str(alert_cycle, &["diagnosis", "Diagnosis"])
        .unwrap_or("No diagnosis recorded yet.");
    let pr_url = entity_field_str(alert_cycle, &["pr_url", "PrUrl"])
        .or_else(|| work_cycle.and_then(|value| entity_field_str(value, &["pr_url", "PrUrl"])))
        .unwrap_or("");
    let work_cycle_line = work_cycle
        .and_then(|value| entity_id(value))
        .map(|id| {
            format!(
                "WorkCycle: {id} ({})",
                entity_status(work_cycle.unwrap()).unwrap_or_else(|| "Unknown".to_string())
            )
        })
        .unwrap_or_else(|| "WorkCycle: n/a".to_string());
    let issue_line = issue
        .and_then(entity_id)
        .map(|id| format!("Issue: {id}"))
        .unwrap_or_else(|| "Issue: n/a".to_string());

    let mut lines = vec![
        "Open Paw self-heal update".to_string(),
        format!("AlertCycle: {alert_cycle_id} ({alert_status})"),
        format!("SRE: {sre_agent_id}"),
        work_cycle_line,
        issue_line,
    ];
    if let Some(repo_url) = repo_url.filter(|value| !value.is_empty()) {
        lines.push(format!("Repo: {repo_url}"));
    }
    lines.push(format!("Diagnosis: {diagnosis}"));
    if !pr_url.is_empty() {
        lines.push(format!("PR: {pr_url}"));
    }
    lines.join("\n")
}

fn verify_signature(secret: &str, headers: &HeaderMap, body: &[u8]) -> Result<()> {
    let provided = headers
        .get("x-webhook-signature-256")
        .or_else(|| headers.get("x-webhook-signature"))
        .context("missing webhook signature header")?
        .to_str()
        .context("signature header was not valid UTF-8")?;
    let provided = provided.strip_prefix("sha256=").unwrap_or(provided);
    let provided = hex::decode(provided).context("signature header was not valid hex")?;

    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).context("invalid webhook secret for HMAC")?;
    mac.update(body);
    mac.verify_slice(&provided)
        .map_err(|_| anyhow!("webhook signature mismatch"))
}

fn map_handler_error(error: anyhow::Error) -> ApiError {
    let message = format!("{error:#}");
    if message.contains("no WorkCycle matched") {
        ApiError::not_found(message)
    } else {
        ApiError::internal(message)
    }
}

fn normalize_webhook_envelope(headers: &HeaderMap, body: &[u8]) -> Result<WebhookPayload, ApiError> {
    let raw: Value = serde_json::from_slice(body)
        .map_err(|error| ApiError::bad_request(format!("invalid webhook JSON: {error}")))?;

    if raw.get("source").is_some() && raw.get("event_type").is_some() && raw.get("payload").is_some()
    {
        return serde_json::from_value(raw)
            .map_err(|error| ApiError::bad_request(format!("invalid webhook envelope: {error}")));
    }

    if is_datadog_payload(&raw) {
        return Ok(WebhookPayload {
            source: "datadog".to_string(),
            event_type: if is_datadog_recovered_value(&raw) {
                "alert_recovered".to_string()
            } else {
                "alert_fired".to_string()
            },
            payload: raw,
        });
    }

    if headers
        .get("x-github-event")
        .and_then(|value| value.to_str().ok())
        == Some("pull_request")
        && raw
            .get("action")
            .and_then(Value::as_str)
            .is_some_and(|value| value == "closed")
        && raw
            .get("pull_request")
            .and_then(|value| value.get("merged"))
            .and_then(Value::as_bool)
            .unwrap_or(false)
    {
        return Ok(WebhookPayload {
            source: "github".to_string(),
            event_type: "pull_request.merged".to_string(),
            payload: raw,
        });
    }

    Err(ApiError::bad_request(
        "invalid webhook payload: expected an Open Paw envelope or a supported Datadog/GitHub webhook body",
    ))
}

fn is_datadog_payload(value: &Value) -> bool {
    value.get("org").and_then(|org| org.get("id")).is_some()
        || value.get("alert_transition").is_some()
        || value.get("alert_type").is_some()
}

fn is_datadog_recovered_value(value: &Value) -> bool {
    extract_string(value, &["alert_transition"])
        .map(|transition| transition.eq_ignore_ascii_case("Recovered"))
        .unwrap_or(false)
}

fn is_datadog_recovery_payload(envelope: &WebhookPayload) -> bool {
    envelope.source.eq_ignore_ascii_case("datadog")
        && (envelope.event_type.eq_ignore_ascii_case("alert_recovered")
            || is_datadog_recovered_value(&envelope.payload))
}

async fn resolve_recovered_alert_cycle(
    state: &WebhookState,
    monitor_id: &str,
    payload: &Value,
) -> Result<String> {
    let filter = format!("monitor_id eq '{}'", escape_odata_string(monitor_id));
    let candidates = state
        .odata
        .query_entities(
            "AlertCycles",
            Some(&filter),
            Some("sequence_nr desc"),
            Some(25),
        )
        .await?;

    if candidates.is_empty() {
        return Ok("recovery_without_active_cycle".to_string());
    }
    let diagnosis = format!(
        "Datadog monitor recovered: {}",
        extract_string(payload, &["text", "title", "event_title"])
            .unwrap_or("monitor returned to normal")
    );

    let recoverable_cycles = candidates
        .iter()
        .filter(|candidate| {
            matches!(
                entity_status(candidate).as_deref(),
                Some("Fixed" | "Deploying" | "Verifying")
            )
        })
        .filter_map(|candidate| entity_id(candidate).map(ToOwned::to_owned))
        .collect::<Vec<_>>();
    if recoverable_cycles.is_empty() {
        return Ok("recovery_waiting_for_verification".to_string());
    }

    for alert_cycle_id in &recoverable_cycles {
        state
            .odata
            .dispatch_action(
                "AlertCycles",
                alert_cycle_id,
                "OpenPaw.Heal.AlertResolved",
                json!({ "diagnosis": diagnosis }),
            )
            .await?;
    }
    Ok(format!("alert_resolved:{}" , recoverable_cycles.len()))
}

fn entity_id(value: &Value) -> Option<&str> {
    value
        .get("entity_id")
        .and_then(Value::as_str)
        .or_else(|| value.get("Id").and_then(Value::as_str))
        .or_else(|| {
            value
                .get("fields")
                .and_then(|fields| fields.get("Id"))
                .and_then(Value::as_str)
        })
}

fn entity_status(value: &Value) -> Option<String> {
    entity_field_str(value, &["status", "Status"]).map(ToOwned::to_owned)
}

fn entity_field_str<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
        .or_else(|| {
            value.get("fields").and_then(|fields| {
                keys.iter()
                    .find_map(|key| fields.get(*key).and_then(Value::as_str))
            })
        })
}

fn extract_monitor_key(payload: &Value) -> Option<String> {
    extract_string(
        payload,
        &[
            "id",
            "monitor_id",
            "dd_monitor_id",
            "monitor.id",
            "monitor.slug",
            "monitor.name",
        ],
    )
    .map(ToOwned::to_owned)
}

fn extract_pr_url(payload: &Value) -> Option<String> {
    extract_string(payload, &["pr_url", "html_url", "pull_request.html_url"]).map(ToOwned::to_owned)
}

fn extract_string<'a>(value: &'a Value, paths: &[&str]) -> Option<&'a str> {
    paths.iter().find_map(|path| {
        let mut current = value;
        for segment in path.split('.') {
            current = current.get(segment)?;
        }
        current.as_str()
    })
}

fn escape_odata_string(value: &str) -> String {
    value.replace('\'', "''")
}

fn canonical_json_string(value: &Value) -> String {
    serde_json::to_string(&canonicalize_json(value)).unwrap_or_else(|_| value.to_string())
}

fn canonicalize_json(value: &Value) -> Value {
    match value {
        Value::Array(values) => Value::Array(values.iter().map(canonicalize_json).collect()),
        Value::Object(map) => {
            let mut pairs: Vec<_> = map.iter().collect();
            pairs.sort_by(|(left, _), (right, _)| left.cmp(right));
            let mut normalized = serde_json::Map::new();
            for (key, value) in pairs {
                normalized.insert(key.clone(), canonicalize_json(value));
            }
            Value::Object(normalized)
        }
        _ => value.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_json_sorts_nested_object_keys() {
        let payload = json!({
            "z": 1,
            "nested": {
                "b": true,
                "a": false
            },
            "a": 2
        });

        assert_eq!(
            canonical_json_string(&payload),
            r#"{"a":2,"nested":{"a":false,"b":true},"z":1}"#
        );
    }

    #[test]
    fn extract_string_supports_nested_paths() {
        let payload = json!({
            "repository": {
                "clone_url": "https://example.com/repo.git"
            }
        });

        assert_eq!(
            extract_string(&payload, &["repository.clone_url", "repo_url"]),
            Some("https://example.com/repo.git")
        );
    }
}
