//! Webhook trigger — thin HTTP endpoint for external webhook ingestion.
//!
//! ONE entity, ONE action. Creates a WebhookEvent entity and dispatches
//! the Received action. Everything else (validation, routing, processing)
//! is handled by WASM integrations on WebhookEvent state transitions.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::post;
use axum::{Json, Router};
use serde_json::{Value, json};

use crate::PawApiClient;

/// Configuration for the webhook trigger.
#[derive(Debug, Clone)]
pub struct WebhookTriggerConfig {
    /// Port to bind the webhook HTTP listener.
    pub port: u16,
}

/// Webhook trigger state shared across request handlers.
struct TriggerState {
    api: PawApiClient,
}

/// Webhook trigger — HTTP endpoint that creates WebhookEvent entities.
pub struct WebhookTrigger {
    config: WebhookTriggerConfig,
    api: PawApiClient,
}

impl WebhookTrigger {
    /// Create a new webhook trigger.
    pub fn new(config: WebhookTriggerConfig, api: PawApiClient) -> Self {
        Self { config, api }
    }

    /// Start the webhook trigger HTTP listener.
    ///
    /// Listens on `/triggers/webhook/{route_key}` for POST requests.
    /// For each request: creates ONE WebhookEvent entity, dispatches ONE
    /// Received action, returns the event ID.
    pub async fn run(&self) -> Result<(), String> {
        let state = Arc::new(TriggerState {
            api: self.api.clone(),
        });

        let app = Router::new()
            .route("/triggers/webhook/{route_key}", post(handle_webhook))
            .with_state(state);

        let addr = SocketAddr::from(([0, 0, 0, 0], self.config.port));
        println!("  [webhook] Trigger listening on {addr}");

        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .map_err(|e| format!("webhook trigger bind failed: {e}"))?;

        axum::serve(listener, app)
            .await
            .map_err(|e| format!("webhook trigger serve failed: {e}"))
    }
}

/// Handle an incoming webhook POST.
///
/// ONE entity, ONE action. Everything else is WASM.
async fn handle_webhook(
    State(state): State<Arc<TriggerState>>,
    Path(route_key): Path<String>,
    headers: HeaderMap,
    body: String,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    // Serialize headers to JSON for the WASM integration to inspect.
    let headers_json = serialize_headers(&headers);

    // ONE entity: create WebhookEvent.
    let entity = state
        .api
        .create_entity("WebhookEvents", json!({}))
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("create WebhookEvent failed: {e}") })),
            )
        })?;

    let event_id = entity
        .get("entity_id")
        .or_else(|| entity.get("Id"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    if event_id.is_empty() {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "WebhookEvent created but no entity_id returned" })),
        ));
    }

    // ONE action: dispatch Received.
    let _ = state
        .api
        .dispatch_action(
            "WebhookEvents",
            &event_id,
            "OpenPaw.Ingest.Received",
            json!({
                "raw_payload": body,
                "raw_headers": headers_json,
                "route_key": route_key,
            }),
        )
        .await
        .map_err(|e| {
            eprintln!("  [webhook] dispatch Received failed for {event_id}: {e}");
            // Entity was created; WASM will handle error state.
            // Don't fail the HTTP response — the event exists for audit.
        });

    Ok(Json(json!({
        "event_id": event_id,
        "status": "received",
    })))
}

/// Serialize HTTP headers to a JSON string.
fn serialize_headers(headers: &HeaderMap) -> String {
    let map: serde_json::Map<String, Value> = headers
        .iter()
        .map(|(k, v)| {
            (
                k.as_str().to_string(),
                Value::String(v.to_str().unwrap_or("").to_string()),
            )
        })
        .collect();
    serde_json::to_string(&Value::Object(map)).unwrap_or_else(|_| "{}".to_string())
}
