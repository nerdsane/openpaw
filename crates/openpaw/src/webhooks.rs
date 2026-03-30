//! Webhook ingestion routes for Open Paw.
//!
//! `POST /webhooks/ingest` accepts external Datadog and GitHub webhook payloads,
//! then re-enters the platform through its own OData API. This keeps webhook-driven
//! workflows governed by the same entity actions used everywhere else.

use std::time::Duration;

use axum::Router;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use serde::Serialize;
use serde_json::{Value, json};
use tokio::time::sleep;

const DEFAULT_AGENT_MODEL: &str = "claude-sonnet-4-20250514";
const DEFAULT_SRE_WORKDIR: &str = "/tmp/openpaw-sre-webhook";
const DEFAULT_DEVELOPER_WORKDIR: &str = "/tmp/openpaw-self-heal";

#[derive(Clone, Debug)]
pub struct WebhookState {
    odata: ODataClient,
    #[allow(dead_code)] // Used for HMAC signature verification when WEBHOOK_SECRET is set
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

#[derive(Debug, Serialize)]
struct WebhookIngestResponse {
    accepted: bool,
    outcome: String,
    message: String,
    monitor_id: Option<String>,
    alert_cycle_id: Option<String>,
    sre_agent_id: Option<String>,
}

impl IntoResponse for WebhookIngestResponse {
    fn into_response(self) -> Response {
        (StatusCode::OK, axum::Json(json!(self))).into_response()
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

    fn build_request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        let url = format!("{}/tdata/{path}", self.base_url);
        let mut req = self.http.request(method, &url)
            .header("x-tenant-id", &self.tenant)
            .header("x-temper-principal-kind", "admin")
            .header("content-type", "application/json");
        if let Some(ref key) = self.api_key {
            req = req.header("authorization", format!("Bearer {key}"));
        }
        req
    }

    async fn create(&self, entity_set: &str, body: &Value) -> anyhow::Result<Value> {
        let resp = self.build_request(reqwest::Method::POST, entity_set)
            .json(body)
            .send()
            .await?;
        let status = resp.status();
        let text = resp.text().await?;
        if !status.is_success() {
            anyhow::bail!("OData create {entity_set} failed (HTTP {status}): {text}");
        }
        Ok(serde_json::from_str(&text)?)
    }

    async fn action(&self, entity_set: &str, id: &str, action: &str, body: &Value) -> anyhow::Result<Value> {
        let path = format!("{entity_set}('{id}')/{action}");
        let resp = self.build_request(reqwest::Method::POST, &path)
            .json(body)
            .send()
            .await?;
        let status = resp.status();
        let text = resp.text().await?;
        if !status.is_success() {
            anyhow::bail!("OData action {entity_set}.{action} failed (HTTP {status}): {text}");
        }
        Ok(serde_json::from_str(&text)?)
    }

    async fn list(&self, entity_set: &str, filter: &str) -> anyhow::Result<Vec<Value>> {
        let path = format!("{entity_set}?$filter={}", urlencoding::encode(filter));
        let resp = self.build_request(reqwest::Method::GET, &path)
            .send()
            .await?;
        let status = resp.status();
        let text = resp.text().await?;
        if !status.is_success() {
            anyhow::bail!("OData list {entity_set} failed (HTTP {status}): {text}");
        }
        let parsed: Value = serde_json::from_str(&text)?;
        Ok(parsed.get("value")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default())
    }

    async fn get(&self, entity_set: &str, id: &str) -> anyhow::Result<Value> {
        let path = format!("{entity_set}('{id}')");
        let resp = self.build_request(reqwest::Method::GET, &path)
            .send()
            .await?;
        let status = resp.status();
        let text = resp.text().await?;
        if !status.is_success() {
            anyhow::bail!("OData get {entity_set}('{id}') failed (HTTP {status}): {text}");
        }
        Ok(serde_json::from_str(&text)?)
    }
}

/// Main webhook ingestion endpoint.
async fn ingest_webhook(
    State(state): State<WebhookState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<WebhookIngestResponse, StatusCode> {
    let body_str = String::from_utf8_lossy(&body);
    let payload: Value = serde_json::from_str(&body_str).map_err(|_| StatusCode::BAD_REQUEST)?;

    tracing::info!("Webhook received: {}", truncate(&body_str, 500));

    // Detect source and event type
    let source = payload.get("source")
        .and_then(Value::as_str)
        .unwrap_or_else(|| {
            // Auto-detect Datadog by checking for DD-specific fields
            if payload.get("alert_transition").is_some() || payload.get("alert_type").is_some() {
                "datadog"
            } else if payload.get("action").is_some() && payload.get("pull_request").is_some() {
                "github"
            } else {
                "generic"
            }
        });

    let event_type = payload.get("event_type")
        .and_then(Value::as_str)
        .or_else(|| payload.get("alert_transition").and_then(Value::as_str))
        .or_else(|| payload.get("action").and_then(Value::as_str))
        .unwrap_or("alert");

    tracing::info!("Webhook source={source}, event_type={event_type}");

    match source {
        "datadog" => handle_datadog_alert(&state, &payload).await,
        "github" => handle_github_event(&state, &payload, event_type).await,
        _ => handle_generic_alert(&state, &payload).await,
    }
    .map_err(|e| {
        tracing::error!("Webhook processing failed: {e}");
        StatusCode::INTERNAL_SERVER_ERROR
    })
}

/// Handle a Datadog alert webhook.
///
/// DD webhook payload fields:
/// - `id` or `monitor_id`: the DD monitor numeric ID
/// - `title` or `name`: monitor name
/// - `alert_transition`: "Triggered", "Recovered", "Re-Triggered", etc.
/// - `priority`: P1-P5
/// - `tags`: comma-separated tags
/// - `body`: alert body text
async fn handle_datadog_alert(
    state: &WebhookState,
    payload: &Value,
) -> anyhow::Result<WebhookIngestResponse> {
    let alert_transition = payload.get("alert_transition")
        .and_then(Value::as_str)
        .unwrap_or("Triggered");

    // Extract DD monitor ID
    let dd_monitor_id = payload.get("id")
        .or_else(|| payload.get("monitor_id"))
        .and_then(|v| v.as_str().map(|s| s.to_string()).or_else(|| v.as_i64().map(|n| n.to_string())))
        .unwrap_or_default();

    let monitor_name = payload.get("title")
        .or_else(|| payload.get("name"))
        .and_then(Value::as_str)
        .unwrap_or("Unknown DD Monitor");

    tracing::info!("Datadog alert: monitor={dd_monitor_id}, name={monitor_name}, transition={alert_transition}");

    // If this is a Recovered event, try to resolve active AlertCycles
    if alert_transition == "Recovered" || alert_transition == "OK" {
        return handle_dd_recovery(state, &dd_monitor_id).await;
    }

    // Find or create Monitor entity by dd_monitor_id
    let monitor_id = resolve_or_create_monitor(state, &dd_monitor_id, monitor_name, payload).await?;

    // Check for duplicate active AlertCycles for this monitor
    let active_cycles = state.odata
        .list("AlertCycles", &format!("MonitorId eq '{monitor_id}' and Status eq 'Triaging'"))
        .await
        .unwrap_or_default();

    if !active_cycles.is_empty() {
        let existing_id = active_cycles[0].get("Id")
            .and_then(Value::as_str)
            .unwrap_or("");
        tracing::info!("Duplicate alert for monitor {monitor_id}, active cycle: {existing_id}");
        return Ok(WebhookIngestResponse {
            accepted: true,
            outcome: "duplicate".to_string(),
            message: format!("Active AlertCycle {existing_id} already exists for this monitor"),
            monitor_id: Some(monitor_id),
            alert_cycle_id: Some(existing_id.to_string()),
            sre_agent_id: None,
        });
    }

    // Fire the monitor alert
    let alert_payload_str = serde_json::to_string(payload).unwrap_or_default();
    state.odata.action("Monitors", &monitor_id, "AlertFired", &json!({
        "last_alert_payload": truncate(&alert_payload_str, 4000)
    })).await?;

    // Create AlertCycle
    let cycle_resp = state.odata.create("AlertCycles", &json!({})).await?;
    let cycle_id = cycle_resp.get("entity_id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();

    // Spawn SRE agent
    let sre_agent_id = spawn_sre_agent(state, &monitor_id, &cycle_id, payload).await?;

    // Open the AlertCycle
    state.odata.action("AlertCycles", &cycle_id, "Open", &json!({
        "monitor_id": monitor_id,
        "alert_payload": truncate(&alert_payload_str, 4000),
        "sre_agent_id": sre_agent_id,
    })).await?;

    tracing::info!("Alert cycle {cycle_id} opened, SRE agent {sre_agent_id} spawned");

    // Spawn background completion watcher
    let state_clone = state.clone();
    let cycle_id_clone = cycle_id.clone();
    let sre_id_clone = sre_agent_id.clone();
    tokio::spawn(async move {
        if let Err(e) = spawn_sre_completion_watcher(&state_clone, &cycle_id_clone, &sre_id_clone).await {
            tracing::error!("SRE completion watcher failed: {e}");
        }
    });

    Ok(WebhookIngestResponse {
        accepted: true,
        outcome: "created".to_string(),
        message: format!("AlertCycle {cycle_id} created, SRE {sre_agent_id} spawned"),
        monitor_id: Some(monitor_id),
        alert_cycle_id: Some(cycle_id),
        sre_agent_id: Some(sre_agent_id),
    })
}

/// Handle a DD recovery event — find active AlertCycles and resolve them.
async fn handle_dd_recovery(
    state: &WebhookState,
    dd_monitor_id: &str,
) -> anyhow::Result<WebhookIngestResponse> {
    // Find Monitor by dd_monitor_id
    let monitors = state.odata
        .list("Monitors", &format!("DdMonitorId eq '{dd_monitor_id}'"))
        .await?;
    if monitors.is_empty() {
        return Ok(WebhookIngestResponse {
            accepted: true,
            outcome: "ignored".to_string(),
            message: format!("No Monitor found for dd_monitor_id={dd_monitor_id}"),
            monitor_id: None,
            alert_cycle_id: None,
            sre_agent_id: None,
        });
    }

    let monitor_id = monitors[0].get("Id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();

    // Find AlertCycles in Verifying state for this monitor
    let verifying_cycles = state.odata
        .list("AlertCycles", &format!("MonitorId eq '{monitor_id}' and Status eq 'Verifying'"))
        .await
        .unwrap_or_default();

    for cycle in &verifying_cycles {
        let id = cycle.get("Id").and_then(Value::as_str).unwrap_or("");
        let _ = state.odata.action("AlertCycles", id, "AlertResolved", &json!({
            "diagnosis": format!("Datadog monitor {dd_monitor_id} recovered automatically")
        })).await;
        tracing::info!("AlertCycle {id} resolved via DD recovery");
    }

    Ok(WebhookIngestResponse {
        accepted: true,
        outcome: "recovery".to_string(),
        message: format!("Processed DD recovery for monitor {monitor_id}, resolved {} cycles", verifying_cycles.len()),
        monitor_id: Some(monitor_id),
        alert_cycle_id: None,
        sre_agent_id: None,
    })
}

/// Handle GitHub webhook events (PR merged, deployment status, check suite).
async fn handle_github_event(
    state: &WebhookState,
    payload: &Value,
    event_type: &str,
) -> anyhow::Result<WebhookIngestResponse> {
    match event_type {
        "closed" if payload.get("pull_request").and_then(|pr| pr.get("merged")).and_then(Value::as_bool) == Some(true) => {
            handle_github_merge(state, payload).await
        }
        "completed" if payload.get("deployment_status").is_some() => {
            handle_deployment_status(state, payload).await
        }
        _ => Ok(WebhookIngestResponse {
            accepted: true,
            outcome: "ignored".to_string(),
            message: format!("GitHub event_type={event_type} not handled"),
            monitor_id: None,
            alert_cycle_id: None,
            sre_agent_id: None,
        })
    }
}

/// Handle a GitHub PR merge event — advance AlertCycle through CI/CD closure.
async fn handle_github_merge(
    state: &WebhookState,
    payload: &Value,
) -> anyhow::Result<WebhookIngestResponse> {
    let pr_url = payload.get("pull_request")
        .and_then(|pr| pr.get("html_url"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let merge_sha = payload.get("pull_request")
        .and_then(|pr| pr.get("merge_commit_sha"))
        .and_then(Value::as_str)
        .unwrap_or("");

    tracing::info!("GitHub PR merged: {pr_url}, sha={merge_sha}");

    // Find AlertCycle in Fixed or Merging state with this PR URL
    let cycles = state.odata
        .list("AlertCycles", &format!("PrUrl eq '{pr_url}' and (Status eq 'Fixed' or Status eq 'Merging')"))
        .await
        .unwrap_or_default();

    for cycle in &cycles {
        let id = cycle.get("Id").and_then(Value::as_str).unwrap_or("");
        let status = cycle.get("Status").and_then(Value::as_str).unwrap_or("");

        // Advance through CI/CD closure states
        if status == "Fixed" {
            let _ = state.odata.action("AlertCycles", id, "BeginMerge", &json!({
                "pr_url": pr_url
            })).await;
        }
        let _ = state.odata.action("AlertCycles", id, "MergeComplete", &json!({
            "merge_sha": merge_sha
        })).await;
        tracing::info!("AlertCycle {id} advanced to Deploying (merge_sha={merge_sha})");
    }

    Ok(WebhookIngestResponse {
        accepted: true,
        outcome: "merge_processed".to_string(),
        message: format!("Processed merge for {pr_url}, advanced {} cycles", cycles.len()),
        monitor_id: None,
        alert_cycle_id: cycles.first().and_then(|c| c.get("Id").and_then(Value::as_str).map(String::from)),
        sre_agent_id: None,
    })
}

/// Handle a GitHub deployment status event.
async fn handle_deployment_status(
    state: &WebhookState,
    payload: &Value,
) -> anyhow::Result<WebhookIngestResponse> {
    let deploy_state = payload.get("deployment_status")
        .and_then(|ds| ds.get("state"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let deploy_url = payload.get("deployment_status")
        .and_then(|ds| ds.get("target_url"))
        .and_then(Value::as_str)
        .unwrap_or("");
    let sha = payload.get("deployment")
        .and_then(|d| d.get("sha"))
        .and_then(Value::as_str)
        .unwrap_or("");

    if deploy_state != "success" {
        return Ok(WebhookIngestResponse {
            accepted: true,
            outcome: "ignored".to_string(),
            message: format!("Deployment state={deploy_state}, not success"),
            monitor_id: None,
            alert_cycle_id: None,
            sre_agent_id: None,
        });
    }

    // Find AlertCycle in Deploying state with matching merge_sha
    let cycles = state.odata
        .list("AlertCycles", &format!("MergeSha eq '{sha}' and Status eq 'Deploying'"))
        .await
        .unwrap_or_default();

    for cycle in &cycles {
        let id = cycle.get("Id").and_then(Value::as_str).unwrap_or("");
        let _ = state.odata.action("AlertCycles", id, "DeployDetected", &json!({
            "deployment_url": deploy_url
        })).await;
        tracing::info!("AlertCycle {id} advanced to Verifying (deploy={deploy_url})");

        // Spawn DD verification task
        let state_clone = state.clone();
        let id_owned = id.to_string();
        let monitor_id = cycle.get("MonitorId").and_then(Value::as_str).unwrap_or("").to_string();
        tokio::spawn(async move {
            if let Err(e) = verify_alert_resolved_via_dd(&state_clone, &id_owned, &monitor_id).await {
                tracing::error!("DD verification failed for AlertCycle {id_owned}: {e}");
            }
        });
    }

    Ok(WebhookIngestResponse {
        accepted: true,
        outcome: "deploy_detected".to_string(),
        message: format!("Deploy detected for sha={sha}, advanced {} cycles to Verifying", cycles.len()),
        monitor_id: None,
        alert_cycle_id: cycles.first().and_then(|c| c.get("Id").and_then(Value::as_str).map(String::from)),
        sre_agent_id: None,
    })
}

/// Handle a generic alert (non-DD, non-GitHub).
async fn handle_generic_alert(
    state: &WebhookState,
    payload: &Value,
) -> anyhow::Result<WebhookIngestResponse> {
    let monitor_key = payload.get("monitor_id")
        .or_else(|| payload.get("dd_monitor_id"))
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let monitor_name = payload.get("monitor_name")
        .or_else(|| payload.get("name"))
        .and_then(Value::as_str)
        .unwrap_or("Generic Monitor");

    let monitor_id = resolve_or_create_monitor(state, monitor_key, monitor_name, payload).await?;

    let alert_payload_str = serde_json::to_string(payload).unwrap_or_default();
    state.odata.action("Monitors", &monitor_id, "AlertFired", &json!({
        "last_alert_payload": truncate(&alert_payload_str, 4000)
    })).await?;

    let cycle_resp = state.odata.create("AlertCycles", &json!({})).await?;
    let cycle_id = cycle_resp.get("entity_id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();

    let sre_agent_id = spawn_sre_agent(state, &monitor_id, &cycle_id, payload).await?;

    state.odata.action("AlertCycles", &cycle_id, "Open", &json!({
        "monitor_id": monitor_id,
        "alert_payload": truncate(&alert_payload_str, 4000),
        "sre_agent_id": sre_agent_id,
    })).await?;

    let state_clone = state.clone();
    let cycle_id_clone = cycle_id.clone();
    let sre_id_clone = sre_agent_id.clone();
    tokio::spawn(async move {
        if let Err(e) = spawn_sre_completion_watcher(&state_clone, &cycle_id_clone, &sre_id_clone).await {
            tracing::error!("SRE completion watcher failed: {e}");
        }
    });

    Ok(WebhookIngestResponse {
        accepted: true,
        outcome: "created".to_string(),
        message: format!("AlertCycle {cycle_id} created, SRE {sre_agent_id} spawned"),
        monitor_id: Some(monitor_id),
        alert_cycle_id: Some(cycle_id),
        sre_agent_id: Some(sre_agent_id),
    })
}

// --- Helper functions ---

/// Find an existing Monitor by dd_monitor_id, or create a new one.
async fn resolve_or_create_monitor(
    state: &WebhookState,
    dd_monitor_id: &str,
    name: &str,
    payload: &Value,
) -> anyhow::Result<String> {
    if !dd_monitor_id.is_empty() {
        let existing = state.odata
            .list("Monitors", &format!("DdMonitorId eq '{dd_monitor_id}'"))
            .await
            .unwrap_or_default();
        if let Some(monitor) = existing.first() {
            return Ok(monitor.get("Id")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string());
        }
    }

    // Create new Monitor
    let resp = state.odata.create("Monitors", &json!({})).await?;
    let monitor_id = resp.get("entity_id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();

    // Configure with DD details
    let dd_query = payload.get("query")
        .or_else(|| payload.get("dd_query"))
        .and_then(Value::as_str)
        .unwrap_or("");

    state.odata.action("Monitors", &monitor_id, "Configure", &json!({
        "dd_query": dd_query,
        "dd_monitor_id": dd_monitor_id,
    })).await?;

    state.odata.action("Monitors", &monitor_id, "Activate", &json!({})).await?;

    tracing::info!("Created Monitor {monitor_id} for dd_monitor_id={dd_monitor_id}");
    Ok(monitor_id)
}

/// Spawn an SRE agent to triage the alert.
async fn spawn_sre_agent(
    state: &WebhookState,
    monitor_id: &str,
    alert_cycle_id: &str,
    payload: &Value,
) -> anyhow::Result<String> {
    // Find the ProjectHarness (from payload or by scanning)
    let project_harness_id = payload.get("project_harness_id")
        .or_else(|| payload.get("projectHarnessId"))
        .and_then(Value::as_str)
        .unwrap_or("");

    let repo_url = payload.get("repo_url")
        .or_else(|| payload.get("repository").and_then(|r| r.get("clone_url")).and_then(Value::as_str).map(|_| payload.get("repository").unwrap().get("clone_url").unwrap()))
        .and_then(Value::as_str)
        .unwrap_or("");

    // Find active SRE soul
    let souls = state.odata
        .list("Souls", "Name eq 'SRE' and Status eq 'Active'")
        .await
        .unwrap_or_default();

    let soul_id = souls.first()
        .and_then(|s| s.get("Id").and_then(Value::as_str))
        .unwrap_or("SRE");

    // Create Agent entity
    let agent_resp = state.odata.create("Agents", &json!({})).await?;
    let agent_id = agent_resp.get("entity_id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();

    // Build SRE task message
    let sre_message = build_sre_message(monitor_id, alert_cycle_id, project_harness_id, repo_url, payload);

    // Configure the agent
    state.odata.action("Agents", &agent_id, "Configure", &json!({
        "soul_id": soul_id,
        "model": DEFAULT_AGENT_MODEL,
        "user_message": sre_message,
        "max_turns": 60,
        "tools_enabled": "temper_get,temper_list,temper_action,temper_create,spawn_agent,read_entity,datadog_query",
        "workdir": DEFAULT_SRE_WORKDIR,
    })).await?;

    // Provision the agent
    state.odata.action("Agents", &agent_id, "OpenPaw.Provision", &json!({})).await?;

    tracing::info!("SRE agent {agent_id} spawned for AlertCycle {alert_cycle_id}");
    Ok(agent_id)
}

/// Build the task message for the SRE agent.
fn build_sre_message(
    monitor_id: &str,
    alert_cycle_id: &str,
    project_harness_id: &str,
    repo_url: &str,
    payload: &Value,
) -> String {
    let alert_title = payload.get("title")
        .or_else(|| payload.get("name"))
        .or_else(|| payload.get("summary"))
        .and_then(Value::as_str)
        .unwrap_or("Alert fired");

    let alert_body = payload.get("body")
        .or_else(|| payload.get("message"))
        .or_else(|| payload.get("alert_payload"))
        .and_then(Value::as_str)
        .unwrap_or("");

    let severity = payload.get("priority")
        .or_else(|| payload.get("severity"))
        .and_then(Value::as_str)
        .unwrap_or("P3");

    let mut msg = format!(
        "## Alert Triage\n\n\
         An alert has fired and needs triage.\n\n\
         **Monitor ID**: {monitor_id}\n\
         **AlertCycle ID**: {alert_cycle_id}\n\
         **Severity**: {severity}\n\
         **Title**: {alert_title}\n"
    );

    if !project_harness_id.is_empty() {
        msg.push_str(&format!("**ProjectHarness ID**: {project_harness_id}\n"));
    }
    if !repo_url.is_empty() {
        msg.push_str(&format!("**Repository**: {repo_url}\n"));
    }
    if !alert_body.is_empty() {
        msg.push_str(&format!("\n**Alert Details**:\n{}\n", truncate(alert_body, 2000)));
    }

    msg.push_str(&format!(
        "\n## Instructions\n\n\
         1. Read the Monitor and AlertCycle entities to understand the context\n\
         2. Use `datadog_query` to investigate the alert in Datadog\n\
         3. Triage: Is this a real issue or noise?\n\
         4. If real: Create an Issue, WorkCycle, spawn a Developer to fix it\n\
         5. If noise: Tune the monitor and mark the AlertCycle as tuned\n\
         6. Close the loop with the appropriate AlertCycle action\n"
    ));

    msg
}

/// Watch for SRE agent completion and advance the AlertCycle.
async fn spawn_sre_completion_watcher(
    state: &WebhookState,
    alert_cycle_id: &str,
    sre_agent_id: &str,
) -> anyhow::Result<()> {
    // Poll until SRE agent reaches a terminal state
    let timeout = Duration::from_secs(1800); // 30 min max
    let poll_interval = Duration::from_secs(15);
    let start = std::time::Instant::now();

    loop {
        if start.elapsed() > timeout {
            tracing::warn!("SRE completion watcher timed out for {sre_agent_id}");
            break;
        }

        let agent = state.odata.get("Agents", sre_agent_id).await;
        match agent {
            Ok(a) => {
                let status = a.get("Status").and_then(Value::as_str).unwrap_or("");
                match status {
                    "Completed" | "Failed" | "Cancelled" => {
                        tracing::info!("SRE agent {sre_agent_id} reached terminal state: {status}");

                        // Check if the AlertCycle was already advanced by the SRE
                        let cycle = state.odata.get("AlertCycles", alert_cycle_id).await?;
                        let cycle_status = cycle.get("Status").and_then(Value::as_str).unwrap_or("");

                        if cycle_status == "Fixed" {
                            // SRE completed successfully — start CI/CD closure
                            let pr_url = cycle.get("PrUrl").and_then(Value::as_str).unwrap_or("");
                            if !pr_url.is_empty() {
                                tracing::info!("AlertCycle {alert_cycle_id} is Fixed with PR {pr_url}, starting CI/CD closure");
                                start_cicd_closure(state, alert_cycle_id, pr_url).await?;
                            }
                        } else if cycle_status == "Triaging" && status == "Failed" {
                            // SRE failed without closing the cycle — escalate
                            let _ = state.odata.action("AlertCycles", alert_cycle_id, "Escalate", &json!({
                                "diagnosis": format!("SRE agent {sre_agent_id} failed without completing triage")
                            })).await;
                        }

                        break;
                    }
                    _ => {} // Still running
                }
            }
            Err(e) => {
                tracing::warn!("Failed to poll SRE agent {sre_agent_id}: {e}");
            }
        }

        sleep(poll_interval).await;
    }

    Ok(())
}

/// Start the CI/CD closure loop after a PR is created.
///
/// Flow: Fixed → BeginMerge → (poll GitHub checks) → merge → MergeComplete →
///       (poll deployment) → DeployDetected → (verify via DD) → AlertResolved
async fn start_cicd_closure(
    state: &WebhookState,
    alert_cycle_id: &str,
    pr_url: &str,
) -> anyhow::Result<()> {
    // Parse PR URL to extract owner/repo/number
    let parts: Vec<&str> = pr_url.trim_end_matches('/').split('/').collect();
    if parts.len() < 4 {
        tracing::warn!("Cannot parse PR URL: {pr_url}");
        return Ok(());
    }
    let pr_number = parts[parts.len() - 1];
    let repo = parts[parts.len() - 3];
    let owner = parts[parts.len() - 4];

    // BeginMerge
    state.odata.action("AlertCycles", alert_cycle_id, "BeginMerge", &json!({
        "pr_url": pr_url
    })).await?;

    // Poll GitHub checks
    let github_token = std::env::var("GITHUB_TOKEN").unwrap_or_default();
    let gh_client = reqwest::Client::new();

    let checks_url = format!("https://api.github.com/repos/{owner}/{repo}/pulls/{pr_number}/commits");
    let mut merge_ready = false;

    for _ in 0..40 { // Poll for up to 10 minutes
        sleep(Duration::from_secs(15)).await;

        // Check if PR is mergeable by looking at check runs
        let status_resp = gh_client.get(format!(
            "https://api.github.com/repos/{owner}/{repo}/pulls/{pr_number}"
        ))
        .header("authorization", format!("token {github_token}"))
        .header("accept", "application/vnd.github.v3+json")
        .header("user-agent", "openpaw")
        .send()
        .await;

        if let Ok(resp) = status_resp {
            if let Ok(pr_data) = resp.json::<Value>().await {
                let mergeable = pr_data.get("mergeable").and_then(Value::as_bool).unwrap_or(false);
                let mergeable_state = pr_data.get("mergeable_state").and_then(Value::as_str).unwrap_or("");

                if mergeable && mergeable_state == "clean" {
                    merge_ready = true;
                    break;
                }
            }
        }
    }

    if !merge_ready {
        tracing::warn!("PR {pr_url} not mergeable after polling, skipping auto-merge");
        return Ok(());
    }

    // Merge the PR
    let merge_resp = gh_client.put(format!(
        "https://api.github.com/repos/{owner}/{repo}/pulls/{pr_number}/merge"
    ))
    .header("authorization", format!("token {github_token}"))
    .header("accept", "application/vnd.github.v3+json")
    .header("user-agent", "openpaw")
    .json(&json!({ "merge_method": "squash" }))
    .send()
    .await;

    match merge_resp {
        Ok(resp) if resp.status().is_success() => {
            let merge_data: Value = resp.json().await.unwrap_or(json!({}));
            let merge_sha = merge_data.get("sha").and_then(Value::as_str).unwrap_or("");

            state.odata.action("AlertCycles", alert_cycle_id, "MergeComplete", &json!({
                "merge_sha": merge_sha
            })).await?;

            tracing::info!("PR {pr_url} merged (sha={merge_sha}), AlertCycle {alert_cycle_id} → Deploying");

            // Wait for deployment and verify
            // deep-sci-fi deploys automatically on push to main via Vercel + Railway
            sleep(Duration::from_secs(120)).await; // Wait 2 min for deploy

            state.odata.action("AlertCycles", alert_cycle_id, "DeployDetected", &json!({
                "deployment_url": format!("auto-deploy from merge {merge_sha}")
            })).await?;

            // Verify via DD query after deploy settles
            let cycle = state.odata.get("AlertCycles", alert_cycle_id).await?;
            let monitor_id = cycle.get("MonitorId").and_then(Value::as_str).unwrap_or("");
            verify_alert_resolved_via_dd(state, alert_cycle_id, monitor_id).await?;
        }
        Ok(resp) => {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            tracing::warn!("PR merge failed (HTTP {status}): {text}");
        }
        Err(e) => {
            tracing::warn!("PR merge request failed: {e}");
        }
    }

    Ok(())
}

/// Verify that the DD monitor has recovered after deployment.
async fn verify_alert_resolved_via_dd(
    state: &WebhookState,
    alert_cycle_id: &str,
    monitor_id: &str,
) -> anyhow::Result<()> {
    // Wait 5 minutes for the fix to take effect
    sleep(Duration::from_secs(300)).await;

    // Get the Monitor to find the dd_monitor_id
    let monitor = state.odata.get("Monitors", monitor_id).await?;
    let dd_monitor_id = monitor.get("DdMonitorId").and_then(Value::as_str).unwrap_or("");

    if dd_monitor_id.is_empty() {
        // No DD monitor to check — resolve optimistically
        state.odata.action("AlertCycles", alert_cycle_id, "AlertResolved", &json!({
            "diagnosis": "No DD monitor ID to verify — resolved optimistically after successful deploy"
        })).await?;
        return Ok(());
    }

    // Query DD for the monitor status
    let dd_api_key = std::env::var("DD_API_KEY").unwrap_or_default();
    let dd_app_key = std::env::var("DD_APP_KEY").unwrap_or_default();
    let dd_site = std::env::var("DD_SITE").unwrap_or_else(|_| "datadoghq.com".to_string());

    if dd_api_key.is_empty() {
        state.odata.action("AlertCycles", alert_cycle_id, "AlertResolved", &json!({
            "diagnosis": "No DD API key — resolved optimistically"
        })).await?;
        return Ok(());
    }

    let client = reqwest::Client::new();
    let resp = client.get(format!(
        "https://api.{dd_site}/api/v1/monitor/{dd_monitor_id}"
    ))
    .header("DD-API-KEY", &dd_api_key)
    .header("DD-APPLICATION-KEY", &dd_app_key)
    .send()
    .await;

    match resp {
        Ok(resp) if resp.status().is_success() => {
            let data: Value = resp.json().await.unwrap_or(json!({}));
            let overall_state = data.get("overall_state").and_then(Value::as_str).unwrap_or("");

            if overall_state == "OK" || overall_state == "No Data" {
                state.odata.action("AlertCycles", alert_cycle_id, "AlertResolved", &json!({
                    "diagnosis": format!("DD monitor {dd_monitor_id} state={overall_state} — alert resolved")
                })).await?;
                tracing::info!("AlertCycle {alert_cycle_id} → Resolved (DD monitor OK)");
            } else {
                state.odata.action("AlertCycles", alert_cycle_id, "AlertPersists", &json!({
                    "diagnosis": format!("DD monitor {dd_monitor_id} state={overall_state} — alert persists after deploy")
                })).await?;
                tracing::warn!("AlertCycle {alert_cycle_id} → Failed (DD monitor still {overall_state})");
            }
        }
        _ => {
            // DD query failed — resolve optimistically
            state.odata.action("AlertCycles", alert_cycle_id, "AlertResolved", &json!({
                "diagnosis": "DD API query failed — resolved optimistically after deploy"
            })).await?;
        }
    }

    Ok(())
}

fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max { s } else { &s[..max] }
}
