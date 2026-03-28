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
const DEFAULT_SCOUT_WORKDIR: &str = "/tmp/openpaw-scout-webhook";
const DEFAULT_DEVELOPER_WORKDIR: &str = "/tmp/openpaw-self-heal";

#[derive(Clone, Debug)]
pub struct WebhookState {
    odata: ODataClient,
    webhook_secret: Option<String>,
}

impl WebhookState {
    pub fn new(
        base_url: String,
        tenant: String,
        api_key: Option<String>,
        webhook_secret: Option<String>,
    ) -> Self {
        Self {
            odata: ODataClient::new(reqwest::Client::new(), base_url, tenant, api_key),
            webhook_secret,
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
    scout_agent_id: Option<String>,
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

    let envelope: WebhookPayload = serde_json::from_slice(&body)
        .map_err(|error| ApiError::bad_request(format!("invalid webhook JSON: {error}")))?;

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

    if let Some(existing) =
        find_duplicate_alert_cycle(state, &monitor_id, &canonical_payload).await?
    {
        let alert_cycle_id = entity_id(&existing)
            .context("duplicate alert cycle missing entity id")?
            .to_string();
        let scout_agent_id =
            entity_field_str(&existing, &["scout_agent_id", "ScoutAgentId"]).map(ToOwned::to_owned);
        return Ok(WebhookIngestResponse {
            accepted: true,
            outcome: "duplicate_alert".to_string(),
            message: "Duplicate alert payload ignored".to_string(),
            duplicate: true,
            monitor_id: Some(monitor_id),
            alert_cycle_id: Some(alert_cycle_id),
            scout_agent_id,
            work_cycle_id: None,
        });
    }

    let alert_cycle = state.odata.create_entity("AlertCycles", json!({})).await?;
    let alert_cycle_id = entity_id(&alert_cycle)
        .context("alert cycle creation did not return an entity id")?
        .to_string();
    let alert_context = resolve_alert_context(state, &envelope.payload).await?;
    let scout_agent_id = match spawn_scout_agent(
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
            tracing::warn!(%error, monitor_id, alert_cycle_id, "webhook alert will remain open without auto-spawned scout");
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
                "scout_agent_id": scout_agent_id.clone().unwrap_or_default(),
            }),
        )
        .await?;

    if let Some(scout_agent_id) = scout_agent_id.clone() {
        state
            .odata
            .dispatch_action("Agents", &scout_agent_id, "OpenPaw.Provision", json!({}))
            .await?;
        spawn_scout_completion_watcher(
            state.clone(),
            alert_cycle_id.clone(),
            scout_agent_id.clone(),
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
        scout_agent_id,
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

    Ok(WebhookIngestResponse {
        accepted: true,
        outcome: outcome.to_string(),
        message: format!("GitHub merged PR processed for WorkCycle {work_cycle_id}"),
        duplicate: false,
        monitor_id: None,
        alert_cycle_id: None,
        scout_agent_id: None,
        work_cycle_id: Some(work_cycle_id),
    })
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
                        "logfire_query": extract_string(payload, &["logfire_query", "query"]).unwrap_or(monitor_key),
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
                "logfire_query": extract_string(payload, &["logfire_query", "query"]).unwrap_or(monitor_key),
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
        severity: extract_string(payload, &["severity"]).map(ToOwned::to_owned),
        summary: extract_string(payload, &["summary", "title"]).map(ToOwned::to_owned),
        failure: extract_string(
            payload,
            &[
                "reproduction.failure",
                "failure",
                "error.message",
                "message",
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

async fn spawn_scout_agent(
    state: &WebhookState,
    monitor_id: &str,
    alert_cycle_id: &str,
    canonical_payload: &str,
    context: &AlertContext,
) -> Result<Option<String>> {
    let Some(project_harness_id) = context.project_harness_id.as_deref() else {
        return Ok(None);
    };

    let active_scout = state
        .odata
        .query_entities(
            "Souls",
            Some("Name eq 'Scout' and Status eq 'Active'"),
            Some("sequence_nr desc"),
            Some(1),
        )
        .await?;
    if active_scout.is_empty() {
        bail!("Scout soul is not active yet");
    }

    let agent = state.odata.create_entity("Agents", json!({})).await?;
    let agent_id = entity_id(&agent)
        .context("agent creation did not return an entity id")?
        .to_string();

    let scout_message = build_scout_message(
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
                "workdir": DEFAULT_SCOUT_WORKDIR,
                "soul_id": "Scout",
                "temper_api_url": state.odata.base_url.clone(),
                "user_message": scout_message,
            }),
        )
        .await?;

    Ok(Some(agent_id))
}

fn spawn_scout_completion_watcher(
    state: WebhookState,
    alert_cycle_id: String,
    scout_agent_id: String,
    project_harness_id: Option<String>,
    repo_url: Option<String>,
    report_target: Option<ReportTarget>,
) {
    tokio::spawn(async move {
        let run = async {
            let scout_terminal = state
                .odata
                .wait_for_agent_terminal(&scout_agent_id, Duration::from_secs(20 * 60))
                .await?;
            converge_alert_cycle_after_scout_terminal(
                &state,
                &alert_cycle_id,
                &scout_agent_id,
                &scout_terminal,
            )
            .await?;
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
                &scout_agent_id,
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
                        "agent_entity_id": scout_agent_id,
                    }),
                )
                .await?;
            Ok::<(), anyhow::Error>(())
        };

        if let Err(error) = run.await {
            tracing::warn!(%error, alert_cycle_id, scout_agent_id, "failed to converge scout-driven alert cycle");
        }
    });
}

async fn converge_alert_cycle_after_scout_terminal(
    state: &WebhookState,
    alert_cycle_id: &str,
    scout_agent_id: &str,
    scout_terminal: &Value,
) -> Result<()> {
    let scout_status = entity_status(scout_terminal);
    if !matches!(scout_status.as_deref(), Some("Failed" | "Cancelled")) {
        return Ok(());
    }

    let alert_cycle = state
        .odata
        .get_entity("AlertCycles", alert_cycle_id)
        .await?;
    if !matches!(entity_status(&alert_cycle).as_deref(), Some("Triaging")) {
        return Ok(());
    }

    let diagnosis = build_scout_failure_diagnosis(scout_terminal, scout_agent_id);
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
            Some("Fixed" | "Tuned" | "Failed") => return Ok(alert_cycle),
            _ => sleep(Duration::from_secs(5)).await,
        }
    }
    state.odata.get_entity("AlertCycles", alert_cycle_id).await
}

fn build_scout_failure_diagnosis(scout_terminal: &Value, scout_agent_id: &str) -> String {
    let scout_status = entity_status(scout_terminal).unwrap_or_else(|| "Unknown".to_string());
    let error = entity_field_str(
        scout_terminal,
        &["error_message", "ErrorMessage", "error", "Error"],
    )
    .unwrap_or("Scout agent terminated before it could classify or remediate the alert.");
    format!("Scout agent {scout_agent_id} ended in {scout_status}: {error}")
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

fn build_scout_message(
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
        "- do not invent a sandbox URL; let platform provisioning use the configured default (local or E2B)".to_string()
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
- apply the smallest safe fix\n\
- if the issue looks like dependency or lockfile drift and a full install hangs, times out, or is killed, do not loop on it; instead remove `node_modules` and use a bounded lockfile refresh such as `timeout 120 npm install --package-lock-only --ignore-scripts --no-fund --no-audit`\n\
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
    scout_agent_id: &str,
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
        format!("Scout: {scout_agent_id}"),
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
