//! REST API for Open Paw setup, secrets, transport management, and agent creation.
//!
//! Mounted at `/paw/` to avoid conflicts with Temper's `/api/` tenant routes.
//! All endpoints are also usable by external agents via HTTP.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use temper_platform::PlatformState;
use temper_store_turso::TursoEventStore;

use crate::transport_manager::{DiscordConnectParams, SlackConnectParams, TransportManager};

/// Shared state for the setup API.
#[derive(Clone)]
pub struct SetupApiState {
    pub platform: PlatformState,
    pub turso_store: TursoEventStore,
    pub transport_manager: Arc<TransportManager>,
    pub tenant: String,
    pub agents_dir: PathBuf,
}

/// Whitelisted secret key names that can be set via the API.
fn allowed_secret_keys() -> HashSet<&'static str> {
    [
        "anthropic_api_key",
        "openai_codex_token",
        "openrouter_api_key",
        "discord_bot_token",
        "discord_public_key",
        "discord_guild_id",
        "discord_feed_channel_id",
        "discord_forum_channel_id",
        "slack_app_token",
        "slack_bot_token",
        "slack_signing_secret",
        "github_token",
        "exa_api_key",
        "tensorlake_api_key",
    ]
    .into_iter()
    .collect()
}

/// Build the `/paw/` router.
pub fn router(state: SetupApiState) -> Router {
    Router::new()
        .route("/paw/setup/status", get(get_setup_status))
        .route("/paw/setup/secrets", get(list_secrets))
        .route("/paw/setup/secrets", post(upsert_secret))
        .route("/paw/setup/secrets/{key}", delete(delete_secret))
        .route("/paw/souls/templates", get(list_soul_templates))
        .route("/paw/agents/create", post(create_agent))
        .route("/paw/transports/status", get(get_transport_status))
        .route("/paw/transports/discord/connect", post(connect_discord))
        .route(
            "/paw/transports/discord/disconnect",
            post(disconnect_discord),
        )
        .route("/paw/transports/slack/connect", post(connect_slack))
        .route("/paw/transports/slack/disconnect", post(disconnect_slack))
        .with_state(state)
}

// ──────────────────────────────── Setup Status ────────────────────────────────

#[derive(Serialize)]
struct SetupStatus {
    has_anthropic_key: bool,
    has_discord: bool,
    has_slack: bool,
    has_agents: bool,
    agent_count: usize,
    discord_connected: bool,
    slack_connected: bool,
}

async fn get_setup_status(State(state): State<SetupApiState>) -> Json<SetupStatus> {
    let vault = state.platform.server.secrets_vault.as_ref();

    let has_anthropic_key = vault
        .and_then(|v| v.get_secret(&state.tenant, "anthropic_api_key"))
        .is_some();
    let has_discord = vault
        .and_then(|v| v.get_secret(&state.tenant, "discord_bot_token"))
        .is_some();
    let has_slack = vault
        .and_then(|v| v.get_secret(&state.tenant, "slack_bot_token"))
        .is_some();

    // Count agents from entity index
    let agent_count = {
        let index = state.platform.server.entity_index.read().unwrap();
        let key = format!("{}:Agent", state.tenant);
        index.get(&key).map(|set| set.len()).unwrap_or(0)
    };

    let transport_status = state.transport_manager.status().await;
    let discord_connected = matches!(
        transport_status.discord,
        crate::transport_manager::TransportStatus::Connected { .. }
    );
    let slack_connected = matches!(
        transport_status.slack,
        crate::transport_manager::TransportStatus::Connected { .. }
    );

    Json(SetupStatus {
        has_anthropic_key,
        has_discord,
        has_slack,
        has_agents: agent_count > 0,
        agent_count,
        discord_connected,
        slack_connected,
    })
}

// ──────────────────────────────── Secrets ────────────────────────────────────

#[derive(Serialize)]
struct SecretKeyList {
    keys: Vec<String>,
}

async fn list_secrets(State(state): State<SetupApiState>) -> Json<SecretKeyList> {
    match state
        .turso_store
        .load_secrets_for_tenant(&state.tenant)
        .await
    {
        Ok(rows) => {
            let keys: Vec<String> = rows.into_iter().map(|(name, _, _)| name).collect();
            Json(SecretKeyList { keys })
        }
        Err(_) => Json(SecretKeyList { keys: vec![] }),
    }
}

#[derive(Deserialize)]
struct UpsertSecretRequest {
    key: String,
    value: String,
}

async fn upsert_secret(
    State(state): State<SetupApiState>,
    Json(req): Json<UpsertSecretRequest>,
) -> impl IntoResponse {
    if !allowed_secret_keys().contains(req.key.as_str()) {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": format!("Unknown secret key: {}", req.key) })),
        );
    }

    let Some(vault) = state.platform.server.secrets_vault.as_ref() else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": "Vault not initialized" })),
        );
    };

    // Cache in memory + persist to Turso
    let _ = vault.cache_secret(&state.tenant, &req.key, req.value.clone());
    if state.tenant != "default" {
        let _ = vault.cache_secret("default", &req.key, req.value.clone());
    }

    if let Ok((ciphertext, nonce)) = vault.encrypt(req.value.as_bytes()) {
        let _ = state
            .turso_store
            .upsert_secret(&state.tenant, &req.key, &ciphertext, &nonce)
            .await;
        if state.tenant != "default" {
            let _ = state
                .turso_store
                .upsert_secret("default", &req.key, &ciphertext, &nonce)
                .await;
        }
    }

    (
        StatusCode::OK,
        Json(serde_json::json!({ "saved": req.key })),
    )
}

async fn delete_secret(
    State(state): State<SetupApiState>,
    Path(key): Path<String>,
) -> impl IntoResponse {
    if let Some(vault) = state.platform.server.secrets_vault.as_ref() {
        vault.remove_secret(&state.tenant, &key);
        if state.tenant != "default" {
            vault.remove_secret("default", &key);
        }
    }
    let _ = state.turso_store.delete_secret(&state.tenant, &key).await;
    if state.tenant != "default" {
        let _ = state.turso_store.delete_secret("default", &key).await;
    }

    (StatusCode::OK, Json(serde_json::json!({ "deleted": key })))
}

// ──────────────────────────────── Soul Templates ─────────────────────────────

#[derive(Serialize)]
struct SoulTemplate {
    name: String,
    description: String,
    path: String,
}

#[derive(Serialize)]
struct SoulTemplateList {
    templates: Vec<SoulTemplate>,
}

async fn list_soul_templates(State(state): State<SetupApiState>) -> Json<SoulTemplateList> {
    let mut templates = Vec::new();

    // Scan os-apps/paw-agent/agents/ for agent templates (new app structure).
    // Each subdirectory is an agent with AGENT.md (operations manual) and optionally SOUL.md.
    if let Ok(entries) = std::fs::read_dir(&state.agents_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();

            // Prefer AGENT.md for description, fall back to SOUL.md
            let desc_file = if path.join("AGENT.md").exists() {
                path.join("AGENT.md")
            } else if path.join("SOUL.md").exists() {
                path.join("SOUL.md")
            } else {
                continue;
            };

            let desc = read_first_line(&desc_file).unwrap_or_default();
            templates.push(SoulTemplate {
                name,
                description: desc,
                path: desc_file.to_string_lossy().to_string(),
            });
        }
    }

    templates.sort_by(|a, b| a.name.cmp(&b.name));
    Json(SoulTemplateList { templates })
}

fn read_first_line(path: &std::path::Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    content
        .lines()
        .find(|line| !line.trim().is_empty() && !line.starts_with('#'))
        .map(|s| s.trim().to_string())
}

// ──────────────────────────────── Agent Creation ─────────────────────────────

#[derive(Deserialize)]
struct CreateAgentRequest {
    name: String,
    role: Option<String>,
    _soul_template: Option<String>,
    model: Option<String>,
    tools_enabled: Option<String>,
    max_turns: Option<String>,
}

async fn create_agent(
    State(state): State<SetupApiState>,
    Json(req): Json<CreateAgentRequest>,
) -> impl IntoResponse {
    let tenant_id = temper_runtime::tenant::TenantId::new(&state.tenant);
    let agent_id = format!("agent-{}", uuid::Uuid::new_v4());

    // Create Agent entity
    let fields = serde_json::json!({
        "name": req.name,
        "role": req.role.unwrap_or_default(),
    });

    match state
        .platform
        .server
        .get_or_create_tenant_entity(&tenant_id, "Agent", &agent_id, fields)
        .await
    {
        Ok(_) => {}
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": format!("Failed to create agent: {e}") })),
            );
        }
    }

    // Dispatch Configure action
    let configure_params = serde_json::json!({
        "model": req.model.unwrap_or_else(|| "claude-sonnet-4-6".to_string()),
        "provider": "anthropic",
        "tools_enabled": req.tools_enabled.unwrap_or_else(|| "read,write,edit,bash".to_string()),
        "max_turns": req.max_turns.unwrap_or_else(|| "20".to_string()),
    });

    match state
        .platform
        .server
        .dispatch_tenant_action(
            &tenant_id,
            "Agent",
            &agent_id,
            "Configure",
            configure_params,
            &temper_server::request_context::AgentContext::system(),
        )
        .await
    {
        Ok(_) => (
            StatusCode::CREATED,
            Json(serde_json::json!({
                "agent_id": agent_id,
                "name": req.name,
                "status": "Active"
            })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": format!("Failed to configure agent: {e}") })),
        ),
    }
}

// ──────────────────────────────── Transports ─────────────────────────────────

async fn get_transport_status(
    State(state): State<SetupApiState>,
) -> Json<crate::transport_manager::AllTransportStatus> {
    Json(state.transport_manager.status().await)
}

#[derive(Deserialize)]
struct DiscordConnectRequest {
    bot_token: String,
    public_key: Option<String>,
    guild_id: Option<String>,
    feed_channel_id: Option<String>,
    forum_channel_id: Option<String>,
}

async fn connect_discord(
    State(state): State<SetupApiState>,
    Json(req): Json<DiscordConnectRequest>,
) -> impl IntoResponse {
    // Save token to vault + Turso
    if let Some(vault) = state.platform.server.secrets_vault.as_ref() {
        let _ = vault.cache_secret(&state.tenant, "discord_bot_token", req.bot_token.clone());
        if let Ok((ct, nc)) = vault.encrypt(req.bot_token.as_bytes()) {
            let _ = state
                .turso_store
                .upsert_secret(&state.tenant, "discord_bot_token", &ct, &nc)
                .await;
        }
    }

    state
        .transport_manager
        .connect_discord(DiscordConnectParams {
            bot_token: req.bot_token,
            public_key: req.public_key.unwrap_or_default(),
            guild_id: req.guild_id,
            feed_channel_id: req.feed_channel_id,
            forum_channel_id: req.forum_channel_id,
        })
        .await;

    Json(serde_json::json!({ "status": "connecting" }))
}

async fn disconnect_discord(State(state): State<SetupApiState>) -> Json<serde_json::Value> {
    state.transport_manager.disconnect_discord().await;
    Json(serde_json::json!({ "status": "disconnected" }))
}

#[derive(Deserialize)]
struct SlackConnectRequest {
    app_token: String,
    bot_token: String,
    signing_secret: Option<String>,
}

async fn connect_slack(
    State(state): State<SetupApiState>,
    Json(req): Json<SlackConnectRequest>,
) -> impl IntoResponse {
    // Save tokens to vault + Turso
    if let Some(vault) = state.platform.server.secrets_vault.as_ref() {
        let _ = vault.cache_secret(&state.tenant, "slack_bot_token", req.bot_token.clone());
        let _ = vault.cache_secret(&state.tenant, "slack_app_token", req.app_token.clone());
        if let Ok((ct, nc)) = vault.encrypt(req.bot_token.as_bytes()) {
            let _ = state
                .turso_store
                .upsert_secret(&state.tenant, "slack_bot_token", &ct, &nc)
                .await;
        }
        if let Ok((ct, nc)) = vault.encrypt(req.app_token.as_bytes()) {
            let _ = state
                .turso_store
                .upsert_secret(&state.tenant, "slack_app_token", &ct, &nc)
                .await;
        }
    }

    state
        .transport_manager
        .connect_slack(SlackConnectParams {
            app_token: req.app_token,
            bot_token: req.bot_token,
            signing_secret: req.signing_secret.unwrap_or_default(),
        })
        .await;

    Json(serde_json::json!({ "status": "connecting" }))
}

async fn disconnect_slack(State(state): State<SetupApiState>) -> Json<serde_json::Value> {
    state.transport_manager.disconnect_slack().await;
    Json(serde_json::json!({ "status": "disconnected" }))
}
