//! REST API for Open Paw setup, secrets, transport management, and agent creation.
//!
//! Mounted at `/paw/` to avoid conflicts with Temper's `/api/` tenant routes.
//! All endpoints are also usable by external agents via HTTP.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use axum::body::{Body, Bytes};
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use temper_platform::PlatformState;
use temper_store_turso::TursoEventStore;

use crate::setup::{SetupRequestAuth, load_paw_soul_content, save_soul_to_temper};
use crate::setup_llm::{
    GeneratedSoul, LlmProvider, UserInterview, generate_personalized_soul, refine_soul,
};
use crate::transport_manager::{DiscordConnectParams, SlackConnectParams, TransportManager};

const DEFAULT_SETUP_AGENT_TOOLS_ENABLED: &str = "temper_create,temper_get,temper_list,temper_action,temper_patch,temper_submit_specs,temper_show_spec,temper_specs,temper_upload_wasm,temper_get_trajectories,temper_get_insights,temper_get_decisions,temper_poll_decision,temper_approve_decision,temper_deny_decision,temper_submit_policy,temper_list_policies,temper_get_policy,temper_update_policy,temper_delete_policy,temper_install_app,temper_list_apps,temper_spawn_session,temper_list_sessions,temper_abort_session,temper_steer_session,temper_save_memory,temper_recall_memory,temper_write,temper_read,temper_run_coding_agent,temper_get_secret,temper_datadog_query,temper_railway,temper_vercel,temper_web_search,temper_web_fetch,read,write,edit,bash";

/// Shared state for the setup API.
#[derive(Clone)]
pub struct SetupApiState {
    pub platform: PlatformState,
    pub turso_store: TursoEventStore,
    pub transport_manager: Arc<TransportManager>,
    pub tenant: String,
    pub agents_dir: PathBuf,
    pub base_url: String,
}

/// Whitelisted secret key names that can be set via the API.
fn allowed_secret_keys() -> HashSet<&'static str> {
    [
        "anthropic_api_key",
        "openai_api_key",
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
        "llm_provider",
        "temper_api_key",
    ]
    .into_iter()
    .collect()
}

/// Build the `/paw/` router.
pub fn router(state: SetupApiState) -> Router {
    Router::new()
        .route("/discord/interaction", post(proxy_discord_interaction))
        .route("/paw/setup/status", get(get_setup_status))
        .route("/paw/setup/secrets", get(list_secrets))
        .route("/paw/setup/secrets", post(upsert_secret))
        .route("/paw/setup/secrets/{key}", get(get_secret))
        .route("/paw/setup/secrets/{key}", delete(delete_secret))
        .route("/paw/setup/soul", get(get_current_soul))
        .route("/paw/setup/soul/generate", post(generate_soul_preview))
        .route("/paw/setup/soul/save", post(save_soul))
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
    discord_interaction_url: Option<String>,
}

async fn get_setup_status(State(state): State<SetupApiState>) -> Json<SetupStatus> {
    let vault = state.platform.server.secrets_vault.as_ref();

    let has_anthropic_key = vault
        .and_then(|v| {
            v.get_secret(&state.tenant, "anthropic_api_key")
                .or_else(|| v.get_secret(&state.tenant, "openai_api_key"))
                .or_else(|| v.get_secret(&state.tenant, "openai_codex_token"))
                .or_else(|| v.get_secret(&state.tenant, "openrouter_api_key"))
        })
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
        discord_interaction_url: state
            .transport_manager
            .discord_interaction_public_url()
            .await,
    })
}

// ──────────────────────────────── Secrets ────────────────────────────────────

#[derive(Serialize)]
struct SecretKeyList {
    keys: Vec<String>,
}

#[derive(Serialize)]
struct SecretValueResponse {
    key: String,
    value: String,
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

async fn get_secret(
    State(state): State<SetupApiState>,
    Path(key): Path<String>,
) -> impl IntoResponse {
    if !allowed_secret_keys().contains(key.as_str()) {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": format!("Unknown secret key: {key}") })),
        );
    }

    let Some(vault) = state.platform.server.secrets_vault.as_ref() else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": "Vault not initialized" })),
        );
    };

    match vault
        .get_secret(&state.tenant, &key)
        .or_else(|| vault.get_secret("default", &key))
    {
        Some(value) => (
            StatusCode::OK,
            Json(serde_json::to_value(SecretValueResponse { key, value }).unwrap()),
        ),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "Secret not found" })),
        ),
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

// ───────────────────────────── Soul Personalization ───────────────────────────

#[derive(Serialize)]
struct CurrentSoulResponse {
    summary: String,
    content: String,
}

#[derive(Deserialize)]
struct GenerateSoulRequest {
    interview: UserInterview,
    previous_summary: Option<String>,
    feedback: Option<String>,
}

async fn get_current_soul(
    State(state): State<SetupApiState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    match load_current_paw_soul(&state, &SetupRequestAuth::from_headers(&headers)).await {
        Ok((summary, content)) => (
            StatusCode::OK,
            Json(serde_json::to_value(CurrentSoulResponse { summary, content }).unwrap()),
        ),
        Err(error) => {
            tracing::warn!(%error, "Could not load current soul");
            (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "error": "Paw soul not found" })),
            )
        }
    }
}

async fn generate_soul_preview(
    State(state): State<SetupApiState>,
    Json(request): Json<GenerateSoulRequest>,
) -> impl IntoResponse {
    match resolve_llm_provider(&state).await {
        Ok(provider) => {
            let generated = if let Some(feedback) = request.feedback.as_deref() {
                refine_soul(
                    &provider,
                    &request.interview,
                    request.previous_summary.as_deref().unwrap_or_default(),
                    feedback,
                )
                .await
            } else {
                generate_personalized_soul(&provider, &request.interview).await
            };

            match generated {
                Ok(soul) => {
                    (StatusCode::OK, Json(serde_json::to_value(soul).unwrap())).into_response()
                }
                Err(error) => {
                    tracing::warn!(%error, "Soul generation failed");
                    (
                        StatusCode::BAD_REQUEST,
                        Json(serde_json::json!({ "error": error.to_string() })),
                    )
                        .into_response()
                }
            }
        }
        Err(error) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": error.to_string() })),
        )
            .into_response(),
    }
}

async fn save_soul(
    State(state): State<SetupApiState>,
    headers: HeaderMap,
    Json(generated): Json<GeneratedSoul>,
) -> impl IntoResponse {
    let client = reqwest::Client::new();
    let auth = SetupRequestAuth::from_headers(&headers);
    match save_soul_to_temper(&client, &state.base_url, &state.tenant, &generated, &auth).await {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({ "saved": true }))).into_response(),
        Err(error) => {
            tracing::warn!(%error, "Saving personalized soul failed");
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": error.to_string() })),
            )
                .into_response()
        }
    }
}

async fn resolve_llm_provider(state: &SetupApiState) -> Result<LlmProvider> {
    let Some(vault) = state.platform.server.secrets_vault.as_ref() else {
        anyhow::bail!("Vault not initialized");
    };

    let provider_hint = vault
        .get_secret(&state.tenant, "llm_provider")
        .or_else(|| vault.get_secret("default", "llm_provider"))
        .unwrap_or_else(|| "anthropic".to_string());

    let api_key = [
        "anthropic_api_key",
        "openrouter_api_key",
        "openai_api_key",
        "openai_codex_token",
    ]
    .into_iter()
    .find_map(|key| {
        vault
            .get_secret(&state.tenant, key)
            .or_else(|| vault.get_secret("default", key))
    })
    .context("Configure an LLM API key before personalizing Paw")?;

    Ok(LlmProvider::detect(&api_key, &provider_hint))
}

async fn load_current_paw_soul(
    state: &SetupApiState,
    auth: &SetupRequestAuth,
) -> Result<(String, String)> {
    let client = reqwest::Client::new();
    load_paw_soul_content(&client, &state.base_url, &state.tenant, auth)
        .await
        .context("Failed to load Paw soul content")
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
        "tools_enabled": req.tools_enabled.unwrap_or_else(|| DEFAULT_SETUP_AGENT_TOOLS_ENABLED.to_string()),
        "workdir": "/workspace",
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

async fn proxy_discord_interaction(
    State(state): State<SetupApiState>,
    headers: axum::http::HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    let status = state.transport_manager.status().await;
    if !matches!(
        status.discord,
        crate::transport_manager::TransportStatus::Connected { .. }
            | crate::transport_manager::TransportStatus::Connecting
    ) {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({ "error": "discord transport is not running" })),
        )
            .into_response();
    }

    let client = reqwest::Client::new();
    let mut request = client
        .post(state.transport_manager.discord_interaction_proxy_url())
        .body(body.to_vec());

    for header_name in [
        "content-type",
        "x-signature-ed25519",
        "x-signature-timestamp",
    ] {
        if let Some(value) = headers.get(header_name) {
            request = request.header(header_name, value.clone());
        }
    }

    match request.send().await {
        Ok(resp) => {
            let status = resp.status();
            let content_type = resp
                .headers()
                .get(axum::http::header::CONTENT_TYPE)
                .cloned();
            let bytes = resp.bytes().await.unwrap_or_default();
            let mut response = axum::http::Response::builder().status(status);
            if let Some(content_type) = content_type {
                response = response.header(axum::http::header::CONTENT_TYPE, content_type);
            }
            response
                .body(Body::from(bytes))
                .unwrap_or_else(|_| axum::http::Response::new(Body::empty()))
        }
        Err(error) => (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({
                "error": format!("failed to proxy discord interaction: {error}")
            })),
        )
            .into_response(),
    }
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
    let public_key = req.public_key.unwrap_or_default();
    if public_key.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "discord public key is required to verify interactions"
            })),
        )
            .into_response();
    }

    match state
        .transport_manager
        .connect_discord(DiscordConnectParams {
            bot_token: req.bot_token.clone(),
            public_key: public_key.clone(),
            guild_id: req.guild_id.clone(),
            feed_channel_id: req.feed_channel_id.clone(),
            forum_channel_id: req.forum_channel_id.clone(),
        })
        .await
    {
        Ok(interaction_url) => {
            // Save Discord config to vault + Turso only after the endpoint is valid enough to start.
            if let Some(vault) = state.platform.server.secrets_vault.as_ref() {
                let _ =
                    vault.cache_secret(&state.tenant, "discord_bot_token", req.bot_token.clone());
                if let Ok((ct, nc)) = vault.encrypt(req.bot_token.as_bytes()) {
                    let _ = state
                        .turso_store
                        .upsert_secret(&state.tenant, "discord_bot_token", &ct, &nc)
                        .await;
                }

                let _ = vault.cache_secret(&state.tenant, "discord_public_key", public_key.clone());
                if let Ok((ct, nc)) = vault.encrypt(public_key.as_bytes()) {
                    let _ = state
                        .turso_store
                        .upsert_secret(&state.tenant, "discord_public_key", &ct, &nc)
                        .await;
                }

                for (key, value) in [
                    ("discord_guild_id", req.guild_id.clone()),
                    ("discord_feed_channel_id", req.feed_channel_id.clone()),
                    ("discord_forum_channel_id", req.forum_channel_id.clone()),
                ] {
                    if let Some(value) = value {
                        let _ = vault.cache_secret(&state.tenant, key, value.clone());
                        if let Ok((ct, nc)) = vault.encrypt(value.as_bytes()) {
                            let _ = state
                                .turso_store
                                .upsert_secret(&state.tenant, key, &ct, &nc)
                                .await;
                        }
                    }
                }
            }

            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "status": "connecting",
                    "discord_interaction_url": interaction_url
                })),
            )
                .into_response()
        }
        Err(error) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": error.to_string()
            })),
        )
            .into_response(),
    }
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
