//! Webhook ingestion routes for external alert sources.
//!
//! These routes intentionally feed Open Paw through its public OData surface:
//! the handler creates AlertCycle entities over HTTP and dispatches bound
//! actions rather than mutating platform state directly in-process.

use anyhow::{Context, Result};
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::post;
use axum::{Json, Router};
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

pub fn router(config: WebhookConfig) -> Router {
    Router::new()
        .route("/webhooks/alerts", post(handle_alert))
        .with_state(config)
}

async fn handle_alert(
    State(config): State<WebhookConfig>,
    Json(payload): Json<Value>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    let alert_cycle_id = payload
        .get("alert_cycle_id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    let monitor_id = payload
        .get("monitor_id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    let scout_agent_id = payload
        .get("scout_agent_id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    let alert_payload = payload
        .get("alert_payload")
        .map(json_value_to_payload_string)
        .unwrap_or_else(|| payload.to_string());

    let created = odata_post(
        &config,
        &format!("{}/tdata/AlertCycles", config.api_url),
        if alert_cycle_id.is_empty() {
            json!({})
        } else {
            json!({ "Id": alert_cycle_id })
        },
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
                .and_then(|fields| fields.get("Id"))
                .and_then(Value::as_str)
        })
        .ok_or_else(|| internal_error(anyhow::anyhow!("AlertCycle creation did not return Id")))?;

    odata_post(
        &config,
        &format!(
            "{}/tdata/AlertCycles('{created_id}')/OpenPaw.Heal.Open",
            config.api_url
        ),
        json!({
            "monitor_id": monitor_id,
            "alert_payload": alert_payload,
            "scout_agent_id": scout_agent_id,
        }),
    )
    .await
    .map_err(internal_error)?;

    if !monitor_id.is_empty() {
        odata_post(
            &config,
            &format!(
                "{}/tdata/Monitors('{monitor_id}')/OpenPaw.Heal.AlertFired",
                config.api_url
            ),
            json!({
                "last_alert_payload": alert_payload,
            }),
        )
        .await
        .map_err(internal_error)?;
    }

    Ok((
        StatusCode::ACCEPTED,
        Json(json!({
            "ok": true,
            "alert_cycle_id": created_id,
            "monitor_id": monitor_id,
            "status": "Triaging",
        })),
    ))
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
    let text = resp
        .text()
        .await
        .context("failed to read webhook OData response")?;
    if !status.is_success() {
        anyhow::bail!("OData POST {url} returned {status}: {text}");
    }
    if text.trim().is_empty() {
        Ok(Value::Null)
    } else {
        serde_json::from_str(&text).context("failed to parse webhook OData JSON")
    }
}
