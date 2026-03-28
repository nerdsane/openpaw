//! Webhook ingestion routes for external alert sources.
//!
//! These routes intentionally feed Open Paw through its public OData surface:
//! the handler creates AlertCycle entities over HTTP and dispatches bound
//! actions rather than mutating platform state directly in-process.
//!
//! Enhanced with: duplicate prevention, Scout auto-spawn, proactive reporting,
//! GitHub PR event handling, and Logfire/Datadog source routing.

use anyhow::{Context, Result};
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::post;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Clone)]
pub struct WebhookConfig {
    api_url: String,
    tenant: String,
    api_key: Option<String>,
    http: reqwest::Client,
}

impl WebhookConfig {
    pub fn new(api_url: String, tenant: String, api_key: Option<String>) -> Self {
        Self {
            api_url,
            tenant,
            api_key,
            http: reqwest::Client::new(),
        }
    }
}

/// Incoming webhook payload envelope.
#[derive(Debug, Deserialize)]
pub struct WebhookPayload {
    /// Source platform: "logfire", "datadog", or "github".
    #[serde(default = "default_source")]
    pub source: String,
    /// Event type — e.g. "alert", "pull_request.merged".
    #[serde(default)]
    pub event_type: String,
    // Fields used by all alert paths (backward compat with Codex format)
    #[serde(default)]
    pub alert_cycle_id: Option<String>,
    #[serde(default)]
    pub monitor_id: Option<String>,
    #[serde(default)]
    pub scout_agent_id: Option<String>,
    #[serde(default)]
    pub alert_payload: Option<Value>,
    /// Opaque nested payload (used by envelope format: {source, event_type, payload})
    #[serde(default)]
    pub payload: Option<Value>,
    /// Optional Channel entity ID for proactive reporting after triage.
    #[serde(default)]
    pub report_channel_id: Option<String>,
    /// Optional thread ID for proactive reporting.
    #[serde(default)]
    pub report_thread_id: Option<String>,
}

fn default_source() -> String {
    "logfire".to_string()
}

pub fn router(config: WebhookConfig) -> Router {
    Router::new()
        .route("/webhooks/alerts", post(handle_webhook))
        .route("/webhooks/ingest", post(handle_webhook))
        .with_state(config)
}

/// Unified webhook handler — supports both the Codex flat format and the
/// envelope format {source, event_type, payload}.
async fn handle_webhook(
    State(config): State<WebhookConfig>,
    Json(payload): Json<WebhookPayload>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    match payload.source.as_str() {
        "github" => handle_github(&config, &payload).await,
        _ => handle_alert(&config, &payload).await,
    }
}

/// Handle an alert from Logfire, Datadog, or the flat Codex format.
async fn handle_alert(
    config: &WebhookConfig,
    payload: &WebhookPayload,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    // Extract fields — support both flat format and envelope format
    let monitor_id = payload
        .monitor_id
        .clone()
        .or_else(|| {
            payload
                .payload
                .as_ref()
                .and_then(|p| p.get("monitor_id"))
                .and_then(Value::as_str)
                .map(String::from)
        })
        .unwrap_or_default();

    let alert_payload_str = if let Some(ref ap) = payload.alert_payload {
        json_value_to_payload_string(ap)
    } else if let Some(ref p) = payload.payload {
        p.to_string()
    } else {
        json!({}).to_string()
    };

    let scout_agent_id = payload.scout_agent_id.clone().unwrap_or_default();

    // Duplicate prevention: check for active AlertCycle on this monitor
    if !monitor_id.is_empty() {
        let filter = format!(
            "monitor_id eq '{}' and (status eq 'Created' or status eq 'Triaging')",
            monitor_id
        );
        if let Ok(existing) = odata_get_list(config, "AlertCycles", &filter).await {
            if !existing.is_empty() {
                let existing_id = existing[0]
                    .get("entity_id")
                    .or_else(|| existing[0].get("fields").and_then(|f| f.get("Id")))
                    .and_then(Value::as_str)
                    .unwrap_or("unknown");
                return Ok((
                    StatusCode::OK,
                    Json(json!({
                        "ok": true,
                        "alert_cycle_id": existing_id,
                        "monitor_id": monitor_id,
                        "status": "Triaging",
                        "message": format!("Duplicate: active AlertCycle exists for monitor {monitor_id}"),
                    })),
                ));
            }
        }
    }

    // Create AlertCycle
    let ac_body = match &payload.alert_cycle_id {
        Some(id) if !id.is_empty() => json!({ "Id": id }),
        _ => json!({}),
    };
    let created = odata_post(
        config,
        &format!("{}/tdata/AlertCycles", config.api_url),
        ac_body,
    )
    .await
    .map_err(internal_error)?;

    let created_id = created
        .get("entity_id")
        .and_then(Value::as_str)
        .or_else(|| created.get("Id").and_then(Value::as_str))
        .or_else(|| {
            created
                .get("fields")
                .and_then(|f| f.get("Id"))
                .and_then(Value::as_str)
        })
        .ok_or_else(|| internal_error(anyhow::anyhow!("AlertCycle creation did not return Id")))?
        .to_string();

    // Open the AlertCycle
    odata_post(
        config,
        &format!(
            "{}/tdata/AlertCycles('{created_id}')/OpenPaw.Heal.Open",
            config.api_url
        ),
        json!({
            "monitor_id": monitor_id,
            "alert_payload": alert_payload_str,
            "scout_agent_id": scout_agent_id,
        }),
    )
    .await
    .map_err(internal_error)?;

    // Fire AlertFired on Monitor if present
    if !monitor_id.is_empty() {
        let _ = odata_post(
            config,
            &format!(
                "{}/tdata/Monitors('{monitor_id}')/OpenPaw.Heal.AlertFired",
                config.api_url
            ),
            json!({ "last_alert_payload": alert_payload_str }),
        )
        .await;
    }

    tracing::info!(
        "Webhook ingested: Monitor={monitor_id} AlertCycle={created_id} source={}",
        "alert"
    );

    // Auto-spawn Scout agent to triage
    let scout_id = spawn_scout_for_alert(config, &monitor_id, &created_id, &alert_payload_str)
        .await;
    let mut message = String::new();
    if let Some(ref sid) = scout_id {
        tracing::info!("Auto-spawned Scout {sid} for AlertCycle {created_id}");
        message = format!("Scout auto-spawned: {sid}");

        // Proactive reporting: spawn background task if report_channel_id provided
        if let Some(ref channel_id) = payload.report_channel_id {
            let cfg = config.clone();
            let channel_id = channel_id.clone();
            let thread_id = payload
                .report_thread_id
                .clone()
                .unwrap_or_else(|| "alert-report".to_string());
            let sid = sid.clone();
            let acid = created_id.clone();
            let mid = monitor_id.clone();
            tokio::spawn(async move {
                report_after_scout_completes(&cfg, &sid, &acid, &mid, &channel_id, &thread_id)
                    .await;
            });
        }
    }

    Ok((
        StatusCode::ACCEPTED,
        Json(json!({
            "ok": true,
            "alert_cycle_id": created_id,
            "monitor_id": monitor_id,
            "status": "Triaging",
            "message": message,
        })),
    ))
}

/// Handle GitHub webhook events (PR merged/closed).
async fn handle_github(
    _config: &WebhookConfig,
    payload: &WebhookPayload,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    let pr_url = payload
        .payload
        .as_ref()
        .and_then(|p| p.get("pull_request"))
        .and_then(|pr| pr.get("html_url"))
        .and_then(Value::as_str)
        .unwrap_or("");

    if pr_url.is_empty() {
        return Ok((
            StatusCode::OK,
            Json(json!({
                "ok": true,
                "message": format!("Ignoring GitHub event: {}", payload.event_type),
            })),
        ));
    }

    tracing::info!("GitHub PR event: {} {}", payload.event_type, pr_url);
    Ok((
        StatusCode::OK,
        Json(json!({
            "ok": true,
            "message": format!("GitHub event received for PR {pr_url}"),
        })),
    ))
}

/// Auto-spawn a Scout agent to triage an AlertCycle.
async fn spawn_scout_for_alert(
    config: &WebhookConfig,
    monitor_id: &str,
    alert_cycle_id: &str,
    alert_payload_str: &str,
) -> Option<String> {
    // Find active Scout soul
    let souls = odata_get_list(config, "Souls", "status eq 'Active' and Name eq 'Scout'")
        .await
        .ok()?;
    if souls.is_empty() {
        tracing::warn!("No active Scout soul — skipping auto-spawn");
        return None;
    }

    // Extract context from alert payload
    let alert_data: Value = serde_json::from_str(alert_payload_str).unwrap_or_default();
    let project_harness_id = alert_data
        .get("project_harness_id")
        .and_then(Value::as_str)
        .unwrap_or("");
    let repo_url = alert_data
        .get("repo_url")
        .and_then(Value::as_str)
        .unwrap_or("");

    let sandbox_url = derive_sandbox_url(&config.api_url);

    let scout_message = format!(
        "You are handling a production alert that needs triage.\n\
         \n\
         Workflow entity IDs:\n\
         - Monitor: {monitor_id}\n\
         - AlertCycle: {alert_cycle_id}\n\
         {harness_line}\
         {repo_line}\
         \n\
         Alert payload:\n\
         {alert_payload_str}\n\
         \n\
         Follow your standard triage workflow:\n\
         1. Read the Monitor and AlertCycle entities.\n\
         {harness_step}\
         2. Assess whether this is a real issue or noise.\n\
         3. If real: create a PM Issue, create a WorkCycle, spawn a Developer to fix it.\n\
         4. If noise: tune the Monitor and mark the AlertCycle as tuned.\n\
         5. Report ALERT_CYCLE_STATUS, WORK_CYCLE_STATUS, ISSUE_ID, PR_URL in your final response.",
        harness_line = if project_harness_id.is_empty() {
            String::new()
        } else {
            format!("- ProjectHarness: {project_harness_id}\n")
        },
        repo_line = if repo_url.is_empty() {
            String::new()
        } else {
            format!("- Repository: {repo_url}\n")
        },
        harness_step = if project_harness_id.is_empty() {
            String::new()
        } else {
            format!("   Read the ProjectHarness ({project_harness_id}) for project context.\n")
        },
    );

    // Create Scout agent
    let scout = odata_post(
        config,
        &format!("{}/tdata/Agents", config.api_url),
        json!({}),
    )
    .await
    .ok()?;
    let scout_id = scout
        .get("entity_id")
        .and_then(Value::as_str)?
        .to_string();

    // Configure
    odata_post(
        config,
        &format!(
            "{}/tdata/Agents('{scout_id}')/OpenPaw.Configure",
            config.api_url
        ),
        json!({
            "model": "claude-sonnet-4-20250514",
            "provider": "anthropic",
            "max_turns": "60",
            "tools_enabled": "temper_get,temper_list,temper_action,temper_create,spawn_agent,read_entity",
            "sandbox_url": sandbox_url,
            "workdir": "/tmp/openpaw-scout-triage",
            "soul_id": "Scout",
            "user_message": scout_message,
        }),
    )
    .await
    .ok()?;

    // Provision
    odata_post(
        config,
        &format!(
            "{}/tdata/Agents('{scout_id}')/OpenPaw.Provision",
            config.api_url
        ),
        json!({}),
    )
    .await
    .ok()?;

    Some(scout_id)
}

/// Wait for Scout to complete, then send a proactive report to a Channel.
async fn report_after_scout_completes(
    config: &WebhookConfig,
    scout_id: &str,
    alert_cycle_id: &str,
    monitor_id: &str,
    channel_id: &str,
    thread_id: &str,
) {
    let timeout = std::time::Duration::from_secs(900);
    let start = std::time::Instant::now();

    loop {
        if start.elapsed() > timeout {
            tracing::warn!("Proactive report timeout: Scout {scout_id} did not complete");
            break;
        }
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;

        let scout = match odata_get(config, "Agents", scout_id).await {
            Ok(s) => s,
            Err(_) => continue,
        };

        let status = scout
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("");

        if !matches!(status, "Completed" | "Failed" | "Cancelled") {
            continue;
        }

        let result = scout
            .get("fields")
            .and_then(|f| f.get("Result"))
            .and_then(Value::as_str)
            .unwrap_or("(no result)");

        let ac_status = odata_get(config, "AlertCycles", alert_cycle_id)
            .await
            .ok()
            .and_then(|ac| ac.get("status").and_then(Value::as_str).map(String::from))
            .unwrap_or_else(|| "unknown".to_string());

        let pr_url = result
            .lines()
            .find(|l| l.starts_with("PR_URL="))
            .and_then(|l| l.strip_prefix("PR_URL="))
            .unwrap_or("");

        let report = format!(
            "**Alert Report** (Monitor: {monitor_id})\n\
             \nScout triage result: **{status}**\n\
             AlertCycle status: **{ac_status}**\n\
             {pr_line}\n\
             {preview}",
            pr_line = if pr_url.is_empty() { String::new() } else { format!("PR: {pr_url}\n") },
            preview = if result.len() > 500 { format!("{}...", &result[..500]) } else { result.to_string() },
        );

        let _ = odata_post(
            config,
            &format!(
                "{}/tdata/Channels('{channel_id}')/Paw.Channel.SendReply",
                config.api_url
            ),
            json!({
                "thread_id": thread_id,
                "content": report,
                "agent_entity_id": scout_id,
            }),
        )
        .await;

        tracing::info!("Proactive report sent for AlertCycle {alert_cycle_id}");
        return;
    }
}

fn derive_sandbox_url(api_url: &str) -> String {
    if let Some(colon_pos) = api_url.rfind(':') {
        if let Ok(port) = api_url[colon_pos + 1..].parse::<u16>() {
            return format!("{}:{}", &api_url[..colon_pos], port + 10);
        }
    }
    format!("{api_url}:3477")
}

fn simple_encode(s: &str) -> String {
    s.replace(' ', "%20")
        .replace('\'', "%27")
        .replace('(', "%28")
        .replace(')', "%29")
}

fn json_value_to_payload_string(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        _ => value.to_string(),
    }
}

fn internal_error(error: anyhow::Error) -> (StatusCode, Json<Value>) {
    tracing::error!(%error, "webhook alert ingestion failed");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({
            "ok": false,
            "error": error.to_string(),
        })),
    )
}

async fn odata_post(config: &WebhookConfig, url: &str, body: Value) -> Result<Value> {
    let mut req = config
        .http
        .post(url)
        .header("x-tenant-id", &config.tenant)
        .header("content-type", "application/json")
        .header("accept", "application/json")
        .json(&body);
    if let Some(api_key) = &config.api_key {
        req = req.header("authorization", format!("Bearer {api_key}"));
    } else {
        req = req.header("x-temper-principal-kind", "admin");
    }
    let resp = req.send().await.context("webhook OData POST failed")?;
    let status = resp.status();
    let text = resp.text().await.context("failed to read response")?;
    if !status.is_success() {
        anyhow::bail!("OData POST {url} returned {status}: {text}");
    }
    if text.trim().is_empty() {
        Ok(Value::Null)
    } else {
        serde_json::from_str(&text).context("failed to parse OData JSON")
    }
}

async fn odata_get(config: &WebhookConfig, entity_set: &str, entity_id: &str) -> Result<Value> {
    let url = format!(
        "{}/tdata/{}('{}')",
        config.api_url, entity_set, entity_id
    );
    let mut req = config
        .http
        .get(&url)
        .header("x-tenant-id", &config.tenant)
        .header("accept", "application/json");
    if let Some(api_key) = &config.api_key {
        req = req.header("authorization", format!("Bearer {api_key}"));
    } else {
        req = req.header("x-temper-principal-kind", "admin");
    }
    let resp = req.send().await.context("webhook OData GET failed")?;
    let text = resp.text().await.context("failed to read response")?;
    serde_json::from_str(&text).context("failed to parse OData JSON")
}

async fn odata_get_list(config: &WebhookConfig, entity_set: &str, filter: &str) -> Result<Vec<Value>> {
    let url = format!(
        "{}/tdata/{}?$filter={}",
        config.api_url,
        entity_set,
        simple_encode(filter)
    );
    let mut req = config
        .http
        .get(&url)
        .header("x-tenant-id", &config.tenant)
        .header("accept", "application/json");
    if let Some(api_key) = &config.api_key {
        req = req.header("authorization", format!("Bearer {api_key}"));
    } else {
        req = req.header("x-temper-principal-kind", "admin");
    }
    let resp = req.send().await.context("webhook OData GET list failed")?;
    let text = resp.text().await.context("failed to read response")?;
    let body: Value = serde_json::from_str(&text).context("failed to parse OData JSON")?;
    Ok(body
        .get("value")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default())
}
