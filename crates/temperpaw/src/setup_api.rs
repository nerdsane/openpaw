//! REST API for Temper Paw setup, secrets, transport management, and agent creation.
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

use crate::setup::{
    SetupRequestAuth, default_paw_soul_content, has_local_personalized_paw_soul,
    load_paw_soul_content, save_soul_to_temper,
};
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
    pub build_version: String,
    pub build_sha: String,
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
        "temper_api_key",
        "llm_provider",
        "llm_model",
        "sandbox_provider",
        "modal_token_id",
        "modal_token_secret",
        "modal_bridge_url",
        // DD_* and railway_* are infrastructure config managed via Railway env vars,
        // not dashboard secrets. They're set by `temperpaw deploy` and changed in Railway.
        "railway_project_id",
        "railway_environment_id",
        "railway_otel_service_id",
        "railway_service_id",
    ]
    .into_iter()
    .collect()
}

/// Metadata for a known secret key — used by the dashboard to render templates.
#[derive(Serialize)]
struct SecretSchema {
    key: &'static str,
    category: &'static str,
    label: &'static str,
    required: bool,
    description: &'static str,
}

fn secrets_schema() -> Vec<SecretSchema> {
    vec![
        SecretSchema {
            key: "anthropic_api_key",
            category: "llm",
            label: "Anthropic API Key",
            required: false,
            description: "Claude models — console.anthropic.com",
        },
        SecretSchema {
            key: "openai_api_key",
            category: "llm",
            label: "OpenAI API Key",
            required: false,
            description: "GPT models — platform.openai.com/api-keys",
        },
        SecretSchema {
            key: "openai_codex_token",
            category: "llm",
            label: "OpenAI Codex Token",
            required: false,
            description: "OAuth token from codex login (~/.codex/auth.json)",
        },
        SecretSchema {
            key: "openrouter_api_key",
            category: "llm",
            label: "OpenRouter API Key",
            required: false,
            description: "Multi-provider routing — openrouter.ai/keys",
        },
        SecretSchema {
            key: "llm_provider",
            category: "llm",
            label: "Active LLM Provider",
            required: false,
            description: "anthropic, openai, openai_codex, or openrouter",
        },
        SecretSchema {
            key: "llm_model",
            category: "llm",
            label: "LLM Model",
            required: false,
            description: "Override default model (e.g. claude-sonnet-4-6, o3-mini)",
        },
        SecretSchema {
            key: "discord_bot_token",
            category: "messaging",
            label: "Discord Bot Token",
            required: false,
            description: "Bot token from Discord developer portal",
        },
        SecretSchema {
            key: "discord_public_key",
            category: "messaging",
            label: "Discord Public Key",
            required: false,
            description: "Optional override; auto-fetched from the bot token when possible",
        },
        SecretSchema {
            key: "discord_guild_id",
            category: "messaging",
            label: "Discord Guild ID",
            required: false,
            description: "Server ID for slash commands",
        },
        SecretSchema {
            key: "discord_feed_channel_id",
            category: "messaging",
            label: "Discord Feed Channel",
            required: false,
            description: "Channel for activity feed",
        },
        SecretSchema {
            key: "discord_forum_channel_id",
            category: "messaging",
            label: "Discord Forum Channel",
            required: false,
            description: "Forum channel for agent threads",
        },
        SecretSchema {
            key: "slack_app_token",
            category: "messaging",
            label: "Slack App Token",
            required: false,
            description: "xapp-... token for Socket Mode",
        },
        SecretSchema {
            key: "slack_bot_token",
            category: "messaging",
            label: "Slack Bot Token",
            required: false,
            description: "xoxb-... token for Web API",
        },
        SecretSchema {
            key: "slack_signing_secret",
            category: "messaging",
            label: "Slack Signing Secret",
            required: false,
            description: "Webhook signature verification",
        },
        SecretSchema {
            key: "exa_api_key",
            category: "web_search",
            label: "Exa API Key",
            required: false,
            description: "Web search via exa.ai — agents can research the internet",
        },
        SecretSchema {
            key: "sandbox_provider",
            category: "sandbox",
            label: "Sandbox Provider",
            required: false,
            description: "tensorlake or modal — where agents run code",
        },
        SecretSchema {
            key: "tensorlake_api_key",
            category: "sandbox",
            label: "TensorLake API Key",
            required: false,
            description: "Cloud sandbox provisioning — tensorlake.ai",
        },
        SecretSchema {
            key: "modal_token_id",
            category: "sandbox",
            label: "Modal Token ID",
            required: false,
            description: "Starts with ak-… — from modal.com/settings or `modal token set`",
        },
        SecretSchema {
            key: "modal_token_secret",
            category: "sandbox",
            label: "Modal Token Secret",
            required: false,
            description: "Starts with as-… — from modal.com/settings or `modal token set`",
        },
        SecretSchema {
            key: "github_token",
            category: "integrations",
            label: "GitHub Token",
            required: false,
            description: "For repo cloning and PR flows",
        },
        // DD_* keys are infrastructure config set via Railway env vars (by `temperpaw deploy`).
        // They don't belong in the dashboard — change them in Railway if needed.
    ]
}

/// Build the `/paw/` router.
pub fn router(state: SetupApiState) -> Router {
    Router::new()
        .route("/discord/interaction", post(proxy_discord_interaction))
        .route("/paw/setup/status", get(get_setup_status))
        .route("/paw/setup/secrets", get(list_secrets))
        .route("/paw/setup/secrets/schema", get(get_secrets_schema))
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
        .route("/paw/infra/railway/status", get(get_railway_status))
        .route("/paw/infra/railway/set-var", post(set_railway_var))
        .route("/paw/infra/railway/redeploy", post(railway_redeploy))
        .route("/paw/version", get(get_version))
        .route("/paw/infra/updates", get(check_for_updates))
        .route("/paw/infra/edge", get(check_edge_build))
        .with_state(state)
}

// ──────────────────────────────── Setup Status ────────────────────────────────

#[derive(Serialize)]
struct SetupStatus {
    has_anthropic_key: bool,
    llm_provider: Option<String>,
    has_discord: bool,
    has_slack: bool,
    has_agents: bool,
    agent_count: usize,
    has_personalized_soul: bool,
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
    let llm_provider = vault.and_then(|v| v.get_secret(&state.tenant, "llm_provider"));
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
    let has_personalized_soul = has_personalized_paw_soul(&state).await;

    Json(SetupStatus {
        has_anthropic_key,
        llm_provider,
        has_discord,
        has_slack,
        has_agents: agent_count > 0,
        agent_count,
        has_personalized_soul,
        discord_connected,
        slack_connected,
        discord_interaction_url: state
            .transport_manager
            .discord_interaction_public_url()
            .await,
    })
}

fn personalized_soul_flag_value(value: Option<&str>) -> bool {
    value.is_some_and(|value| matches!(value.trim(), "1" | "true" | "yes"))
}

fn persisted_personalized_soul_flag(state: &SetupApiState) -> bool {
    personalized_soul_flag_value(
        state
            .platform
            .server
            .secrets_vault
            .as_ref()
            .and_then(|vault| vault.get_secret(&state.tenant, "paw_personalized_soul"))
            .as_deref(),
    )
}

async fn has_personalized_paw_soul(state: &SetupApiState) -> bool {
    if persisted_personalized_soul_flag(state) || has_local_personalized_paw_soul() {
        return true;
    }

    let Ok(default_content) = default_paw_soul_content() else {
        return false;
    };
    let Ok((_, current_content)) = load_current_paw_soul(state, &SetupRequestAuth::default()).await
    else {
        return false;
    };

    current_content.trim() != default_content.trim()
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
    let keys = state
        .platform
        .server
        .secrets_vault
        .as_ref()
        .map(|vault| vault.list_keys(&state.tenant))
        .unwrap_or_default();
    Json(SecretKeyList { keys })
}

async fn get_secrets_schema() -> Json<Vec<SecretSchema>> {
    Json(secrets_schema())
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

    if key == "temper_api_key" {
        return match state.platform.api_token.clone() {
            Some(value) => (
                StatusCode::OK,
                Json(serde_json::to_value(SecretValueResponse { key, value }).unwrap()),
            ),
            None => (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "error": "Secret not found" })),
            ),
        };
    }

    let Some(vault) = state.platform.server.secrets_vault.as_ref() else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": "Vault not initialized" })),
        );
    };

    match vault.get_secret(&state.tenant, &key) {
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

fn discord_connect_params_for_secret_update<F>(
    get_secret: F,
    updated_key: &str,
    updated_value: &str,
) -> Option<DiscordConnectParams>
where
    F: Fn(&str) -> Option<String>,
{
    fn effective_secret<F>(
        get_secret: &F,
        updated_key: &str,
        updated_value: &str,
        key: &str,
    ) -> Option<String>
    where
        F: Fn(&str) -> Option<String>,
    {
        let value = if key == updated_key {
            Some(updated_value.to_string())
        } else {
            get_secret(key)
        }?;

        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    }

    if !matches!(
        updated_key,
        "discord_bot_token"
            | "discord_public_key"
            | "discord_guild_id"
            | "discord_feed_channel_id"
            | "discord_forum_channel_id"
    ) {
        return None;
    }

    Some(DiscordConnectParams {
        bot_token: effective_secret(&get_secret, updated_key, updated_value, "discord_bot_token")?,
        public_key: effective_secret(
            &get_secret,
            updated_key,
            updated_value,
            "discord_public_key",
        ),
        guild_id: effective_secret(&get_secret, updated_key, updated_value, "discord_guild_id"),
        feed_channel_id: effective_secret(
            &get_secret,
            updated_key,
            updated_value,
            "discord_feed_channel_id",
        ),
        forum_channel_id: effective_secret(
            &get_secret,
            updated_key,
            updated_value,
            "discord_forum_channel_id",
        ),
    })
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

    let discord_params = discord_connect_params_for_secret_update(
        |key| {
            vault
                .get_secret(&state.tenant, key)
                .or_else(|| vault.get_secret("default", key))
        },
        &req.key,
        &req.value,
    );

    if let Some(params) = discord_params {
        if let Err(error) = state.transport_manager.connect_discord(params).await {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": format!("Saved value would not produce a working Discord connection: {error}")
                })),
            );
        }
    }

    // Cache in memory + persist to Turso
    let _ = vault.cache_secret(&state.tenant, &req.key, req.value.clone());

    if let Ok((ciphertext, nonce)) = vault.encrypt(req.value.as_bytes()) {
        let _ = state
            .turso_store
            .upsert_secret(&state.tenant, &req.key, &ciphertext, &nonce)
            .await;
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
    }
    let _ = state.turso_store.delete_secret(&state.tenant, &key).await;

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
        Ok(()) => {
            if let Some(vault) = state.platform.server.secrets_vault.as_ref() {
                let _ =
                    vault.cache_secret(&state.tenant, "paw_personalized_soul", "true".to_string());
                if let Ok((ciphertext, nonce)) = vault.encrypt(b"true") {
                    let _ = state
                        .turso_store
                        .upsert_secret(&state.tenant, "paw_personalized_soul", &ciphertext, &nonce)
                        .await;
                }
            }

            (StatusCode::OK, Json(serde_json::json!({ "saved": true }))).into_response()
        }
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
        .unwrap_or_else(|| "anthropic".to_string());

    let api_key = [
        "anthropic_api_key",
        "openrouter_api_key",
        "openai_api_key",
        "openai_codex_token",
    ]
    .into_iter()
    .find_map(|key| vault.get_secret(&state.tenant, key))
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

    // Dispatch Configure action — resolve provider/model from vault
    let vault = state.platform.server.secrets_vault.as_ref();
    let resolved_provider = vault
        .and_then(|v| v.get_secret(&state.tenant, "llm_provider"))
        .unwrap_or_else(|| "anthropic".to_string());
    let default_model = match resolved_provider.as_str() {
        "openai" | "openai_codex" => "gpt-5.4".to_string(),
        "openrouter" => "anthropic/claude-sonnet-4.6".to_string(),
        _ => "claude-sonnet-4-6".to_string(),
    };
    let configure_params = serde_json::json!({
        "model": req.model.unwrap_or(default_model),
        "provider": resolved_provider,
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

// ──────────────────────────── Railway Integration ────────────────────────────

#[derive(Serialize)]
struct RailwayStatus {
    configured: bool,
    can_update: bool,
    project_id: Option<String>,
    environment_id: Option<String>,
    service_id: Option<String>,
    otel_service_id: Option<String>,
}

async fn get_railway_status(State(state): State<SetupApiState>) -> Json<RailwayStatus> {
    let vault = state.platform.server.secrets_vault.as_ref();
    let has_token = vault
        .and_then(|v| v.get_secret(&state.tenant, "railway_token"))
        .is_some();
    let project_id = vault.and_then(|v| v.get_secret(&state.tenant, "railway_project_id"));
    let environment_id = vault.and_then(|v| v.get_secret(&state.tenant, "railway_environment_id"));
    let service_id = vault.and_then(|v| v.get_secret(&state.tenant, "railway_service_id"));
    let otel_service_id =
        vault.and_then(|v| v.get_secret(&state.tenant, "railway_otel_service_id"));
    let configured = has_token && project_id.is_some() && environment_id.is_some();
    let can_update = configured && service_id.is_some();
    Json(RailwayStatus {
        configured,
        can_update,
        project_id,
        environment_id,
        service_id,
        otel_service_id,
    })
}

#[derive(Deserialize)]
struct SetRailwayVarRequest {
    service: String,
    key: String,
    value: String,
}

/// Allowlist of (service, key) pairs that can be set via this endpoint.
fn allowed_railway_vars() -> Vec<(&'static str, &'static str)> {
    vec![
        ("otel-collector", "DD_API_KEY"),
        ("otel-collector", "DD_SITE"),
    ]
}

async fn set_railway_var(
    State(state): State<SetupApiState>,
    Json(req): Json<SetRailwayVarRequest>,
) -> impl IntoResponse {
    // Validate against allowlist
    if !allowed_railway_vars()
        .iter()
        .any(|(s, k)| *s == req.service && *k == req.key)
    {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": format!("Setting {} on {} is not allowed", req.key, req.service)
            })),
        )
            .into_response();
    }

    let vault = match state.platform.server.secrets_vault.as_ref() {
        Some(v) => v,
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Vault not initialized" })),
            )
                .into_response();
        }
    };

    let railway_token = vault.get_secret(&state.tenant, "railway_token");
    let project_id = vault.get_secret(&state.tenant, "railway_project_id");
    let environment_id = vault.get_secret(&state.tenant, "railway_environment_id");

    let (Some(token), Some(project), Some(env)) = (railway_token, project_id, environment_id)
    else {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "Railway integration not configured. Deploy with `temperpaw deploy` first."
            })),
        )
            .into_response();
    };

    // Resolve service ID — for otel-collector, read from vault
    let service_id = if req.service == "otel-collector" {
        vault.get_secret(&state.tenant, "railway_otel_service_id")
    } else {
        None
    };

    let Some(service_id) = service_id else {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": format!("Service ID for {} not found in vault", req.service)
            })),
        )
            .into_response();
    };

    // Call Railway GraphQL API — variableUpsert
    let graphql_query = serde_json::json!({
        "query": "mutation($input: VariableUpsertInput!) { variableUpsert(input: $input) }",
        "variables": {
            "input": {
                "projectId": project,
                "environmentId": env,
                "serviceId": service_id,
                "name": req.key,
                "value": req.value,
            }
        }
    });

    let client = reqwest::Client::new();
    match client
        .post("https://backboard.railway.com/graphql/v2")
        .header("Authorization", format!("Bearer {token}"))
        .header("Content-Type", "application/json")
        .json(&graphql_query)
        .send()
        .await
    {
        Ok(resp) => {
            let status = resp.status();
            let body: serde_json::Value = resp.json().await.unwrap_or_default();
            if status.is_success() && body.get("errors").is_none() {
                (
                    StatusCode::OK,
                    Json(serde_json::json!({
                        "set": req.key,
                        "service": req.service,
                    })),
                )
                    .into_response()
            } else {
                let error_msg = body["errors"]
                    .as_array()
                    .and_then(|e| e.first())
                    .and_then(|e| e["message"].as_str())
                    .unwrap_or("Railway API error");
                (
                    StatusCode::BAD_GATEWAY,
                    Json(serde_json::json!({ "error": error_msg })),
                )
                    .into_response()
            }
        }
        Err(e) => (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({ "error": format!("Railway API request failed: {e}") })),
        )
            .into_response(),
    }
}

// ────────────────────────────── Version + Updates ─────────────────────────────

#[derive(Serialize)]
struct VersionInfo {
    version: String,
    sha: String,
}

async fn get_version(State(state): State<SetupApiState>) -> Json<VersionInfo> {
    Json(VersionInfo {
        version: state.build_version.clone(),
        sha: state.build_sha.clone(),
    })
}

#[derive(Serialize)]
struct UpdateCheck {
    current_version: String,
    latest_version: Option<String>,
    latest_sha: Option<String>,
    update_available: bool,
    release_url: Option<String>,
    release_notes: Option<String>,
}

async fn check_for_updates(State(state): State<SetupApiState>) -> impl IntoResponse {
    let current = &state.build_version;

    let client = reqwest::Client::builder()
        .user_agent("temperpaw-server")
        .build()
        .unwrap_or_default();

    match client
        .get("https://api.github.com/repos/nerdsane/temperpaw/releases/latest")
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            let body: serde_json::Value = resp.json().await.unwrap_or_default();
            let tag = body["tag_name"].as_str().unwrap_or("").to_string();
            let html_url = body["html_url"].as_str().map(|s| s.to_string());
            let notes = body["body"].as_str().map(|s| {
                // Truncate release notes to first 500 chars for the API response
                if s.len() > 500 {
                    format!("{}...", &s[..500])
                } else {
                    s.to_string()
                }
            });

            // If current is "dev" or "sha-*" (non-release build), any release is an update.
            // If current is a release tag (e.g. "v0.2.0"), compare against latest tag.
            let update_available = if tag.is_empty() {
                false
            } else if current == "dev" || current.starts_with("sha-") || current == "unknown" {
                true
            } else {
                tag != *current
            };

            (
                StatusCode::OK,
                Json(serde_json::json!(UpdateCheck {
                    current_version: current.clone(),
                    latest_version: if tag.is_empty() { None } else { Some(tag) },
                    latest_sha: None,
                    update_available,
                    release_url: html_url,
                    release_notes: notes,
                })),
            )
                .into_response()
        }
        Ok(resp) => {
            let status = resp.status();
            (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({
                    "error": format!("GitHub API returned {status}"),
                    "current_version": current,
                })),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({
                "error": format!("Failed to check GitHub releases: {e}"),
                "current_version": current,
            })),
        )
            .into_response(),
    }
}

#[derive(Serialize)]
struct EdgeBuild {
    available: bool,
    sha: Option<String>,
    short_sha: Option<String>,
    message: Option<String>,
    committed_at: Option<String>,
}

async fn check_edge_build(_state: State<SetupApiState>) -> impl IntoResponse {
    let client = reqwest::Client::builder()
        .user_agent("temperpaw-server")
        .build()
        .unwrap_or_default();

    match client
        .get("https://api.github.com/repos/nerdsane/temperpaw/commits/main")
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            let body: serde_json::Value = resp.json().await.unwrap_or_default();
            let sha = body["sha"].as_str().map(|s| s.to_string());
            let short_sha = sha.as_deref().map(|s| s[..7.min(s.len())].to_string());
            let message = body["commit"]["message"]
                .as_str()
                .map(|s| s.lines().next().unwrap_or(s).to_string());
            let committed_at = body["commit"]["committer"]["date"]
                .as_str()
                .map(|s| s.to_string());

            (
                StatusCode::OK,
                Json(serde_json::json!(EdgeBuild {
                    available: sha.is_some(),
                    sha,
                    short_sha,
                    message,
                    committed_at,
                })),
            )
                .into_response()
        }
        Ok(resp) => {
            let status = resp.status();
            (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({ "error": format!("GitHub API returned {status}") })),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({ "error": format!("Failed to check edge build: {e}") })),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
struct RedeployRequest {
    /// Optional image tag to deploy. "edge" for latest main build, "latest" for latest release.
    /// If omitted, redeploys with the current configuration (no tag change).
    image_tag: Option<String>,
}

async fn railway_redeploy(
    State(state): State<SetupApiState>,
    Json(req): Json<RedeployRequest>,
) -> impl IntoResponse {
    // Only allow known tags to prevent arbitrary image injection
    if let Some(ref tag) = req.image_tag {
        if tag != "latest" && tag != "edge" {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": "image_tag must be 'latest' or 'edge'" })),
            )
                .into_response();
        }
    }

    let vault = match state.platform.server.secrets_vault.as_ref() {
        Some(v) => v,
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": "Vault not initialized" })),
            )
                .into_response();
        }
    };

    let railway_token = vault.get_secret(&state.tenant, "railway_token");
    let project_id = vault.get_secret(&state.tenant, "railway_project_id");
    let environment_id = vault.get_secret(&state.tenant, "railway_environment_id");
    let service_id = vault.get_secret(&state.tenant, "railway_service_id");

    let (Some(token), Some(project), Some(env), Some(svc)) =
        (railway_token, project_id, environment_id, service_id)
    else {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "Railway integration not configured. Set railway_token, railway_project_id, railway_environment_id, and railway_service_id."
            })),
        )
            .into_response();
    };

    let client = reqwest::Client::new();
    let railway_url = "https://backboard.railway.com/graphql/v2";

    // If an image_tag was requested, set it as a Railway variable first.
    // The deploy Dockerfile uses `ARG IMAGE_TAG=latest` so this controls which image is pulled.
    if let Some(ref tag) = req.image_tag {
        let var_query = serde_json::json!({
            "query": "mutation($input: VariableUpsertInput!) { variableUpsert(input: $input) }",
            "variables": {
                "input": {
                    "projectId": project,
                    "environmentId": env,
                    "serviceId": svc,
                    "name": "IMAGE_TAG",
                    "value": tag,
                }
            }
        });

        match client
            .post(railway_url)
            .header("Authorization", format!("Bearer {token}"))
            .header("Content-Type", "application/json")
            .json(&var_query)
            .send()
            .await
        {
            Ok(resp) => {
                let body: serde_json::Value = resp.json().await.unwrap_or_default();
                if body.get("errors").is_some() {
                    let error_msg = body["errors"]
                        .as_array()
                        .and_then(|e| e.first())
                        .and_then(|e| e["message"].as_str())
                        .unwrap_or("Failed to set IMAGE_TAG");
                    return (
                        StatusCode::BAD_GATEWAY,
                        Json(serde_json::json!({ "error": error_msg })),
                    )
                        .into_response();
                }
            }
            Err(e) => {
                return (
                    StatusCode::BAD_GATEWAY,
                    Json(serde_json::json!({ "error": format!("Failed to set IMAGE_TAG: {e}") })),
                )
                    .into_response();
            }
        }
    }

    // Trigger the redeploy
    let redeploy_query = format!(
        "mutation {{ serviceInstanceRedeploy(serviceId: \"{svc}\", environmentId: \"{env}\") }}"
    );

    match client
        .post(railway_url)
        .header("Authorization", format!("Bearer {token}"))
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({ "query": redeploy_query }))
        .send()
        .await
    {
        Ok(resp) => {
            let status = resp.status();
            let body: serde_json::Value = resp.json().await.unwrap_or_default();
            if status.is_success() && body.get("errors").is_none() {
                (
                    StatusCode::OK,
                    Json(serde_json::json!({
                        "triggered": true,
                        "image_tag": req.image_tag.as_deref().unwrap_or("current"),
                    })),
                )
                    .into_response()
            } else {
                let error_msg = body["errors"]
                    .as_array()
                    .and_then(|e| e.first())
                    .and_then(|e| e["message"].as_str())
                    .unwrap_or("Railway API error");
                (
                    StatusCode::BAD_GATEWAY,
                    Json(serde_json::json!({ "error": error_msg })),
                )
                    .into_response()
            }
        }
        Err(e) => (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({ "error": format!("Railway API request failed: {e}") })),
        )
            .into_response(),
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
    let signature = headers
        .get("x-signature-ed25519")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    let timestamp = headers
        .get("x-signature-timestamp")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    let vault = state.platform.server.secrets_vault.as_ref();
    let configured_public_key = vault
        .and_then(|vault| vault.get_secret(&state.tenant, "discord_public_key"))
        .filter(|value| !value.trim().is_empty());

    if !signature.is_empty() && !timestamp.is_empty() {
        let mut verified_public_key = configured_public_key
            .as_deref()
            .filter(|public_key| verify_discord_signature(public_key, signature, timestamp, &body))
            .map(str::to_string);

        if verified_public_key.is_none() {
            let bot_token =
                vault.and_then(|vault| vault.get_secret(&state.tenant, "discord_bot_token"));
            if let (Some(vault), Some(bot_token)) = (vault, bot_token) {
                match resolve_and_persist_discord_public_key(
                    vault,
                    &state.turso_store,
                    &state.tenant,
                    &bot_token,
                    configured_public_key.as_deref(),
                )
                .await
                {
                    Ok(refreshed_public_key)
                        if verify_discord_signature(
                            &refreshed_public_key,
                            signature,
                            timestamp,
                            &body,
                        ) =>
                    {
                        if configured_public_key.as_deref() != Some(refreshed_public_key.as_str()) {
                            tracing::info!(
                                "Healed stale Discord verify_key during public endpoint verification"
                            );
                        }
                        verified_public_key = Some(refreshed_public_key);
                    }
                    Ok(_) | Err(_) => {}
                }
            }
        }

        if verified_public_key.is_none() {
            tracing::warn!("Discord interaction signature verification failed at public endpoint");
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({ "error": "invalid signature" })),
            )
                .into_response();
        }

        if is_discord_ping(&body) {
            tracing::info!("Discord interaction endpoint verification succeeded");
            return (StatusCode::OK, Json(serde_json::json!({ "type": 1 }))).into_response();
        }
    }

    let status = state.transport_manager.status().await;
    if !matches!(
        status.discord,
        crate::transport_manager::TransportStatus::Connected { .. }
            | crate::transport_manager::TransportStatus::Connecting
    ) {
        tracing::warn!("Discord interaction received while transport is unavailable");
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
            if !status.is_success() {
                tracing::warn!(
                    http_status = %status,
                    "Discord interaction proxy returned a non-success status"
                );
            }
            let mut response = axum::http::Response::builder().status(status);
            if let Some(content_type) = content_type {
                response = response.header(axum::http::header::CONTENT_TYPE, content_type);
            }
            response
                .body(Body::from(bytes))
                .unwrap_or_else(|_| axum::http::Response::new(Body::empty()))
        }
        Err(error) => {
            tracing::error!(%error, "Failed to proxy Discord interaction");
            (
                StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({
                    "error": format!("failed to proxy discord interaction: {error}")
                })),
            )
                .into_response()
        }
    }
}

pub(crate) async fn resolve_and_persist_discord_public_key(
    vault: &Arc<temper_server::secrets::SecretsVault>,
    turso_store: &TursoEventStore,
    tenant: &str,
    bot_token: &str,
    configured_public_key: Option<&str>,
) -> Result<String> {
    let public_key =
        crate::discord_app::resolve_verify_key(bot_token, configured_public_key).await?;
    persist_discord_public_key(vault, turso_store, tenant, &public_key).await;
    Ok(public_key)
}

async fn persist_discord_public_key(
    vault: &Arc<temper_server::secrets::SecretsVault>,
    turso_store: &TursoEventStore,
    tenant: &str,
    public_key: &str,
) {
    let _ = vault.cache_secret(tenant, "discord_public_key", public_key.to_string());
    if let Ok((ct, nc)) = vault.encrypt(public_key.as_bytes()) {
        let _ = turso_store
            .upsert_secret(tenant, "discord_public_key", &ct, &nc)
            .await;
    }
}

fn is_discord_ping(body: &[u8]) -> bool {
    #[derive(Deserialize)]
    struct InteractionEnvelope {
        #[serde(rename = "type")]
        interaction_type: u8,
    }

    matches!(
        serde_json::from_slice::<InteractionEnvelope>(body),
        Ok(InteractionEnvelope {
            interaction_type: 1
        })
    )
}

fn verify_discord_signature(
    public_key_hex: &str,
    signature_hex: &str,
    timestamp: &str,
    body: &[u8],
) -> bool {
    use ed25519_dalek::{Signature, Verifier, VerifyingKey};

    let Ok(pk_bytes) = hex::decode(public_key_hex) else {
        return false;
    };
    let pk_bytes: [u8; 32] = match pk_bytes.try_into() {
        Ok(bytes) => bytes,
        Err(_) => return false,
    };
    let Ok(verifying_key) = VerifyingKey::from_bytes(&pk_bytes) else {
        return false;
    };

    let Ok(sig_bytes) = hex::decode(signature_hex) else {
        return false;
    };
    let sig_bytes: [u8; 64] = match sig_bytes.try_into() {
        Ok(bytes) => bytes,
        Err(_) => return false,
    };
    let signature = Signature::from_bytes(&sig_bytes);

    let mut message = Vec::with_capacity(timestamp.len() + body.len());
    message.extend_from_slice(timestamp.as_bytes());
    message.extend_from_slice(body);

    verifying_key.verify(&message, &signature).is_ok()
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
    let Some(vault) = state.platform.server.secrets_vault.as_ref() else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": "secrets vault is not configured"
            })),
        )
            .into_response();
    };
    let resolved_public_key = match resolve_and_persist_discord_public_key(
        vault,
        &state.turso_store,
        &state.tenant,
        &req.bot_token,
        req.public_key.as_deref(),
    )
    .await
    {
        Ok(public_key) => public_key,
        Err(error) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": error.to_string()
                })),
            )
                .into_response();
        }
    };

    match state
        .transport_manager
        .connect_discord(DiscordConnectParams {
            bot_token: req.bot_token.clone(),
            public_key: Some(resolved_public_key.clone()),
            guild_id: req.guild_id.clone(),
            feed_channel_id: req.feed_channel_id.clone(),
            forum_channel_id: req.forum_channel_id.clone(),
        })
        .await
    {
        Ok(interaction_url) => {
            // Save Discord config to vault + Turso only after the endpoint is valid enough to start.
            let _ = vault.cache_secret(&state.tenant, "discord_bot_token", req.bot_token.clone());
            if let Ok((ct, nc)) = vault.encrypt(req.bot_token.as_bytes()) {
                let _ = state
                    .turso_store
                    .upsert_secret(&state.tenant, "discord_bot_token", &ct, &nc)
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

#[cfg(test)]
mod tests {
    use super::{
        allowed_secret_keys, discord_connect_params_for_secret_update, is_discord_ping,
        persist_discord_public_key, personalized_soul_flag_value, secrets_schema,
        verify_discord_signature,
    };
    use ed25519_dalek::{Signer, SigningKey};
    use std::sync::Arc;
    use temper_server::secrets::SecretsVault;
    use temper_store_turso::TursoEventStore;

    #[test]
    fn discord_secret_update_builds_reconnect_params_when_config_is_complete() {
        let params = discord_connect_params_for_secret_update(
            |key| match key {
                "discord_public_key" => Some("pub-key".to_string()),
                "discord_guild_id" => Some("guild-123".to_string()),
                _ => None,
            },
            "discord_bot_token",
            "new-token",
        )
        .expect("discord reconnect params should be built");

        assert_eq!(params.bot_token, "new-token");
        assert_eq!(params.public_key.as_deref(), Some("pub-key"));
        assert_eq!(params.guild_id.as_deref(), Some("guild-123"));
        assert_eq!(params.feed_channel_id, None);
        assert_eq!(params.forum_channel_id, None);
    }

    #[test]
    fn discord_secret_update_skips_reconnect_for_non_discord_secret_updates() {
        let params =
            discord_connect_params_for_secret_update(|_| None, "openai_api_key", "new-token");

        assert!(params.is_none());
    }

    #[test]
    fn discord_secret_update_reconnects_without_a_manual_public_key() {
        let params = discord_connect_params_for_secret_update(
            |key| match key {
                "discord_guild_id" => Some("guild-123".to_string()),
                _ => None,
            },
            "discord_bot_token",
            "new-token",
        )
        .expect("discord reconnect params should be built");

        assert_eq!(params.bot_token, "new-token");
        assert_eq!(params.public_key, None);
        assert_eq!(params.guild_id.as_deref(), Some("guild-123"));
    }

    #[test]
    fn personalized_soul_flag_value_accepts_truthy_values() {
        assert!(personalized_soul_flag_value(Some("true")));
        assert!(personalized_soul_flag_value(Some("1")));
        assert!(personalized_soul_flag_value(Some("yes")));
        assert!(!personalized_soul_flag_value(Some("false")));
        assert!(!personalized_soul_flag_value(None));
    }

    #[test]
    fn discord_signature_verification_accepts_valid_signature() {
        let signing_key = SigningKey::from_bytes(&[7u8; 32]);
        let body = br#"{"type":1}"#;
        let timestamp = "1744848000";
        let mut message = Vec::from(timestamp.as_bytes());
        message.extend_from_slice(body);
        let signature = signing_key.sign(&message);
        let public_key_hex = hex::encode(signing_key.verifying_key().to_bytes());
        let signature_hex = hex::encode(signature.to_bytes());

        assert!(verify_discord_signature(
            &public_key_hex,
            &signature_hex,
            timestamp,
            body,
        ));
    }

    #[test]
    fn discord_ping_detection_identifies_ping_payloads() {
        assert!(is_discord_ping(br#"{"type":1}"#));
        assert!(!is_discord_ping(br#"{"type":3}"#));
        assert!(!is_discord_ping(br#"not-json"#));
    }

    #[tokio::test]
    async fn discord_public_key_persistence_updates_vault_and_turso() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let db_path = tempdir.path().join("setup-api.db");
        let turso_store = TursoEventStore::new(&format!("file:{}", db_path.display()), None)
            .await
            .expect("local turso store");
        let vault = Arc::new(SecretsVault::new(&[9u8; 32]));
        let tenant = "default";
        let refreshed_public_key =
            "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

        persist_discord_public_key(&vault, &turso_store, tenant, refreshed_public_key).await;
        assert_eq!(
            vault.get_secret(tenant, "discord_public_key").as_deref(),
            Some(refreshed_public_key)
        );

        let rows = turso_store
            .load_secrets_for_tenant(tenant)
            .await
            .expect("load tenant secrets");
        let stored_public_key = rows
            .into_iter()
            .find_map(|(key_name, ciphertext, nonce)| {
                (key_name == "discord_public_key").then(|| {
                    vault
                        .decrypt(&ciphertext, &nonce)
                        .expect("decrypt public key")
                })
            })
            .expect("persisted public key");
        assert_eq!(
            String::from_utf8(stored_public_key).expect("utf8 public key"),
            refreshed_public_key
        );
    }

    #[test]
    fn modal_bridge_url_remains_internal_only() {
        assert!(allowed_secret_keys().contains("modal_bridge_url"));
        assert!(
            !secrets_schema()
                .iter()
                .any(|secret| secret.key == "modal_bridge_url"),
            "modal_bridge_url should be provisioned by deploy, not shown in the dashboard schema"
        );
    }
}
