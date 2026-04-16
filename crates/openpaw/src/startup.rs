//! Open Paw 9-phase startup sequence.
//!
//! Replicates the Temper CLI's boot flow (`temper serve`) in an embedded context.
//! The daemon boots the Temper platform, installs Paw OS apps, seeds souls,
//! and starts the Discord transport.

use std::collections::HashSet;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use temper_platform::PlatformState;
use temper_platform::os_apps::{get_os_app, list_startup_os_apps};
use temper_platform::recovery::{recover_cedar_policies, restore_installed_skills};
use temper_platform::router::build_platform_router;
use temper_runtime::scheduler::sim_now;
use temper_runtime::tenant::TenantId;
use temper_server::event_store::ServerEventStore;
use temper_server::registry::{EntityLevelSummary, EntityVerificationResult, VerificationStatus};
use temper_server::registry_bootstrap::restore_registry_from_turso;
use temper_store_turso::{TursoEventStore, TursoSpecVerificationUpdate};
use tokio::task::JoinHandle;

use crate::config::Config;

const DEFAULT_AGENT_TOOLS_ENABLED: &str = "temper_create,temper_get,temper_list,temper_action,temper_patch,temper_submit_specs,temper_show_spec,temper_specs,temper_upload_wasm,temper_get_trajectories,temper_get_insights,temper_get_decisions,temper_poll_decision,temper_approve_decision,temper_deny_decision,temper_submit_policy,temper_list_policies,temper_get_policy,temper_update_policy,temper_delete_policy,temper_install_app,temper_list_apps,temper_spawn_session,temper_list_sessions,temper_abort_session,temper_steer_session,temper_save_memory,temper_recall_memory,temper_write,temper_read,temper_run_coding_agent,temper_get_secret,temper_datadog_query,temper_railway,temper_vercel,temper_web_search,temper_web_fetch,read,write,edit,bash";
const DEFAULT_AGENT_WORKDIR: &str = "/workspace";

#[derive(Debug, Clone, PartialEq, Eq)]
enum RuntimeRecoveryStep {
    PopulateIndex(String),
    PopulateFieldIndex(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LocalWasmStartupPolicy {
    BuildIfMissing,
    LoadPersistedOnly,
}

fn local_wasm_startup_policy(raw: Option<&str>) -> LocalWasmStartupPolicy {
    match raw.map(|value| value.trim().to_ascii_lowercase()) {
        Some(value) if matches!(value.as_str(), "build" | "build-if-missing" | "true" | "1") => {
            LocalWasmStartupPolicy::BuildIfMissing
        }
        Some(value) if matches!(value.as_str(), "load-only" | "persisted" | "false" | "0") => {
            LocalWasmStartupPolicy::LoadPersistedOnly
        }
        _ => LocalWasmStartupPolicy::LoadPersistedOnly,
    }
}

fn runtime_recovery_plan(tenant_ids: &[TenantId]) -> Vec<RuntimeRecoveryStep> {
    let mut steps = Vec::with_capacity(tenant_ids.len() * 2);
    for tenant_id in tenant_ids {
        steps.push(RuntimeRecoveryStep::PopulateIndex(
            tenant_id.as_str().to_string(),
        ));
    }
    for tenant_id in tenant_ids {
        steps.push(RuntimeRecoveryStep::PopulateFieldIndex(
            tenant_id.as_str().to_string(),
        ));
    }
    steps
}

fn startup_os_apps() -> Vec<String> {
    list_startup_os_apps()
}

fn startup_discord_connect_result(result: anyhow::Result<String>) -> Option<String> {
    match result {
        Ok(interaction_url) => Some(interaction_url),
        Err(error) => {
            tracing::error!(
                error = %error,
                "Discord transport failed during startup; continuing without Discord"
            );
            None
        }
    }
}

fn spawn_runtime_server(
    listener: tokio::net::TcpListener,
    router: axum::Router,
) -> JoinHandle<std::io::Result<()>> {
    tokio::spawn(async move { axum::serve(listener, router).await })
}

async fn wait_for_runtime_server(url: &str, timeout: Duration) -> Result<()> {
    let deadline = Instant::now() + timeout;
    let client = reqwest::Client::new();

    loop {
        match client.get(url).send().await {
            Ok(response) if response.status().is_success() => return Ok(()),
            Ok(_) | Err(_) if Instant::now() < deadline => {
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            Ok(response) => {
                anyhow::bail!(
                    "Runtime server did not become ready: GET {url} -> {}",
                    response.status()
                )
            }
            Err(error) => anyhow::bail!("Runtime server did not become ready at {url}: {error}"),
        }
    }
}

async fn recover_runtime_indexes(state: &PlatformState, tenant_ids: &[TenantId]) {
    for step in runtime_recovery_plan(tenant_ids) {
        match step {
            RuntimeRecoveryStep::PopulateIndex(tenant) => {
                let tenant_id = TenantId::new(&tenant);
                state.server.populate_index_from_store(&tenant_id).await;
                let count = state
                    .server
                    .active_entity_counts_by_tenant()
                    .get(&tenant)
                    .copied()
                    .unwrap_or(0);
                tracing::info!(tenant = %tenant, count, "live restore: populate_index");
            }
            RuntimeRecoveryStep::PopulateFieldIndex(tenant) => {
                let tenant_id = TenantId::new(&tenant);
                state
                    .server
                    .populate_field_index_from_snapshots(&tenant_id)
                    .await;
            }
        }
    }
}

/// Run the Open Paw daemon.
///
/// If `force_soul_setup` is true, the soul personalization interview runs
/// after boot regardless of current configuration (used by `openpaw setup`).
pub async fn run(mut config: Config, force_soul_setup: bool) -> Result<()> {
    let startup_started = Instant::now();
    let port = config.port;
    let tenant = config.tenant.clone();
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    let data_dir = Path::new(&home).join(".local/share/openpaw");
    std::fs::create_dir_all(&data_dir)
        .with_context(|| format!("Failed to create data dir: {}", data_dir.display()))?;
    let api_key_path = data_dir.join("api.key");
    config.temper_api_key = Some(load_or_create_temper_api_key(
        config.temper_api_key.clone(),
        &api_key_path,
    )?);

    // Phase 0: Config setup (API key + messaging — runs pre-boot)
    let needs_soul_setup = if crate::setup::needs_setup(&data_dir, &config) {
        let setup_result = crate::setup::run_setup_config(&config).await?;
        crate::setup::merge_setup_into_config(&mut config, setup_result);
        true
    } else {
        force_soul_setup
    };

    // Reserve the API listener before bootstrapping any app config that needs a local base URL.
    // This gives us the real port up front and prevents other local helper processes from
    // stealing the preferred port while startup is still seeding secrets and entity config.
    tracing::info!("Phase 0.5: Reserving API listener...");
    let listener = match tokio::net::TcpListener::bind(format!("0.0.0.0:{port}")).await {
        Ok(l) => l,
        Err(_) => {
            tracing::warn!(port, "Port {port} in use — binding to a free port");
            tokio::net::TcpListener::bind("0.0.0.0:0")
                .await
                .context("Failed to bind to any port")?
        }
    };
    let actual_port = listener.local_addr()?.port();
    if actual_port != port {
        tracing::info!("Using port {actual_port} instead of {port}");
    }

    // Phase 1: Storage backend (Turso)
    tracing::info!("Phase 1: Initializing storage...");
    let default_db_path = data_dir.join("paw.db");
    let turso_url = config
        .turso_url
        .clone()
        .unwrap_or_else(|| format!("file:{}", default_db_path.display()));
    let turso_store = TursoEventStore::new(&turso_url, config.turso_auth_token.as_deref())
        .await
        .map_err(|e| anyhow::anyhow!("Failed to connect to Turso/libSQL: {e}"))?;
    tracing::info!("Storage: turso ({turso_url})");

    // Phase 2: Build empty registry
    tracing::info!("Phase 2: Building spec registry...");
    let registry = temper_server::SpecRegistry::new();

    // Phase 3: Set OS apps directory + reference apps
    tracing::info!("Phase 3: Loading OS apps from ./os-apps/...");
    let os_apps_dir = PathBuf::from("os-apps");
    if os_apps_dir.exists() {
        temper_platform::os_apps::set_os_apps_dir(os_apps_dir.clone());
    } else {
        tracing::warn!("os-apps/ directory not found — OS apps will not be available");
    }

    // Register reference apps (available for install_app() but NOT auto-installed)
    let reference_apps_dir = PathBuf::from("reference-projects/deep-sci-fi");
    if reference_apps_dir.exists() {
        temper_platform::os_apps::add_os_apps_dir(reference_apps_dir);
        tracing::info!("  Reference apps directory registered (available for install)");
    }

    // Register Kotowari teaching platform apps
    let kotowari_dir = PathBuf::from("reference-projects/kotowari");
    if kotowari_dir.exists() {
        temper_platform::os_apps::add_os_apps_dir(kotowari_dir);
        tracing::info!("  Kotowari apps directory registered (available for install)");
    }

    // Phase 3b: Sync git app sources (TEMPER_APP_SOURCES env var)
    if std::env::var("TEMPER_APP_SOURCES").is_ok() {
        let git_apps_cache = data_dir.join("git-apps");
        match temper_platform::os_apps::git_sources::sync_and_register_git_sources(&git_apps_cache)
        {
            Ok(repos) => {
                for name in &repos {
                    tracing::info!("  Git app source registered: {name}");
                }
            }
            Err(e) => tracing::warn!("Failed to sync git app sources: {e}"),
        }
    }

    // Phase 4: Assemble PlatformState
    tracing::info!("Phase 4: Assembling platform state...");
    let llm_api_key = config
        .anthropic_api_key
        .clone()
        .or_else(|| config.openrouter_api_key.clone())
        .or_else(|| config.openai_api_key.clone())
        .or_else(|| config.openai_codex_token.clone());
    let mut state = PlatformState::with_registry(registry, llm_api_key);
    state.api_token = config.temper_api_key.clone();
    state.server.data_dir = data_dir.clone();
    state.server.event_store = Some(Arc::new(ServerEventStore::Turso(turso_store.clone())));

    {
        let mut registry = state.registry.write().unwrap(); // ci-ok: infallible lock
        let restored = restore_registry_from_turso(&mut registry, &turso_store)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to restore registry from Turso: {e}"))?;
        if restored > 0 {
            tracing::info!("Restored {restored} specs from Turso");
        }
    }

    // Phase 4b: Bootstrap system + agent specs (GovernanceDecision, Agent, Plan, etc.)
    // Required for Cedar authorization to work — temper-system needs GovernanceDecision.
    {
        let sys_cache = turso_store
            .load_verification_cache("temper-system")
            .await
            .unwrap_or_default();
        let sys_hashes = temper_platform::bootstrap_system_tenant(&state, &sys_cache);
        temper_platform::persist_system_verification(&turso_store, &sys_hashes).await;

        let agent_cache = turso_store
            .load_verification_cache(&tenant)
            .await
            .unwrap_or_default();
        let agent_hashes =
            temper_platform::bootstrap_agent_specs(&state, &tenant, true, &agent_cache);
        temper_platform::persist_agent_verification(&turso_store, &tenant, &agent_hashes).await;
        tracing::info!("Bootstrapped system + agent specs for temper-system and {tenant}");
    }

    // Phase 5: Secrets vault
    tracing::info!("Phase 5: Configuring secrets vault...");
    let vault_key_bytes: [u8; 32] = {
        let vault_key_path = data_dir.join("vault.key");
        let key_bytes: [u8; 32] = if let Some(ref key_b64) = config.vault_key {
            use base64::Engine as _;

            match base64::engine::general_purpose::STANDARD.decode(key_b64) {
                Ok(decoded) if decoded.len() == 32 => {
                    tracing::info!("Vault key loaded from TEMPER_VAULT_KEY env var");
                    decoded.try_into().unwrap()
                }
                Ok(decoded) => {
                    let mut key = [0u8; 32];
                    rand::fill(&mut key);
                    tracing::warn!(
                        actual_len = decoded.len(),
                        "TEMPER_VAULT_KEY was not 32 bytes after base64 decode — using ephemeral vault key"
                    );
                    key
                }
                Err(error) => {
                    let mut key = [0u8; 32];
                    rand::fill(&mut key);
                    tracing::warn!(
                        %error,
                        "TEMPER_VAULT_KEY was invalid base64 — using ephemeral vault key"
                    );
                    key
                }
            }
        } else if vault_key_path.exists() {
            // Load persisted vault key from file
            use base64::Engine as _;
            match std::fs::read_to_string(&vault_key_path) {
                Ok(contents) => {
                    match base64::engine::general_purpose::STANDARD.decode(contents.trim()) {
                        Ok(decoded) if decoded.len() == 32 => {
                            tracing::info!(
                                path = %vault_key_path.display(),
                                "Vault key loaded from file"
                            );
                            decoded.try_into().unwrap()
                        }
                        _ => {
                            tracing::warn!(
                                path = %vault_key_path.display(),
                                "Vault key file was corrupt — generating new key"
                            );
                            let key = generate_and_save_vault_key(&vault_key_path)?;
                            key
                        }
                    }
                }
                Err(error) => {
                    tracing::warn!(
                        %error,
                        path = %vault_key_path.display(),
                        "Failed to read vault key file — generating new key"
                    );
                    generate_and_save_vault_key(&vault_key_path)?
                }
            }
        } else {
            // First run: generate and persist a new vault key
            tracing::info!(
                path = %vault_key_path.display(),
                "No vault key found — generating and saving new key"
            );
            generate_and_save_vault_key(&vault_key_path)?
        };
        // If we generated a new key (no env var) and Railway is available, persist it
        // so the key survives across container redeploys (Railway has no persistent disk).
        if config.vault_key.is_none() {
            if let (Some(token), Some(project_id), Some(env_id), Some(service_id)) = (
                &config.railway_token,
                &config.railway_project_id,
                &config.railway_environment_id,
                &config.railway_service_id,
            ) {
                use base64::Engine as _;
                let key_b64 = base64::engine::general_purpose::STANDARD.encode(&key_bytes);
                match persist_vault_key_to_railway(token, project_id, env_id, service_id, &key_b64)
                    .await
                {
                    Ok(()) => {
                        tracing::info!("Vault key persisted to Railway env var TEMPER_VAULT_KEY");
                    }
                    Err(e) => {
                        tracing::warn!(
                            %e,
                            "Failed to persist vault key to Railway — account data will be lost on next redeploy"
                        );
                    }
                }
            }
        }

        let vault = temper_server::secrets::vault::SecretsVault::new(&key_bytes);
        state.server.secrets_vault = Some(Arc::new(vault));
        key_bytes
    };

    // Phase 5b: Restore secrets from Turso (before env seeding so env vars take priority)
    if let Some(ref vault) = state.server.secrets_vault {
        restore_secrets_from_turso_as_platform(vault, &turso_store, &tenant).await;
        if tenant != "default" {
            // Migration shim for older deployments that stored shared startup
            // secrets under the legacy "default" tenant bucket.
            restore_secrets_from_turso_as_platform(vault, &turso_store, "default").await;
        }
    }

    // Phase 5c: Seed secrets from env (env vars override Turso-stored values)
    //
    // Each secret is cached in-memory AND persisted to Turso so it survives
    // restarts even if the env var is later removed.
    if let Some(ref vault) = state.server.secrets_vault {
        /// Helper to seed a shared platform secret from an optional env value.
        macro_rules! seed_secret {
            ($vault:expr, $store:expr, $tenant:expr, $key:expr, $value:expr) => {
                if let Some(ref val) = $value {
                    cache_platform_and_persist_secret($vault, $store, $tenant, $key, val.clone())
                        .await;
                }
            };
        }

        seed_secret!(
            vault,
            &turso_store,
            &tenant,
            "anthropic_api_key",
            config.anthropic_api_key
        );
        seed_secret!(
            vault,
            &turso_store,
            &tenant,
            "openrouter_api_key",
            config.openrouter_api_key
        );
        seed_secret!(
            vault,
            &turso_store,
            &tenant,
            "openai_api_key",
            config.openai_api_key
        );
        seed_secret!(
            vault,
            &turso_store,
            &tenant,
            "openai_codex_token",
            config.openai_codex_token
        );
        seed_secret!(
            vault,
            &turso_store,
            &tenant,
            "llm_provider",
            config.llm_provider
        );
        // Seed llm_model derived from llm_provider
        {
            let provider = config.llm_provider.as_deref().unwrap_or("anthropic");
            let default_model = match provider {
                "openai" | "openai_codex" => {
                    std::env::var("LLM_MODEL").unwrap_or_else(|_| "gpt-5.4".to_string())
                }
                "openrouter" => std::env::var("LLM_MODEL")
                    .unwrap_or_else(|_| "anthropic/claude-sonnet-4.6".to_string()),
                _ => std::env::var("LLM_MODEL").unwrap_or_else(|_| "claude-sonnet-4-6".to_string()),
            };
            cache_platform_and_persist_secret(
                vault,
                &turso_store,
                &tenant,
                "llm_model",
                default_model,
            )
            .await;
        }
        seed_secret!(
            vault,
            &turso_store,
            &tenant,
            "tensorlake_api_key",
            config.tensorlake_api_key
        );
        seed_secret!(
            vault,
            &turso_store,
            &tenant,
            "sandbox_provider",
            config.sandbox_provider
        );
        seed_secret!(
            vault,
            &turso_store,
            &tenant,
            "modal_token_id",
            config.modal_token_id
        );
        seed_secret!(
            vault,
            &turso_store,
            &tenant,
            "modal_token_secret",
            config.modal_token_secret
        );
        seed_secret!(
            vault,
            &turso_store,
            &tenant,
            "modal_bridge_url",
            config.modal_bridge_url
        );
        seed_secret!(
            vault,
            &turso_store,
            &tenant,
            "github_token",
            config.github_token
        );
        seed_secret!(
            vault,
            &turso_store,
            &tenant,
            "dd_api_key",
            config.dd_api_key
        );
        seed_secret!(
            vault,
            &turso_store,
            &tenant,
            "dd_app_key",
            config.dd_app_key
        );
        seed_secret!(
            vault,
            &turso_store,
            &tenant,
            "exa_api_key",
            config.exa_api_key
        );
        seed_secret!(
            vault,
            &turso_store,
            &tenant,
            "temper_api_key",
            config.temper_api_key
        );
        seed_secret!(
            vault,
            &turso_store,
            &tenant,
            "discord_bot_token",
            config.discord_bot_token
        );
        seed_secret!(
            vault,
            &turso_store,
            &tenant,
            "discord_public_key",
            config.discord_public_key
        );
        seed_secret!(
            vault,
            &turso_store,
            &tenant,
            "discord_guild_id",
            config.discord_guild_id
        );
        seed_secret!(
            vault,
            &turso_store,
            &tenant,
            "discord_feed_channel_id",
            config.discord_feed_channel_id
        );
        seed_secret!(
            vault,
            &turso_store,
            &tenant,
            "discord_forum_channel_id",
            config.discord_forum_channel_id
        );
        seed_secret!(
            vault,
            &turso_store,
            &tenant,
            "slack_bot_token",
            config.slack_bot_token
        );
        seed_secret!(
            vault,
            &turso_store,
            &tenant,
            "slack_app_token",
            config.slack_app_token
        );
        seed_secret!(
            vault,
            &turso_store,
            &tenant,
            "fly_api_token",
            config.fly_api_token
        );
        seed_secret!(
            vault,
            &turso_store,
            &tenant,
            "railway_token",
            config.railway_token
        );
        seed_secret!(
            vault,
            &turso_store,
            &tenant,
            "railway_project_id",
            config.railway_project_id
        );
        seed_secret!(
            vault,
            &turso_store,
            &tenant,
            "railway_environment_id",
            config.railway_environment_id
        );
        seed_secret!(
            vault,
            &turso_store,
            &tenant,
            "railway_otel_service_id",
            config.railway_otel_service_id
        );
        seed_secret!(
            vault,
            &turso_store,
            &tenant,
            "railway_service_id",
            config.railway_service_id
        );
        seed_secret!(
            vault,
            &turso_store,
            &tenant,
            "vercel_token",
            config.vercel_token
        );

        // dd_site always has a value (defaults to "datadoghq.com")
        cache_platform_and_persist_secret(
            vault,
            &turso_store,
            &tenant,
            "dd_site",
            config.dd_site.clone(),
        )
        .await;

        // temper_api_url — always set to local server
        let api_url = format!("http://127.0.0.1:{actual_port}");
        let _ = vault.cache_platform_secret("temper_api_url", api_url);

        // Sandbox URL: explicit override for testing, otherwise Tensorlake provisions on demand.
        if let Some(sandbox_url) = std::env::var("SANDBOX_URL").ok().filter(|s| !s.is_empty()) {
            cache_platform_and_persist_secret(
                vault,
                &turso_store,
                &tenant,
                "sandbox_url",
                sandbox_url.clone(),
            )
            .await;
        } else {
            let provider = vault
                .get_secret(&tenant, "sandbox_provider")
                .or_else(|| config.sandbox_provider.clone())
                .unwrap_or_else(|| "tensorlake".to_string());
            let tensorlake_api_key = vault
                .get_secret(&tenant, "tensorlake_api_key")
                .or_else(|| config.tensorlake_api_key.clone());
            let modal_token_id = vault
                .get_secret(&tenant, "modal_token_id")
                .or_else(|| config.modal_token_id.clone());
            let modal_token_secret = vault
                .get_secret(&tenant, "modal_token_secret")
                .or_else(|| config.modal_token_secret.clone());
            let modal_bridge_url = vault
                .get_secret(&tenant, "modal_bridge_url")
                .or_else(|| config.modal_bridge_url.clone());
            match provider.as_str() {
                "tensorlake" if tensorlake_api_key.is_some() => {
                    tracing::info!("Sandbox provider: tensorlake (API key configured)");
                }
                "modal"
                    if modal_token_id.is_some()
                        && modal_token_secret.is_some()
                        && modal_bridge_url.is_some() =>
                {
                    tracing::info!("Sandbox provider: modal (token + bridge configured)");
                }
                "modal" if modal_token_id.is_some() && modal_token_secret.is_some() => {
                    tracing::warn!(
                        "Sandbox provider is 'modal' but MODAL_BRIDGE_URL / modal_bridge_url is not set; OpenPaw deploy should provision it automatically"
                    );
                }
                "modal" => {
                    tracing::warn!(
                        "Sandbox provider is 'modal' but MODAL_TOKEN_ID or MODAL_TOKEN_SECRET not set"
                    );
                }
                "tensorlake" => {
                    tracing::warn!("No TL_API_KEY or SANDBOX_URL — sandbox provisioning will fail");
                }
                other => {
                    tracing::warn!(
                        "Unsupported SANDBOX_PROVIDER={other} — use 'tensorlake' or 'modal'"
                    );
                }
            }
        }

        // Blob store for TemperFS content uploads/downloads.
        //
        // Default to Temper's own internal blob route so local deployments keep
        // storage in-process and can benefit from server-side backpressure and
        // fast paths. External S3/R2-compatible endpoints can still override via
        // BLOB_ENDPOINT.
        let blob_endpoint = if let Ok(url) = std::env::var("BLOB_ENDPOINT") {
            url
        } else {
            format!("http://127.0.0.1:{actual_port}/_internal/blobs")
        };
        let blob_bucket = std::env::var("BLOB_BUCKET").unwrap_or_else(|_| "temper-fs".into());
        let _ = vault.cache_platform_secret("blob_endpoint", blob_endpoint);
        let _ = vault.cache_platform_secret("blob_bucket", blob_bucket);

        // HMAC credentials for GCS (or any S3-compatible blob store).
        if let Ok(key) = std::env::var("BLOB_ACCESS_KEY") {
            cache_platform_and_persist_secret(
                vault,
                &turso_store,
                &tenant,
                "blob_access_key",
                key.clone(),
            )
            .await;
        }
        if let Ok(key) = std::env::var("BLOB_SECRET_KEY") {
            cache_platform_and_persist_secret(
                vault,
                &turso_store,
                &tenant,
                "blob_secret_key",
                key.clone(),
            )
            .await;
        }
    }

    // Phase 6: Install Paw OS apps
    let phase_started = Instant::now();
    tracing::info!("Phase 6: Installing Paw OS apps...");
    let wasm_policy =
        local_wasm_startup_policy(std::env::var("OPENPAW_WASM_STARTUP_POLICY").ok().as_deref());
    tracing::info!(?wasm_policy, "WASM startup policy selected");
    let startup_apps = startup_os_apps();
    tracing::info!(apps = ?startup_apps, "Startup OS app surface resolved from manifests");
    if wasm_policy == LocalWasmStartupPolicy::BuildIfMissing
        && let Err(error) = build_missing_wasm_modules(&os_apps_dir, &startup_apps)
    {
        tracing::error!(%error, "Failed to build local OS app WASM artifacts");
    }
    for app_name in &startup_apps {
        if temper_platform::os_apps::get_os_app(app_name).is_none() {
            tracing::warn!("Skipping OS app '{app_name}' because its bundle is missing or invalid");
            continue;
        }
        match temper_platform::install_os_app(&state, &tenant, app_name).await {
            Ok(result) => {
                persist_os_app_verification(&state, &turso_store, &tenant, app_name).await;
                tracing::info!("  Installed {app_name}: {result:?}");
            }
            Err(e) => tracing::error!("  Failed to install {app_name}: {e}"),
        }
    }

    // Safety net: commit all specs for the tenant.
    // install_os_app() calls commit_specs() internally, but if a previous daemon
    // run crashed between upsert (committed=0) and commit (committed=1), specs
    // would be left uncommitted and deleted on the NEXT restart by
    // delete_uncommitted_specs(). This explicit commit ensures all OS app specs
    // are durable before we proceed to entity hydration.
    if let Err(e) = turso_store.commit_specs(&tenant).await {
        tracing::error!("Failed to commit specs after OS app install: {e}");
    } else {
        tracing::info!("Specs committed for tenant {tenant}");
    }
    tracing::info!(
        elapsed_ms = phase_started.elapsed().as_millis(),
        "phase_6_os_app_reconcile complete"
    );

    // Phase 7: Recovery (Cedar policies + WASM modules + secrets from store)
    let phase_started = Instant::now();
    tracing::info!("Phase 7: Recovery...");
    recover_cedar_policies(&state, &turso_store).await;
    restore_installed_skills(&state, &turso_store).await;
    state
        .server
        .load_wasm_modules()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to recover WASM modules: {e}"))?;
    state.server.rebuild_reaction_dispatcher();
    let tenant_ids: Vec<TenantId> = {
        let registry = state.registry.read().unwrap(); // ci-ok: infallible lock
        registry.tenant_ids().into_iter().cloned().collect()
    };
    recover_runtime_indexes(&state, &tenant_ids).await;
    tracing::info!(
        elapsed_ms = phase_started.elapsed().as_millis(),
        "phase_7_runtime_recovery complete"
    );

    // Phase 7b: Session recovery — recover or fail orphaned sessions (ADR-0025)
    {
        let terminal_states: HashSet<&str> =
            ["Completed", "Failed", "Cancelled"].into_iter().collect();
        let recoverable_states: HashSet<&str> = [
            "Thinking",
            "Executing",
            "Compacting",
            "Steering",
            "WaitingForApproval",
        ]
        .into_iter()
        .collect();
        let tenant_id = TenantId::new(&tenant);
        let session_ids: Vec<String> = {
            let index = state.server.entity_index.read().unwrap(); // ci-ok: infallible lock
            let index_key = format!("{tenant_id}:Session");
            index
                .get(&index_key)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .collect()
        };

        let mut failed = 0u32;
        let mut recovering = 0u32;
        for session_id in &session_ids {
            match state
                .server
                .get_tenant_entity_state(&tenant_id, "Session", session_id)
                .await
            {
                Ok(resp) if recoverable_states.contains(resp.state.status.as_str()) => {
                    // Recoverable state — attempt RecoverFromRestart (ADR-0025)
                    let status = &resp.state.status;
                    tracing::info!(session_id, status, "Recovering session from restart");
                    let params = serde_json::json!({
                        "error_message": format!("process restart — recovering from {status}")
                    });
                    match state
                        .server
                        .dispatch_tenant_action(
                            &tenant_id,
                            "Session",
                            session_id,
                            "RecoverFromRestart",
                            params.clone(),
                            &temper_server::request_context::AgentContext::system(),
                        )
                        .await
                    {
                        Ok(_) => recovering += 1,
                        Err(e) => {
                            tracing::warn!(session_id, %e, "RecoverFromRestart failed, falling back to Fail");
                            let _ = state
                                .server
                                .dispatch_tenant_action(
                                    &tenant_id,
                                    "Session",
                                    session_id,
                                    "Fail",
                                    params,
                                    &temper_server::request_context::AgentContext::system(),
                                )
                                .await;
                            failed += 1;
                        }
                    }
                }
                Ok(resp) if !terminal_states.contains(resp.state.status.as_str()) => {
                    // Non-recoverable (Created, Provisioning) — just fail
                    let status = &resp.state.status;
                    tracing::info!(session_id, status, "Failing orphaned session");
                    let params = serde_json::json!({
                        "error_message": format!("process restart — session recovered from {status} state")
                    });
                    let _ = state
                        .server
                        .dispatch_tenant_action(
                            &tenant_id,
                            "Session",
                            session_id,
                            "Fail",
                            params,
                            &temper_server::request_context::AgentContext::system(),
                        )
                        .await;
                    failed += 1;
                }
                Ok(_) => {} // terminal state, skip
                Err(e) => tracing::warn!(session_id, %e, "Failed to read session state"),
            }
        }
        if recovering > 0 || failed > 0 {
            tracing::info!(recovering, failed, "Session recovery complete");
        }
    }

    // Phase 8: Banner (printed after bind so we show the actual port)
    tracing::info!("Phase 8: Bootstrap complete");

    // Phase 9: Start transports + serve using the reserved listener
    tracing::info!("Phase 9: Starting server...");
    let _ = state.server.listen_port.set(actual_port);

    // Create transport manager for hot-connect/disconnect of Discord/Slack
    let transport_manager = Arc::new(crate::transport_manager::TransportManager::new(
        tenant.clone(),
        actual_port,
        config.temper_api_key.clone(),
        config.public_base_url.clone(),
        config.ngrok_bin.clone(),
        config.ngrok_authtoken.clone(),
    ));

    // Build platform router + setup API
    let cookie_secure = config
        .public_base_url
        .as_deref()
        .map(|url| url.starts_with("https://"))
        .unwrap_or(false);
    let auth_state = crate::auth::AuthState::new(
        turso_store.clone(),
        state
            .server
            .secrets_vault
            .as_ref()
            .context("Vault must be initialized before auth")?
            .clone(),
        vault_key_bytes.to_vec(),
        tenant.clone(),
        cookie_secure,
    );

    let router = build_platform_router(state.clone());
    let setup_state = crate::setup_api::SetupApiState {
        platform: state.clone(),
        turso_store: turso_store.clone(),
        transport_manager: transport_manager.clone(),
        tenant: tenant.clone(),
        agents_dir: PathBuf::from("os-apps/paw-agent/agents"),
        base_url: format!("http://127.0.0.1:{actual_port}"),
        build_version: config.build_version.clone(),
        build_sha: config.build_sha.clone(),
    };
    let router = router
        .merge(crate::setup_api::router(setup_state))
        .merge(crate::auth::router(auth_state.clone()));

    // Raise the default body limit from 2 MB to 50 MB so large REPL state
    // files and embodiment uploads don't get rejected with HTTP 413.
    let router = router.layer(axum::extract::DefaultBodyLimit::max(50 * 1024 * 1024));

    // Serve the dashboard SPA from dashboard/build if available.
    let router = if std::path::Path::new("dashboard/build").exists() {
        use tower_http::services::{ServeDir, ServeFile};
        router.nest_service(
            "/dashboard",
            ServeDir::new("dashboard/build").fallback(ServeFile::new("dashboard/build/index.html")),
        )
    } else {
        router
    };
    let router = router.layer(axum::middleware::from_fn_with_state(
        auth_state,
        crate::auth::middleware,
    ));

    let serve_handle = spawn_runtime_server(listener, router);
    wait_for_runtime_server(
        format!("http://127.0.0.1:{actual_port}/healthz").as_str(),
        Duration::from_secs(5),
    )
    .await
    .context("Open Paw HTTP API failed to become reachable during startup")?;

    // Spawn webhook trigger (ONE entity, ONE action per request).
    spawn_webhook_trigger(&tenant, actual_port, config.temper_api_key.clone());

    // Cron scheduling is now handled by the platform's schedule_at effect —
    // CronJob entities self-schedule via ActivateComplete/TriggerComplete.

    // Start transports from vault (env vars were seeded into vault in Phase 5).
    // The TransportManager enables runtime connect/disconnect via the /paw/ API.
    {
        let vault = state.server.secrets_vault.as_ref();
        let discord_token = vault.and_then(|v| v.get_secret(&tenant, "discord_bot_token"));
        if let Some(token) = discord_token {
            let public_key = vault
                .and_then(|v| v.get_secret(&tenant, "discord_public_key"))
                .or_else(|| config.discord_public_key.clone())
                .unwrap_or_default();
            let guild_id = vault
                .and_then(|v| v.get_secret(&tenant, "discord_guild_id"))
                .or_else(|| config.discord_guild_id.clone());
            let feed_channel_id = vault
                .and_then(|v| v.get_secret(&tenant, "discord_feed_channel_id"))
                .or_else(|| config.discord_feed_channel_id.clone());
            let forum_channel_id = vault
                .and_then(|v| v.get_secret(&tenant, "discord_forum_channel_id"))
                .or_else(|| config.discord_forum_channel_id.clone());

            if let Some(interaction_url) = startup_discord_connect_result(
                transport_manager
                    .connect_discord(crate::transport_manager::DiscordConnectParams {
                        bot_token: token,
                        public_key,
                        guild_id: guild_id.clone(),
                        feed_channel_id: feed_channel_id.clone(),
                        forum_channel_id: forum_channel_id.clone(),
                    })
                    .await,
            ) {
                tracing::info!(%interaction_url, "Discord transport ready");

                // Spawn Discord observer (SSE → Discord feed/forum).
                if feed_channel_id.is_some() || forum_channel_id.is_some() {
                    let bot_token_for_observer = vault
                        .and_then(|v| v.get_secret(&tenant, "discord_bot_token"))
                        .unwrap_or_default();
                    let observer_api =
                        paw_transport::PawApiClient::new(paw_transport::PawApiConfig {
                            base_url: format!("http://127.0.0.1:{actual_port}"),
                            tenant: tenant.clone(),
                            api_key: config.temper_api_key.clone(),
                        });
                    let observer_config = paw_transport::discord::ObserverConfig {
                        bot_token: bot_token_for_observer,
                        feed_channel_id,
                        forum_channel_id,
                    };
                    tokio::spawn(async move {
                        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                        if let Err(e) =
                            paw_transport::discord::run_observer(observer_api, observer_config)
                                .await
                        {
                            tracing::error!("Discord observer failed: {e}");
                        }
                    });
                }
            }
        } else {
            tracing::warn!("No discord_bot_token in vault — Discord transport not started");
        }

        let slack_bot = vault.and_then(|v| v.get_secret(&tenant, "slack_bot_token"));
        let slack_app = vault.and_then(|v| v.get_secret(&tenant, "slack_app_token"));
        if let (Some(app_token), Some(bot_token)) = (slack_app, slack_bot) {
            let signing_secret = vault
                .and_then(|v| v.get_secret(&tenant, "slack_signing_secret"))
                .or_else(|| config.slack_signing_secret.clone())
                .unwrap_or_default();
            transport_manager
                .connect_slack(crate::transport_manager::SlackConnectParams {
                    app_token,
                    bot_token,
                    signing_secret,
                })
                .await;
        } else {
            tracing::warn!("No slack tokens in vault — Slack transport not started");
        }
    }

    // Spawn background loops
    state.server.spawn_runtime_metrics_loop();
    spawn_actor_passivation_loop(&state);

    // Resolve LLM provider from vault (dashboard-set) before falling back to env var / default.
    let resolved_llm_provider = state
        .server
        .secrets_vault
        .as_ref()
        .and_then(|v| v.get_secret(&tenant, "llm_provider"))
        .or_else(|| config.llm_provider.clone())
        .unwrap_or_else(|| "anthropic".to_string());
    let preserve_personalized_paw_soul = state
        .server
        .secrets_vault
        .as_ref()
        .and_then(|v| v.get_secret(&tenant, "paw_personalized_soul"))
        .is_some_and(|value| matches!(value.trim(), "1" | "true" | "yes"));

    spawn_soul_bootstrap(
        actual_port,
        tenant.clone(),
        config.temper_api_key.clone(),
        resolved_llm_provider,
        preserve_personalized_paw_soul,
    );

    // Print startup summary
    {
        let vault = state.server.secrets_vault.as_ref();
        let has_api_key = vault
            .and_then(|v| {
                v.get_secret(&tenant, "anthropic_api_key")
                    .or_else(|| v.get_secret(&tenant, "openrouter_api_key"))
                    .or_else(|| v.get_secret(&tenant, "openai_api_key"))
                    .or_else(|| v.get_secret(&tenant, "openai_codex_token"))
            })
            .is_some();
        let has_discord = vault
            .and_then(|v| v.get_secret(&tenant, "discord_bot_token"))
            .is_some();
        let has_slack = vault
            .and_then(|v| v.get_secret(&tenant, "slack_bot_token"))
            .is_some();

        println!();
        println!("  Open Paw is running.");
        println!();
        println!("  API:       http://localhost:{actual_port}/tdata");
        println!("  Dashboard: http://localhost:{actual_port}/dashboard");
        println!(
            "  API key:   {}",
            config.temper_api_key.as_deref().unwrap_or("")
        );
        println!();
        if has_api_key {
            println!("  \u{2713} LLM API key");
        }
        if has_discord {
            println!("  \u{2713} Discord");
            if let Some(interaction_url) = transport_manager.discord_interaction_public_url().await
            {
                println!("  Discord interactions: {interaction_url}");
            }
        }
        if has_slack {
            println!("  \u{2713} Slack");
        }
        if !has_api_key && !has_discord && !has_slack {
            println!("  Run setup: cargo run -- setup");
        }
        println!();
    }
    tracing::info!("Open Paw listening on port {actual_port}");
    tracing::info!(elapsed_ms = startup_started.elapsed().as_millis(), tenant = %tenant, "startup: time to healthy");

    // Phase 10: Soul personalization (post-boot, writes to TemperFS via OData)
    if needs_soul_setup {
        let api_key = state
            .server
            .secrets_vault
            .as_ref()
            .and_then(|v| {
                v.get_secret(&tenant, "anthropic_api_key")
                    .or_else(|| v.get_secret(&tenant, "openrouter_api_key"))
                    .or_else(|| v.get_secret(&tenant, "openai_api_key"))
                    .or_else(|| v.get_secret(&tenant, "openai_codex_token"))
            })
            .unwrap_or_default();
        let provider_name = state
            .server
            .secrets_vault
            .as_ref()
            .and_then(|v| v.get_secret(&tenant, "llm_provider"))
            .unwrap_or_else(|| "anthropic".to_string());
        let setup_auth = crate::setup::SetupRequestAuth::from_cookie(
            crate::auth::issue_session_cookie_value(&vault_key_bytes, "bootstrap@local.openpaw")?,
        );

        if let Err(e) =
            crate::setup::run_setup_soul(actual_port, &api_key, &provider_name, &tenant, setup_auth)
                .await
        {
            tracing::warn!("Soul setup failed: {e}");
        }

        serve_handle.await??;
    } else {
        serve_handle.await??;
    }

    Ok(())
}

/// Cache a shared platform secret in-memory and persist it under the configured
/// tenant bucket so it survives restarts without a Turso schema change.
async fn cache_platform_and_persist_secret(
    vault: &temper_server::secrets::vault::SecretsVault,
    store: &TursoEventStore,
    tenant: &str,
    key: &str,
    value: String,
) {
    let _ = vault.cache_platform_secret(key, value.clone());
    match vault.encrypt(value.as_bytes()) {
        Ok((ciphertext, nonce)) => {
            if let Err(e) = store.upsert_secret(tenant, key, &ciphertext, &nonce).await {
                tracing::warn!(key, tenant, %e, "Failed to persist secret to Turso");
            }
        }
        Err(e) => {
            tracing::warn!(key, tenant, %e, "Failed to encrypt secret for persistence");
        }
    }
}

/// Restore persisted shared secrets from Turso into the platform cache.
///
/// The first restored value wins so a configured tenant bucket takes
/// precedence over the legacy `"default"` bucket during migration.
async fn restore_secrets_from_turso_as_platform(
    vault: &temper_server::secrets::vault::SecretsVault,
    store: &TursoEventStore,
    tenant: &str,
) {
    match store.load_secrets_for_tenant(tenant).await {
        Ok(rows) => {
            let mut restored = 0u32;
            for (key_name, ciphertext, nonce) in rows {
                match vault.decrypt(&ciphertext, &nonce) {
                    Ok(plaintext) => {
                        if let Ok(value) = String::from_utf8(plaintext) {
                            if vault.get_platform_secret(&key_name).is_none() {
                                let _ = vault.cache_platform_secret(&key_name, value);
                                restored += 1;
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!(
                            key = key_name,
                            tenant,
                            %e,
                            "Failed to decrypt secret from Turso — skipping"
                        );
                    }
                }
            }
            if restored > 0 {
                tracing::info!(tenant, restored, "Restored secrets from Turso");
            }
        }
        Err(e) => {
            tracing::warn!(tenant, %e, "Failed to load secrets from Turso");
        }
    }
}

/// Generate a random 32-byte vault key, save it to disk as base64, and return the raw bytes.
fn generate_and_save_vault_key(path: &Path) -> Result<[u8; 32]> {
    use base64::Engine as _;

    let mut key = [0u8; 32];
    rand::fill(&mut key);
    let encoded = base64::engine::general_purpose::STANDARD.encode(&key);
    std::fs::write(path, &encoded)
        .with_context(|| format!("Failed to write vault key to {}", path.display()))?;

    // Set file permissions to owner-only (0o600) on Unix.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o600);
        std::fs::set_permissions(path, perms)
            .with_context(|| format!("Failed to set permissions on {}", path.display()))?;
    }

    tracing::info!(path = %path.display(), "Saved new vault key to file");
    Ok(key)
}

/// Persist the vault key to Railway as an environment variable so it survives container redeploys.
/// Railway containers have no persistent disk, so without this the vault key is regenerated
/// on every deploy and all encrypted secrets (including user accounts) become unreadable.
async fn persist_vault_key_to_railway(
    token: &str,
    project_id: &str,
    environment_id: &str,
    service_id: &str,
    vault_key_b64: &str,
) -> Result<()> {
    let client = reqwest::Client::new();
    let query = serde_json::json!({
        "query": "mutation($input: VariableCollectionUpsertInput!) { variableCollectionUpsert(input: $input) }",
        "variables": {
            "input": {
                "projectId": project_id,
                "environmentId": environment_id,
                "serviceId": service_id,
                "variables": {
                    "TEMPER_VAULT_KEY": vault_key_b64
                }
            }
        }
    });

    let resp = client
        .post("https://backboard.railway.com/graphql/v2")
        .bearer_auth(token)
        .json(&query)
        .send()
        .await
        .context("Railway GraphQL request failed")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("Railway API returned {status}: {body}");
    }

    let body: serde_json::Value = resp.json().await.unwrap_or_default();
    if let Some(errors) = body.get("errors") {
        anyhow::bail!("Railway GraphQL errors: {errors}");
    }

    Ok(())
}

fn load_or_create_temper_api_key(explicit_key: Option<String>, path: &Path) -> Result<String> {
    if let Some(key) = explicit_key {
        return Ok(key);
    }

    if path.exists() {
        let key = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read API key from {}", path.display()))?;
        let key = key.trim().to_string();
        if !key.is_empty() {
            return Ok(key);
        }
    }

    let key = {
        use rand::Rng;
        let bytes: [u8; 32] = rand::rng().random();
        hex::encode(bytes)
    };
    std::fs::write(path, &key)
        .with_context(|| format!("Failed to write API key to {}", path.display()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o600);
        std::fs::set_permissions(path, perms)
            .with_context(|| format!("Failed to set permissions on {}", path.display()))?;
    }

    tracing::info!(path = %path.display(), "Saved new API key to file");
    Ok(key)
}

async fn persist_os_app_verification(
    state: &PlatformState,
    store: &TursoEventStore,
    tenant: &str,
    app_name: &str,
) {
    let Some(bundle) = get_os_app(app_name) else {
        return;
    };
    let verified_at = sim_now().to_rfc3339();
    let tenant_id = TenantId::new(tenant);

    for (entity_type, _) in &bundle.specs {
        if let Err(error) = store
            .persist_spec_verification(
                tenant,
                entity_type,
                TursoSpecVerificationUpdate {
                    status: "completed",
                    verified: true,
                    levels_passed: None,
                    levels_total: None,
                    verification_result_json: None,
                },
            )
            .await
        {
            tracing::warn!(
                tenant,
                app = app_name,
                entity_type,
                error = %error,
                "Failed to persist OS app verification status"
            );
        }

        let mut registry = state.registry.write().unwrap(); // ci-ok: infallible lock
        registry.set_verification_status(
            &tenant_id,
            entity_type,
            VerificationStatus::Completed(EntityVerificationResult {
                all_passed: true,
                levels: vec![EntityLevelSummary {
                    level: "Bootstrap".to_string(),
                    passed: true,
                    summary: format!("Pre-verified via os-app install ({app_name})"),
                    details: None,
                }],
                verified_at: verified_at.clone(),
            }),
        );
    }
}

/// Bootstrap Paw souls into the entity system.
///
/// Reads soul files from `os-apps/paw-agent/agents/` directory, creates TemperFS File entities
/// for the content, and registers Soul entities. Runs once on first boot;
/// skips if souls already exist.
fn spawn_soul_bootstrap(
    port: u16,
    tenant: String,
    api_key: Option<String>,
    llm_provider: String,
    preserve_personalized_paw_soul: bool,
) {
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(500)).await;

        let api_url = format!("http://127.0.0.1:{port}");
        let client = reqwest::Client::new();

        // Check for personalized Paw soul from `openpaw setup`
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
        let generated_dir = Path::new(&home).join(".local/share/openpaw/generated");
        let gen_soul = generated_dir.join("paw-soul.md");
        let gen_style = generated_dir.join("paw-style.md");
        let gen_user = generated_dir.join("user.md");

        // Build Paw's soul paths: prefer generated files, always include AGENT.md for operations
        let mut paw_paths: Vec<String> = Vec::new();
        if gen_soul.exists() {
            paw_paths.push(gen_soul.to_string_lossy().to_string());
            if gen_style.exists() {
                paw_paths.push(gen_style.to_string_lossy().to_string());
            }
            if gen_user.exists() {
                paw_paths.push(gen_user.to_string_lossy().to_string());
            }
            tracing::info!("Using personalized Paw soul from setup");
        } else {
            paw_paths.push("os-apps/paw-agent/agents/paw/SOUL.md".to_string());
            paw_paths.push("os-apps/paw-agent/agents/paw/STYLE.md".to_string());
        }
        // AGENT.md always included — operational instructions don't change with personalization
        paw_paths.push("os-apps/paw-agent/agents/paw/AGENT.md".to_string());
        let paw_path_refs: Vec<&str> = paw_paths.iter().map(|s| s.as_str()).collect();

        // Agent definitions: (name, role, description, soul_paths)
        // Agent is the primary entity. Soul is optional — attached to Agent by ID.
        let agents: Vec<(&str, &str, &str, Option<Vec<&str>>)> = vec![
            (
                "Paw",
                "chief-of-staff",
                "Paw chief of staff agent",
                Some(paw_path_refs),
            ),
            (
                "SWE",
                "developer",
                "Software developer agent",
                Some(vec!["os-apps/paw-agent/agents/swe/AGENT.md"]),
            ),
            (
                "SRE",
                "sre",
                "Site reliability engineering agent",
                Some(vec!["os-apps/paw-agent/agents/sre/AGENT.md"]),
            ),
            (
                "Probe",
                "probe",
                "Foresight probe agent for projecting product futures",
                Some(vec!["os-apps/paw-agent/agents/probe/AGENT.md"]),
            ),
        ];

        let default_config = default_agent_config(&api_url, &api_key, &llm_provider);

        for (name, role, description, soul_paths) in &agents {
            // Step 1: Create Agent entity (agent-first)
            let agent_id = match bootstrap_agent(
                &client,
                &api_url,
                &tenant,
                &api_key,
                name,
                role,
                description,
                &default_config,
            )
            .await
            {
                Ok(id) => {
                    tracing::info!("  Agent '{name}' ready: {id}");
                    id
                }
                Err(e) => {
                    tracing::error!("  Failed to bootstrap agent '{name}': {e}");
                    continue;
                }
            };

            // Step 2: Optionally create/attach Soul
            if let Some(paths) = soul_paths {
                match bootstrap_soul(
                    &client,
                    &api_url,
                    &tenant,
                    &api_key,
                    &agent_id,
                    name,
                    description,
                    paths,
                    preserve_personalized_paw_soul && *name == "Paw",
                )
                .await
                {
                    Ok(soul_id) => {
                        // Attach Soul to Agent by ID
                        if let Err(e) = attach_soul_to_agent(
                            &client, &api_url, &tenant, &api_key, &agent_id, &soul_id,
                        )
                        .await
                        {
                            tracing::warn!("  Could not attach soul to agent '{name}': {e}");
                        } else {
                            tracing::info!(
                                "  Soul '{name}' ({soul_id}) attached to Agent {agent_id}"
                            );
                        }
                    }
                    Err(e) => tracing::warn!("  Failed to bootstrap soul for '{name}': {e}"),
                }
            }
        }

        // Skills are now bootstrapped as TemperFS files by the OS app installer
        // (install_os_app → bootstrap_skills). No separate skill bootstrap needed.

        // Point the global AgentRoute to the Paw Agent entity (by ID, not by name)
        if let Err(e) = set_default_agent(&client, &api_url, &tenant, &api_key, "Paw").await {
            tracing::warn!("Could not set default agent on AgentRoute: {e}");
        }
    });
}

/// Create or find an Agent entity by name.
async fn bootstrap_agent(
    client: &reqwest::Client,
    api_url: &str,
    tenant: &str,
    api_key: &Option<String>,
    name: &str,
    role: &str,
    description: &str,
    config: &serde_json::Value,
) -> Result<String> {
    let escaped_name = name.replace('\'', "''");
    let filter = format!("name eq '{escaped_name}' and Status eq 'Active'");
    let list_url = format!("{api_url}/tdata/Agents?$filter={filter}");
    let resp = odata_get(client, &list_url, tenant, api_key).await?;

    if let Some(items) = resp["value"].as_array() {
        if let Some(existing) = items.first() {
            let id = entity_id_from_json(existing).unwrap_or("unknown");
            tracing::info!("  Agent '{name}' already exists: {id}");
            return Ok(id.to_string());
        }
    }

    // Create new Agent entity
    let create_resp = odata_post(
        client,
        &format!("{api_url}/tdata/Agents"),
        tenant,
        api_key,
        serde_json::json!({}),
    )
    .await?;
    let agent_id = create_resp["entity_id"]
        .as_str()
        .or_else(|| create_resp["fields"]["Id"].as_str())
        .or_else(|| create_resp["Id"].as_str())
        .context("Agent creation did not return Id")?
        .to_string();

    // Configure the agent
    let model = config["model"].as_str().unwrap_or("claude-sonnet-4-6");
    let provider = config["provider"].as_str().unwrap_or("anthropic");
    let tools_enabled = config["tools_enabled"].as_str().unwrap_or("");
    let max_turns = config["max_turns"].as_str().unwrap_or("24");

    odata_post(
        client,
        &format!("{api_url}/tdata/Agents('{agent_id}')/OpenPaw.Configure"),
        tenant,
        api_key,
        serde_json::json!({
            "name": name,
            "role": role,
            "description": description,
            "model": model,
            "provider": provider,
            "tools_enabled": tools_enabled,
            "max_turns": max_turns,
        }),
    )
    .await?;

    Ok(agent_id)
}

/// Attach a Soul entity to an Agent by updating the Agent's soul_id field.
async fn attach_soul_to_agent(
    client: &reqwest::Client,
    api_url: &str,
    tenant: &str,
    api_key: &Option<String>,
    agent_id: &str,
    soul_id: &str,
) -> Result<()> {
    odata_post(
        client,
        &format!("{api_url}/tdata/Agents('{agent_id}')/OpenPaw.Update"),
        tenant,
        api_key,
        serde_json::json!({ "soul_id": soul_id }),
    )
    .await?;
    Ok(())
}

/// Create or find a Soul entity for the given soul files.
///
/// Multiple paths are concatenated with `\n\n` separators (e.g. SOUL.md + STYLE.md + SKILL.md).
async fn bootstrap_soul(
    client: &reqwest::Client,
    api_url: &str,
    tenant: &str,
    api_key: &Option<String>,
    agent_id: &str,
    name: &str,
    description: &str,
    paths: &[&str],
    preserve_existing_content: bool,
) -> Result<String> {
    let content = paths
        .iter()
        .map(|p| {
            std::fs::read_to_string(p).with_context(|| format!("Failed to read soul file: {p}"))
        })
        .collect::<Result<Vec<_>>>()?
        .join("\n\n");

    let agent_resp = odata_get(
        client,
        &format!("{api_url}/tdata/Agents('{agent_id}')"),
        tenant,
        api_key,
    )
    .await?;
    if let Some(attached_soul_id) = entity_field_str(&agent_resp, &["soul_id", "SoulId"]) {
        let soul_resp = odata_get(
            client,
            &format!("{api_url}/tdata/Souls('{attached_soul_id}')"),
            tenant,
            api_key,
        )
        .await?;
        if let Some(file_id) = entity_field_str(&soul_resp, &["ContentFileId", "content_file_id"]) {
            if should_preserve_paw_soul_content(
                client,
                api_url,
                tenant,
                api_key,
                name,
                file_id,
                preserve_existing_content,
            )
            .await
            {
                tracing::info!("  Preserving existing soul '{name}': {attached_soul_id}");
                return Ok(attached_soul_id.to_string());
            }
            let upload_url = format!("{api_url}/tdata/Files('{file_id}')/$value");
            odata_put_bytes(
                client,
                &upload_url,
                tenant,
                api_key,
                "text/markdown",
                content.clone().into_bytes(),
            )
            .await
            .with_context(|| format!("Failed to refresh attached soul content for '{name}'"))?;
            tracing::info!("  Soul '{name}' already attached: {attached_soul_id}");
            return Ok(attached_soul_id.to_string());
        }
    }

    for filter in soul_lookup_filters(name) {
        let list_url = format!("{api_url}/tdata/Souls?$filter={filter}");
        let resp = odata_get(client, &list_url, tenant, api_key).await?;
        if let Some(existing) = resp["value"].as_array().and_then(|items| items.first()) {
            let id = entity_id_from_json(existing).unwrap_or("unknown");
            if let Some(file_id) = entity_field_str(existing, &["ContentFileId", "content_file_id"])
            {
                if should_preserve_paw_soul_content(
                    client,
                    api_url,
                    tenant,
                    api_key,
                    name,
                    file_id,
                    preserve_existing_content,
                )
                .await
                {
                    tracing::info!("  Preserving existing soul '{name}': {id}");
                    return Ok(id.to_string());
                }
                let upload_url = format!("{api_url}/tdata/Files('{file_id}')/$value");
                odata_put_bytes(
                    client,
                    &upload_url,
                    tenant,
                    api_key,
                    "text/markdown",
                    content.into_bytes(),
                )
                .await
                .with_context(|| format!("Failed to refresh existing soul content for '{name}'"))?;
            }
            tracing::info!("  Soul '{name}' already exists: {id}");
            return Ok(id.to_string());
        }
    }

    let file_resp = odata_post(
        client,
        &format!("{api_url}/tdata/Files"),
        tenant,
        api_key,
        serde_json::json!({
            "Name": format!("{name}.soul.md"),
            "MimeType": "text/markdown"
        }),
    )
    .await?;
    let file_id = file_resp["entity_id"]
        .as_str()
        .or_else(|| file_resp["fields"]["Id"].as_str())
        .or_else(|| file_resp["Id"].as_str())
        .context("File creation did not return Id")?
        .to_string();

    let upload_url = format!("{api_url}/tdata/Files('{file_id}')/$value");
    odata_put_bytes(
        client,
        &upload_url,
        tenant,
        api_key,
        "text/markdown",
        content.into_bytes(),
    )
    .await?;

    let soul_resp = odata_post(
        client,
        &format!("{api_url}/tdata/Souls"),
        tenant,
        api_key,
        serde_json::json!({
            "Name": name,
            "Description": description,
            "ContentFileId": file_id
        }),
    )
    .await?;
    let soul_id = soul_resp["entity_id"]
        .as_str()
        .or_else(|| soul_resp["fields"]["Id"].as_str())
        .or_else(|| soul_resp["Id"].as_str())
        .context("Soul creation did not return Id")?
        .to_string();

    odata_post(
        client,
        &format!("{api_url}/tdata/Souls('{soul_id}')/OpenPaw.Publish"),
        tenant,
        api_key,
        serde_json::json!({}),
    )
    .await?;

    Ok(soul_id)
}

async fn should_preserve_paw_soul_content(
    client: &reqwest::Client,
    api_url: &str,
    tenant: &str,
    api_key: &Option<String>,
    name: &str,
    file_id: &str,
    preserve_existing_content: bool,
) -> bool {
    if preserve_existing_content {
        return true;
    }

    if name != "Paw" {
        return false;
    }

    let Ok(default_content) = crate::setup::default_paw_soul_content() else {
        return false;
    };
    let Ok(current_content) = odata_get_text(
        client,
        &format!("{api_url}/tdata/Files('{file_id}')/$value"),
        tenant,
        api_key,
    )
    .await
    else {
        return false;
    };

    paw_soul_content_is_personalized(&current_content, &default_content)
}

fn paw_soul_content_is_personalized(current_content: &str, default_content: &str) -> bool {
    current_content.trim() != default_content.trim()
}

fn soul_lookup_filters(name: &str) -> [String; 2] {
    let escaped_name = name.replace('\'', "''");
    let escaped_lower_name = name.to_lowercase().replace('\'', "''");
    [
        format!("Name eq '{escaped_name}'"),
        format!("name eq '{escaped_lower_name}'"),
    ]
}

/// Point the global AgentRoute to the named Agent entity (by ID).
async fn set_default_agent(
    client: &reqwest::Client,
    api_url: &str,
    tenant: &str,
    api_key: &Option<String>,
    agent_name: &str,
) -> Result<()> {
    // Find the Agent entity by name
    let escaped_name = agent_name.replace('\'', "''");
    let agents_resp = odata_get(
        client,
        &format!("{api_url}/tdata/Agents?$filter=name eq '{escaped_name}' and Status eq 'Active'"),
        tenant,
        api_key,
    )
    .await?;
    let agents = agents_resp["value"]
        .as_array()
        .context("Failed to list active agents")?;

    let target_agent = agents
        .first()
        .context(format!("Agent '{agent_name}' not found"))?;
    let target_agent_id = entity_id_from_json(target_agent)
        .context("Agent entity missing ID")?
        .to_string();

    let routes_resp = odata_get(
        client,
        &format!("{api_url}/tdata/AgentRoutes"),
        tenant,
        api_key,
    )
    .await?;

    let mut has_global_route = false;
    if let Some(routes) = routes_resp["value"].as_array() {
        for route in routes {
            let route_id = entity_id_from_json(route).unwrap_or("");
            let current_agent_id = entity_field_str(route, &["AgentId", "agent_id"]).unwrap_or("");
            let channel_id = entity_field_str(route, &["ChannelId", "channel_id"]).unwrap_or("");
            let current_config =
                entity_field_str(route, &["AgentConfig", "agent_config"]).unwrap_or("");

            // Repair: update agent_id if missing or pointing to wrong agent
            let needs_repair = current_agent_id.is_empty() || current_agent_id != target_agent_id;
            if needs_repair && !route_id.is_empty() {
                odata_post(
                    client,
                    &format!("{api_url}/tdata/AgentRoutes('{route_id}')/Paw.Channel.Update"),
                    tenant,
                    api_key,
                    serde_json::json!({ "agent_id": target_agent_id }),
                )
                .await
                .ok();
                tracing::info!("  Set agent_id={target_agent_id} on AgentRoute {route_id}");
            }
            if !route_id.is_empty() {
                if let Some(repaired_config) =
                    repaired_agent_config(current_config, api_url, api_key, channel_id.is_empty())
                {
                    odata_post(
                        client,
                        &format!("{api_url}/tdata/AgentRoutes('{route_id}')/Paw.Channel.Update"),
                        tenant,
                        api_key,
                        serde_json::json!({ "agent_config": repaired_config }),
                    )
                    .await
                    .ok();
                    tracing::info!("  Repaired agent_config on AgentRoute {route_id}");
                }
            }
            if channel_id.is_empty() {
                has_global_route = true;
            }
        }
    }

    // Ensure a global fallback AgentRoute exists pointing to the Agent entity.
    if !has_global_route {
        tracing::info!(
            "  No global AgentRoute found — creating one with agent '{agent_name}' ({target_agent_id})"
        );
        let create_resp = odata_post(
            client,
            &format!("{api_url}/tdata/AgentRoutes"),
            tenant,
            api_key,
            serde_json::json!({}),
        )
        .await;
        if let Ok(created) = create_resp {
            let route_id = entity_id_from_json(&created).unwrap_or("");
            if !route_id.is_empty() {
                let resolved_provider =
                    std::env::var("LLM_PROVIDER").unwrap_or_else(|_| "anthropic".to_string());
                let agent_config = default_agent_config(api_url, api_key, &resolved_provider);
                odata_post(
                    client,
                    &format!("{api_url}/tdata/AgentRoutes('{route_id}')/Paw.Channel.Register"),
                    tenant,
                    api_key,
                    serde_json::json!({
                        "binding_tier": "global",
                        "channel_id": "",
                        "guild_id": "",
                        "match_pattern": "",
                        "agent_config": agent_config.to_string(),
                        "agent_id": target_agent_id,
                    }),
                )
                .await
                .ok();
                tracing::info!(
                    "  Created global AgentRoute {route_id} with agent '{agent_name}' ({target_agent_id})"
                );
            }
        }
    }

    Ok(())
}

fn default_agent_config(
    api_url: &str,
    api_key: &Option<String>,
    llm_provider: &str,
) -> serde_json::Value {
    let default_model = match llm_provider {
        "openai" | "openai_codex" => {
            std::env::var("LLM_MODEL").unwrap_or_else(|_| "gpt-5.4".to_string())
        }
        "openrouter" => {
            std::env::var("LLM_MODEL").unwrap_or_else(|_| "anthropic/claude-sonnet-4.6".to_string())
        }
        _ => std::env::var("LLM_MODEL").unwrap_or_else(|_| "claude-sonnet-4-6".to_string()),
    };
    let mut config = serde_json::json!({
        "model": default_model,
        "provider": llm_provider,
        "tools_enabled": DEFAULT_AGENT_TOOLS_ENABLED,
        "workdir": DEFAULT_AGENT_WORKDIR,
        "max_turns": "24",
        "temper_api_url": api_url,
        "max_follow_ups": "8",
    });
    if let Some(key) = api_key {
        config["temper_api_key"] = serde_json::Value::String(key.clone());
    }
    config
}

fn repaired_agent_config(
    raw: &str,
    api_url: &str,
    api_key: &Option<String>,
    is_global_route: bool,
) -> Option<String> {
    let original = raw.trim();
    let mut config = if original.is_empty() {
        serde_json::Map::new()
    } else {
        serde_json::from_str::<serde_json::Value>(original)
            .ok()
            .and_then(|value| value.as_object().cloned())
            .unwrap_or_default()
    };

    let original_normalized = serde_json::to_string(&config).ok();
    // Use the provider already stored in the config if available, otherwise fall back to env/default.
    let existing_provider = config
        .get("provider")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let provider_for_defaults = if existing_provider.is_empty() {
        std::env::var("LLM_PROVIDER").unwrap_or_else(|_| "anthropic".to_string())
    } else {
        existing_provider.to_string()
    };
    let defaults = default_agent_config(api_url, api_key, &provider_for_defaults);
    let normalized_tools = normalize_tools_enabled(
        config
            .get("tools_enabled")
            .and_then(|value| value.as_str())
            .unwrap_or(""),
        is_global_route,
    );
    let current_workdir = config
        .get("workdir")
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .to_string();

    let needs_repair = is_global_route
        || normalized_tools.is_some()
        || original.is_empty()
        || normalize_legacy_workdir(&current_workdir).is_some();
    if !needs_repair {
        return None;
    }

    if !config.contains_key("model") {
        config.insert("model".to_string(), defaults["model"].clone());
    }
    if !config.contains_key("provider") {
        config.insert("provider".to_string(), defaults["provider"].clone());
    }
    config.insert(
        "temper_api_url".to_string(),
        serde_json::Value::String(api_url.to_string()),
    );
    if let Some(key) = api_key {
        config.insert(
            "temper_api_key".to_string(),
            serde_json::Value::String(key.clone()),
        );
    }
    if let Some(normalized_workdir) = normalize_legacy_workdir(&current_workdir) {
        config.insert(
            "workdir".to_string(),
            serde_json::Value::String(normalized_workdir),
        );
    }
    if is_global_route {
        config.insert(
            "tools_enabled".to_string(),
            serde_json::Value::String(DEFAULT_AGENT_TOOLS_ENABLED.to_string()),
        );
    } else if let Some(tokens) = normalized_tools {
        config.insert(
            "tools_enabled".to_string(),
            serde_json::Value::String(tokens),
        );
    }

    let repaired = serde_json::to_string(&config).ok()?;
    if original == repaired || original_normalized.as_deref() == Some(&repaired) {
        None
    } else {
        Some(repaired)
    }
}

fn normalize_tools_enabled(raw: &str, replace_all: bool) -> Option<String> {
    if raw.trim().is_empty() {
        return Some(DEFAULT_AGENT_TOOLS_ENABLED.to_string());
    }

    let mut changed = replace_all;
    let mut tokens = Vec::new();
    for token in raw
        .split(',')
        .map(str::trim)
        .filter(|token| !token.is_empty())
    {
        let normalized = match token {
            "read_entity" => {
                changed = true;
                Some("temper_get")
            }
            "save_memory" => {
                changed = true;
                Some("temper_save_memory")
            }
            "recall_memory" => {
                changed = true;
                Some("temper_recall_memory")
            }
            "spawn_agent" => {
                changed = true;
                Some("temper_spawn_session")
            }
            "spawn_session" => {
                changed = true;
                Some("temper_spawn_session")
            }
            "temper_file_upload" => {
                changed = true;
                Some("temper_write")
            }
            "temper_get_agent_id" | "temper_done" | "temper_switch_provider" => {
                changed = true;
                None
            }
            other => Some(other),
        };

        if let Some(token) = normalized {
            if !tokens.iter().any(|existing| existing == token) {
                tokens.push(token.to_string());
            }
        }
    }

    if replace_all {
        return Some(DEFAULT_AGENT_TOOLS_ENABLED.to_string());
    }
    if changed {
        if tokens.is_empty() {
            Some(DEFAULT_AGENT_TOOLS_ENABLED.to_string())
        } else {
            Some(tokens.join(","))
        }
    } else {
        None
    }
}

fn normalize_legacy_workdir(current_workdir: &str) -> Option<String> {
    if current_workdir.is_empty() {
        return Some(DEFAULT_AGENT_WORKDIR.to_string());
    }

    if let Some(suffix) = current_workdir.strip_prefix("/tmp/workspace") {
        return Some(format!("{DEFAULT_AGENT_WORKDIR}{suffix}"));
    }

    if let Some(name) = current_workdir.strip_prefix("/tmp/openpaw-") {
        return Some(format!("{DEFAULT_AGENT_WORKDIR}/openpaw-{name}"));
    }

    None
}

/// OData GET helper with tenant + admin auth headers.
async fn odata_get(
    client: &reqwest::Client,
    url: &str,
    tenant: &str,
    api_key: &Option<String>,
) -> Result<serde_json::Value> {
    let mut req = client
        .get(url)
        .header("x-tenant-id", tenant)
        .header("x-temper-principal-kind", "admin");
    if let Some(key) = api_key {
        req = req.header("authorization", format!("Bearer {key}"));
    }
    let resp = req.send().await.context("OData GET failed")?;
    let status = resp.status();
    let body = resp.text().await.context("Failed to read response")?;
    if !status.is_success() {
        anyhow::bail!("OData GET {url} returned {status}: {body}");
    }
    serde_json::from_str(&body).context("Failed to parse JSON response")
}

/// OData POST helper with tenant + admin auth headers.
async fn odata_post(
    client: &reqwest::Client,
    url: &str,
    tenant: &str,
    api_key: &Option<String>,
    body: serde_json::Value,
) -> Result<serde_json::Value> {
    let mut req = client
        .post(url)
        .header("x-tenant-id", tenant)
        .header("x-temper-principal-kind", "admin")
        .header("content-type", "application/json")
        .json(&body);
    if let Some(key) = api_key {
        req = req.header("authorization", format!("Bearer {key}"));
    }
    let resp = req.send().await.context("OData POST failed")?;
    let status = resp.status();
    let text = resp.text().await.context("Failed to read response")?;
    if !status.is_success() {
        anyhow::bail!("OData POST {url} returned {status}: {text}");
    }
    Ok(serde_json::from_str(&text).unwrap_or(serde_json::Value::Null))
}

async fn odata_put_bytes(
    client: &reqwest::Client,
    url: &str,
    tenant: &str,
    api_key: &Option<String>,
    content_type: &str,
    body: Vec<u8>,
) -> Result<()> {
    let mut req = client
        .put(url)
        .header("x-tenant-id", tenant)
        .header("x-temper-principal-kind", "admin")
        .header("content-type", content_type)
        .body(body);
    if let Some(key) = api_key {
        req = req.header("authorization", format!("Bearer {key}"));
    }

    let resp = req.send().await.context("OData PUT failed")?;
    let status = resp.status();
    let text = resp.text().await.context("Failed to read PUT response")?;
    if !status.is_success() {
        anyhow::bail!("OData PUT {url} returned {status}: {text}");
    }
    Ok(())
}

async fn odata_get_text(
    client: &reqwest::Client,
    url: &str,
    tenant: &str,
    api_key: &Option<String>,
) -> Result<String> {
    let mut req = client
        .get(url)
        .header("x-tenant-id", tenant)
        .header("x-temper-principal-kind", "admin");
    if let Some(key) = api_key {
        req = req.header("authorization", format!("Bearer {key}"));
    }

    let resp = req.send().await.context("OData text GET failed")?;
    let status = resp.status();
    let body = resp.text().await.context("Failed to read text response")?;
    if !status.is_success() {
        anyhow::bail!("OData GET {url} returned {status}: {body}");
    }
    Ok(body)
}

fn entity_id_from_json(value: &serde_json::Value) -> Option<&str> {
    value
        .get("entity_id")
        .and_then(serde_json::Value::as_str)
        .or_else(|| value.get("Id").and_then(serde_json::Value::as_str))
        .or_else(|| {
            value
                .get("fields")
                .and_then(|fields| fields.get("Id"))
                .and_then(serde_json::Value::as_str)
        })
}

fn entity_field_str<'a>(value: &'a serde_json::Value, names: &[&str]) -> Option<&'a str> {
    names.iter().find_map(|name| {
        value
            .get(*name)
            .and_then(serde_json::Value::as_str)
            .or_else(|| {
                value
                    .get("fields")
                    .and_then(|fields| fields.get(*name))
                    .and_then(serde_json::Value::as_str)
            })
    })
}

fn build_missing_wasm_modules(os_apps_dir: &Path, startup_apps: &[String]) -> Result<()> {
    for build_script in wasm_build_scripts(os_apps_dir, startup_apps)? {
        let build_dir = build_script
            .parent()
            .context("build.sh path missing parent directory")?;
        if !wasm_build_needed(build_dir)? {
            continue;
        }

        tracing::info!(path = %build_script.display(), "Building local WASM modules");
        let script_name = build_script
            .file_name()
            .and_then(OsStr::to_str)
            .context("build.sh path missing file name")?;
        let status = std::process::Command::new("bash")
            .arg(script_name)
            .current_dir(build_dir)
            .status()
            .with_context(|| format!("Failed to run {}", build_script.display()))?;
        if !status.success() {
            anyhow::bail!("{} exited with status {status}", build_script.display());
        }
    }

    Ok(())
}

fn wasm_build_scripts(os_apps_dir: &Path, startup_apps: &[String]) -> Result<Vec<PathBuf>> {
    let mut scripts = Vec::new();
    let startup_app_set: HashSet<&str> = startup_apps.iter().map(String::as_str).collect();

    for app_entry in std::fs::read_dir(os_apps_dir)? {
        let app_dir = match app_entry {
            Ok(entry) if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) => entry.path(),
            _ => continue,
        };
        let app_name = app_dir
            .file_name()
            .and_then(OsStr::to_str)
            .unwrap_or_default();
        if !startup_app_set.contains(app_name) {
            continue;
        }
        let wasm_dir = app_dir.join("wasm");
        if !wasm_dir.is_dir() {
            continue;
        }

        let root_build = wasm_dir.join("build.sh");
        if root_build.is_file() {
            scripts.push(root_build);
            continue;
        }

        for child in std::fs::read_dir(&wasm_dir)? {
            let child_dir = match child {
                Ok(entry) if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) => {
                    entry.path()
                }
                _ => continue,
            };
            let child_build = child_dir.join("build.sh");
            if child_build.is_file() {
                scripts.push(child_build);
            }
        }
    }

    scripts.sort();
    Ok(scripts)
}

fn wasm_build_needed(build_dir: &Path) -> Result<bool> {
    if build_dir.join("Cargo.toml").is_file() {
        let module_name = build_dir
            .file_name()
            .and_then(OsStr::to_str)
            .unwrap_or_default();
        return Ok(find_wasm_binary(build_dir, module_name).is_none());
    }

    for child in std::fs::read_dir(build_dir)? {
        let child_dir = match child {
            Ok(entry) if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) => entry.path(),
            _ => continue,
        };
        if !child_dir.join("Cargo.toml").is_file() {
            continue;
        }

        let module_name = child_dir
            .file_name()
            .and_then(OsStr::to_str)
            .unwrap_or_default();
        if find_wasm_binary(&child_dir, module_name).is_none() {
            return Ok(true);
        }
    }

    Ok(false)
}

fn find_wasm_binary(module_dir: &Path, module_name: &str) -> Option<PathBuf> {
    if module_name.is_empty() {
        return None;
    }

    // Check both wasm32-unknown-unknown and wasm32-wasip1 targets.
    // WASI modules (e.g., monty_repl) compile to wasip1; all others
    // use unknown-unknown. The Temper WASM engine auto-detects which
    // linker to use based on the module's imports.
    let release_dir = module_dir.join("target/wasm32-unknown-unknown/release");
    let wasi_release_dir = module_dir.join("target/wasm32-wasip1/release");
    let candidates = [
        release_dir.join(format!("{module_name}.wasm")),
        release_dir.join(format!("{}.wasm", module_name.replace('_', "-"))),
        wasi_release_dir.join(format!("{module_name}.wasm")),
        wasi_release_dir.join(format!("{}.wasm", module_name.replace('_', "-"))),
        module_dir.join(format!("{module_name}.wasm")),
        module_dir.join(format!("{}.wasm", module_name.replace('_', "-"))),
    ];

    candidates.into_iter().find(|path| path.is_file())
}

/// Spawn the webhook trigger (HTTP endpoint for external webhooks).
///
/// Listens on port+12 for POST /triggers/webhook/{route_key}.
/// ONE entity, ONE action — everything else is WASM integrations.
fn spawn_webhook_trigger(tenant: &str, port: u16, api_key: Option<String>) {
    use paw_transport::PawApiConfig;
    use paw_transport::webhook::{WebhookTrigger, WebhookTriggerConfig};

    let tenant = tenant.to_string();
    let api_url = format!("http://127.0.0.1:{port}");
    let trigger_port = port + 12;
    tracing::info!("Webhook trigger: listening on port {trigger_port} (tenant={tenant})");

    tokio::spawn(async move {
        let api = paw_transport::PawApiClient::new(PawApiConfig {
            base_url: api_url,
            tenant,
            api_key,
        });
        let config = WebhookTriggerConfig { port: trigger_port };
        let trigger = WebhookTrigger::new(config, api);
        if let Err(e) = trigger.run().await {
            tracing::error!("Webhook trigger fatal error: {e}");
        }
    });
}

fn actor_passivation_check_interval_secs(raw: Option<&str>) -> u64 {
    raw.and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(60)
        .clamp(1, 86_400)
}

fn spawn_actor_passivation_loop(state: &PlatformState) {
    let interval_secs = actor_passivation_check_interval_secs(
        std::env::var("TEMPER_PASSIVATION_CHECK_INTERVAL")
            .ok()
            .as_deref(),
    );

    let server = state.server.clone();
    tokio::spawn(async move {
        // determinism-ok: background task for resource management
        let mut ticker = tokio::time::interval(Duration::from_secs(interval_secs));
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        ticker.tick().await; // consume immediate tick

        loop {
            ticker.tick().await;
            server.passivate_idle_actors().await;
        }
    });
}

// Transport spawning is now handled by TransportManager (see transport_manager.rs).

#[cfg(test)]
mod tests {
    use axum::Router;
    use axum::body::Body;
    use axum::extract::State;
    use axum::http::{Method, Request, StatusCode};
    use axum::response::IntoResponse;
    use axum::routing::any;
    use std::path::Path;
    use std::sync::{Arc, Mutex};

    use anyhow::anyhow;
    use serde_json::Value;
    use temper_runtime::tenant::TenantId;

    use super::{
        LocalWasmStartupPolicy, RuntimeRecoveryStep, actor_passivation_check_interval_secs,
        bootstrap_soul, load_or_create_temper_api_key, local_wasm_startup_policy,
        paw_soul_content_is_personalized, runtime_recovery_plan, soul_lookup_filters,
        spawn_runtime_server, startup_discord_connect_result, startup_os_apps,
        wait_for_runtime_server,
    };

    #[test]
    fn actor_passivation_interval_defaults_and_clamps() {
        assert_eq!(actor_passivation_check_interval_secs(None), 60);
        assert_eq!(actor_passivation_check_interval_secs(Some("0")), 1);
        assert_eq!(actor_passivation_check_interval_secs(Some("garbage")), 60);
        assert_eq!(actor_passivation_check_interval_secs(Some("5")), 5);
        assert_eq!(
            actor_passivation_check_interval_secs(Some("999999")),
            86_400
        );
    }

    #[test]
    fn runtime_recovery_finishes_query_plane_before_post_boot_tasks() {
        let tenants = vec![TenantId::new("default"), TenantId::new("temper-system")];
        let plan = runtime_recovery_plan(&tenants);

        assert_eq!(
            plan,
            vec![
                RuntimeRecoveryStep::PopulateIndex("default".to_string()),
                RuntimeRecoveryStep::PopulateIndex("temper-system".to_string()),
                RuntimeRecoveryStep::PopulateFieldIndex("default".to_string()),
                RuntimeRecoveryStep::PopulateFieldIndex("temper-system".to_string()),
            ]
        );
    }

    #[test]
    fn local_wasm_policy_defaults_and_overrides() {
        assert_eq!(
            local_wasm_startup_policy(Some("load-only")),
            LocalWasmStartupPolicy::LoadPersistedOnly
        );
        assert_eq!(
            local_wasm_startup_policy(Some("build")),
            LocalWasmStartupPolicy::BuildIfMissing
        );
        assert_eq!(
            local_wasm_startup_policy(Some("0")),
            LocalWasmStartupPolicy::LoadPersistedOnly
        );
        assert_eq!(
            local_wasm_startup_policy(Some("1")),
            LocalWasmStartupPolicy::BuildIfMissing
        );
        assert_eq!(
            local_wasm_startup_policy(None),
            LocalWasmStartupPolicy::LoadPersistedOnly
        );
    }

    #[test]
    fn startup_discord_connect_result_keeps_success() {
        assert_eq!(
            startup_discord_connect_result(Ok("https://example.com/discord/interaction".into())),
            Some("https://example.com/discord/interaction".into())
        );
    }

    #[test]
    fn startup_discord_connect_result_drops_failure() {
        assert_eq!(startup_discord_connect_result(Err(anyhow!("boom"))), None);
    }

    #[test]
    fn startup_os_apps_includes_core_apps() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        temper_platform::os_apps::set_os_apps_dir(repo_root.join("os-apps"));
        let apps = startup_os_apps();
        for expected in ["paw-agent", "paw-channels", "paw-fs", "paw-research"] {
            assert!(
                apps.iter().any(|app| app == expected),
                "expected startup OS app {expected} to be present in {apps:?}"
            );
        }
    }

    #[test]
    fn datadog_configs_use_tenant_aware_entity_queries() {
        let repo_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let dashboard_path = repo_root.join("dd-dashboards/openpaw-overview.json");
        let monitor_path = repo_root.join("dd-monitors/openpaw-monitors.json");

        let dashboard: Value =
            serde_json::from_str(&std::fs::read_to_string(&dashboard_path).unwrap()).unwrap();
        let monitors: Value =
            serde_json::from_str(&std::fs::read_to_string(&monitor_path).unwrap()).unwrap();

        let indexed_entities_query = dashboard["widgets"]
            .as_array()
            .unwrap()
            .iter()
            .find_map(|widget| {
                let widgets = widget["definition"]["widgets"].as_array()?;
                widgets.iter().find_map(|inner| {
                    let definition = &inner["definition"];
                    if matches!(
                        definition["title"].as_str()?,
                        "Indexed Entities" | "Indexed Entities (Query Plane)"
                    ) {
                        definition["requests"][0]["q"].as_str()
                    } else {
                        None
                    }
                })
            })
            .expect("Entity count widget query should exist");
        assert_eq!(
            indexed_entities_query,
            "sum:temper_indexed_entities{service:openpaw,tenant:*}"
        );

        let active_actors_query = dashboard["widgets"]
            .as_array()
            .unwrap()
            .iter()
            .find_map(|widget| {
                let widgets = widget["definition"]["widgets"].as_array()?;
                widgets.iter().find_map(|inner| {
                    let definition = &inner["definition"];
                    if matches!(
                        definition["title"].as_str()?,
                        "Active Actors" | "Active Actors (Hydrated)"
                    ) {
                        definition["requests"][0]["q"].as_str()
                    } else {
                        None
                    }
                })
            })
            .expect("Active Actors widget query should exist");
        assert_eq!(
            active_actors_query,
            "avg:temper_active_actors{service:openpaw}"
        );

        let process_memory_query = dashboard["widgets"]
            .as_array()
            .unwrap()
            .iter()
            .find_map(|widget| {
                let widgets = widget["definition"]["widgets"].as_array()?;
                widgets.iter().find_map(|inner| {
                    let definition = &inner["definition"];
                    if matches!(
                        definition["title"].as_str()?,
                        "Process Memory (RSS)" | "OpenPaw Process Memory (RSS)"
                    ) {
                        definition["requests"][0]["q"].as_str()
                    } else {
                        None
                    }
                })
            })
            .expect("Process Memory widget query should exist");
        assert_eq!(
            process_memory_query,
            "avg:process_resident_memory_bytes{service:openpaw}"
        );

        let indexed_entities_by_host_query = dashboard["widgets"]
            .as_array()
            .unwrap()
            .iter()
            .find_map(|widget| {
                let widgets = widget["definition"]["widgets"].as_array()?;
                widgets.iter().find_map(|inner| {
                    let definition = &inner["definition"];
                    if definition["title"].as_str()? == "Indexed Entities by Host" {
                        definition["requests"][0]["q"].as_str()
                    } else {
                        None
                    }
                })
            })
            .expect("Indexed Entities by Host widget query should exist");
        assert_eq!(
            indexed_entities_by_host_query,
            "sum:temper_indexed_entities{service:openpaw,tenant:*} by {host}"
        );

        let active_actors_by_host_query = dashboard["widgets"]
            .as_array()
            .unwrap()
            .iter()
            .find_map(|widget| {
                let widgets = widget["definition"]["widgets"].as_array()?;
                widgets.iter().find_map(|inner| {
                    let definition = &inner["definition"];
                    if definition["title"].as_str()? == "Active Actors by Host" {
                        definition["requests"][0]["q"].as_str()
                    } else {
                        None
                    }
                })
            })
            .expect("Active Actors by Host widget query should exist");
        assert_eq!(
            active_actors_by_host_query,
            "avg:temper_active_actors{service:openpaw} by {host}"
        );

        let process_memory_by_host_query = dashboard["widgets"]
            .as_array()
            .unwrap()
            .iter()
            .find_map(|widget| {
                let widgets = widget["definition"]["widgets"].as_array()?;
                widgets.iter().find_map(|inner| {
                    let definition = &inner["definition"];
                    if definition["title"].as_str()? == "OpenPaw RSS by Host" {
                        definition["requests"][0]["q"].as_str()
                    } else {
                        None
                    }
                })
            })
            .expect("OpenPaw RSS by Host widget query should exist");
        assert_eq!(
            process_memory_by_host_query,
            "avg:process_resident_memory_bytes{service:openpaw} by {host}"
        );

        let projected_entities_query = dashboard["widgets"]
            .as_array()
            .unwrap()
            .iter()
            .find_map(|widget| {
                let widgets = widget["definition"]["widgets"].as_array()?;
                widgets.iter().find_map(|inner| {
                    let definition = &inner["definition"];
                    if matches!(
                        definition["title"].as_str()?,
                        "Projected Entities" | "Projected Entities (Durable Catalog)"
                    ) {
                        definition["requests"][0]["q"].as_str()
                    } else {
                        None
                    }
                })
            })
            .expect("Projected Entities widget query should exist");
        assert_eq!(
            projected_entities_query,
            "sum:temper_projected_entities{service:openpaw,tenant:*}"
        );

        let projection_coverage_query = dashboard["widgets"]
            .as_array()
            .unwrap()
            .iter()
            .find_map(|widget| {
                let widgets = widget["definition"]["widgets"].as_array()?;
                widgets.iter().find_map(|inner| {
                    let definition = &inner["definition"];
                    if definition["title"].as_str()? == "Projection Coverage" {
                        definition["requests"][0]["q"].as_str()
                    } else {
                        None
                    }
                })
            })
            .expect("Projection Coverage widget query should exist");
        assert_eq!(
            projection_coverage_query,
            "avg:temper_projection_coverage_ratio{service:openpaw}"
        );

        let snapshot_miss_query = dashboard["widgets"]
            .as_array()
            .unwrap()
            .iter()
            .find_map(|widget| {
                let widgets = widget["definition"]["widgets"].as_array()?;
                widgets.iter().find_map(|inner| {
                    let definition = &inner["definition"];
                    if definition["title"].as_str()? == "Projection Snapshot Misses" {
                        definition["requests"][0]["q"].as_str()
                    } else {
                        None
                    }
                })
            })
            .expect("Projection Snapshot Misses widget query should exist");
        assert_eq!(
            snapshot_miss_query,
            "default_zero(sum:temper_projection_backfill_snapshot_misses_total{service:openpaw}.as_count().rollup(sum, 60))"
        );

        let reconcile_query = dashboard["widgets"]
            .as_array()
            .unwrap()
            .iter()
            .find_map(|widget| {
                let widgets = widget["definition"]["widgets"].as_array()?;
                widgets.iter().find_map(|inner| {
                    let definition = &inner["definition"];
                    if definition["title"].as_str()? == "OS App Reconcile" {
                        definition["requests"][0]["q"].as_str()
                    } else {
                        None
                    }
                })
            })
            .expect("OS App Reconcile widget query should exist");
        assert_eq!(
            reconcile_query,
            "default_zero(sum:temper_os_app_reconcile_total{service:openpaw} by {app,result}.as_count().rollup(sum, 60))"
        );

        let reconcile_duration_query = dashboard["widgets"]
            .as_array()
            .unwrap()
            .iter()
            .find_map(|widget| {
                let widgets = widget["definition"]["widgets"].as_array()?;
                widgets.iter().find_map(|inner| {
                    let definition = &inner["definition"];
                    if definition["title"].as_str()? == "OS App Reconcile Duration" {
                        definition["requests"][0]["q"].as_str()
                    } else {
                        None
                    }
                })
            })
            .expect("OS App Reconcile Duration widget query should exist");
        assert_eq!(
            reconcile_duration_query,
            "default_zero(avg:temper_os_app_reconcile_duration_ms{service:openpaw} by {app,result}.rollup(avg, 60))"
        );

        let startup_restore_query = dashboard["widgets"]
            .as_array()
            .unwrap()
            .iter()
            .find_map(|widget| {
                let widgets = widget["definition"]["widgets"].as_array()?;
                widgets.iter().find_map(|inner| {
                    let definition = &inner["definition"];
                    if definition["title"].as_str()? == "Startup Live Restore Entities" {
                        definition["requests"][0]["q"].as_str()
                    } else {
                        None
                    }
                })
            })
            .expect("Startup Live Restore Entities widget query should exist");
        assert_eq!(
            startup_restore_query,
            "default_zero(sum:temper_startup_live_restore_entities_total{service:openpaw} by {tenant}.as_count().rollup(sum, 60))"
        );

        let session_context_tokens_query = dashboard["widgets"]
            .as_array()
            .unwrap()
            .iter()
            .find_map(|widget| {
                let widgets = widget["definition"]["widgets"].as_array()?;
                widgets.iter().find_map(|inner| {
                    let definition = &inner["definition"];
                    if definition["title"].as_str()? == "Session Context Tokens" {
                        definition["requests"][0]["q"].as_str()
                    } else {
                        None
                    }
                })
            })
            .expect("Session Context Tokens widget query should exist");
        assert_eq!(
            session_context_tokens_query,
            "avg:temper_session_context_tokens{service:openpaw} by {provider}.rollup(avg, 60)"
        );

        let session_context_bytes_query = dashboard["widgets"]
            .as_array()
            .unwrap()
            .iter()
            .find_map(|widget| {
                let widgets = widget["definition"]["widgets"].as_array()?;
                widgets.iter().find_map(|inner| {
                    let definition = &inner["definition"];
                    if definition["title"].as_str()? == "Session Context Bytes" {
                        definition["requests"][0]["q"].as_str()
                    } else {
                        None
                    }
                })
            })
            .expect("Session Context Bytes widget query should exist");
        assert_eq!(
            session_context_bytes_query,
            "avg:temper_session_context_bytes{service:openpaw} by {provider}.rollup(avg, 60)"
        );

        let provider_request_bytes_query = dashboard["widgets"]
            .as_array()
            .unwrap()
            .iter()
            .find_map(|widget| {
                let widgets = widget["definition"]["widgets"].as_array()?;
                widgets.iter().find_map(|inner| {
                    let definition = &inner["definition"];
                    if definition["title"].as_str()? == "Provider Request Bytes" {
                        definition["requests"][0]["q"].as_str()
                    } else {
                        None
                    }
                })
            })
            .expect("Provider Request Bytes widget query should exist");
        assert_eq!(
            provider_request_bytes_query,
            "avg:temper_session_provider_request_bytes{service:openpaw} by {provider}.rollup(avg, 60)"
        );

        let memory_budget_exceeded_query = dashboard["widgets"]
            .as_array()
            .unwrap()
            .iter()
            .find_map(|widget| {
                let widgets = widget["definition"]["widgets"].as_array()?;
                widgets.iter().find_map(|inner| {
                    let definition = &inner["definition"];
                    if definition["title"].as_str()? == "Session Memory Budget Exceeded" {
                        definition["requests"][0]["q"].as_str()
                    } else {
                        None
                    }
                })
            })
            .expect("Session Memory Budget Exceeded widget query should exist");
        assert_eq!(
            memory_budget_exceeded_query,
            "default_zero(sum:temper_session_memory_limit_exceeded_total{service:openpaw}.as_count().rollup(sum, 60))"
        );

        let indexed_entities_drop_query = monitors
            .as_array()
            .unwrap()
            .iter()
            .find_map(|monitor| {
                if monitor["name"].as_str()? == "[OpenPaw] Indexed Entities Drop" {
                    monitor["query"].as_str()
                } else {
                    None
                }
            })
            .expect("Indexed Entities Drop monitor query should exist");
        assert_eq!(
            indexed_entities_drop_query,
            "avg(last_15m):sum:temper_indexed_entities{service:openpaw,tenant:*} < 1"
        );

        let startup_regression_query = monitors
            .as_array()
            .unwrap()
            .iter()
            .find_map(|monitor| {
                if monitor["name"].as_str()? == "[OpenPaw] Startup Time Regression" {
                    monitor["query"].as_str()
                } else {
                    None
                }
            })
            .expect("Startup Time Regression monitor query should exist");
        assert_eq!(
            startup_regression_query,
            "avg(last_15m):avg:temper_startup_time_to_healthy_ms{service:openpaw} > 120000"
        );

        let reconcile_regression_query = monitors
            .as_array()
            .unwrap()
            .iter()
            .find_map(|monitor| {
                if monitor["name"].as_str()? == "[OpenPaw] OS App Reconcile Regression" {
                    monitor["query"].as_str()
                } else {
                    None
                }
            })
            .expect("OS App Reconcile Regression monitor query should exist");
        assert_eq!(
            reconcile_regression_query,
            "avg(last_1h):avg:temper_startup_phase_duration_ms{service:openpaw,phase:phase_6_os_app_reconcile} > 60000"
        );

        let wasm_failure_monitor_query = monitors
            .as_array()
            .unwrap()
            .iter()
            .find_map(|monitor| {
                if monitor["name"].as_str()? == "[OpenPaw] Required WASM Load Failures" {
                    monitor["query"].as_str()
                } else {
                    None
                }
            })
            .expect("Required WASM Load Failures monitor query should exist");
        assert_eq!(
            wasm_failure_monitor_query,
            "sum(last_15m):sum:temper_wasm_module_load_failures_total{service:openpaw,criticality:(platform-required OR app-required)}.as_count() > 0"
        );

        let session_memory_monitor_query = monitors
            .as_array()
            .unwrap()
            .iter()
            .find_map(|monitor| {
                if monitor["name"].as_str()? == "[OpenPaw] Session Memory Budget Exceeded" {
                    monitor["query"].as_str()
                } else {
                    None
                }
            })
            .expect("Session Memory Budget Exceeded monitor query should exist");
        assert_eq!(
            session_memory_monitor_query,
            "sum(last_15m):sum:temper_session_memory_limit_exceeded_total{service:openpaw}.as_count() > 0"
        );

        let dashboard_json = dashboard.to_string();
        assert!(
            dashboard_json.contains("avg:temper_up{service:openpaw}"),
            "Dashboard should include the metrics pipeline canary."
        );
        assert!(
            dashboard_json.contains(
                "sum:temper_cedar_evaluations_total{service:openpaw}.as_count().rollup(sum, 60)"
            ),
            "Dashboard should include Cedar evaluation volume."
        );
        assert!(
            dashboard_json.contains(
                "avg:temper_turso_query_duration{service:openpaw} by {operation}.rollup(avg, 60)"
            ),
            "Dashboard should include Turso query duration."
        );
        assert!(
            dashboard_json.contains(
                "sum:temper_wasm_host_http_requests_total{service:openpaw} by {call_kind,status_code_class}.as_count().rollup(sum, 60)"
            ),
            "Dashboard should include WASM host HTTP request volume."
        );
        assert!(
            dashboard_json.contains(
                "avg:temper_wasm_host_http_duration_ms{service:openpaw} by {call_kind,status_code_class}.rollup(avg, 60)"
            ),
            "Dashboard should include WASM host HTTP latency."
        );
        assert!(
            dashboard_json.contains(
                "avg:temper_event_replay_duration{service:openpaw} by {tenant,entity_type}.rollup(avg, 60)"
            ),
            "Dashboard should include event replay duration."
        );
        assert!(
            dashboard_json.contains(
                "avg:temper_session_context_prepare_duration_ms{service:openpaw}.rollup(avg, 60)"
            ),
            "Dashboard should include session context prepare duration."
        );
        assert!(
            dashboard_json.contains(
                "avg:temper_session_provider_response_bytes{service:openpaw} by {provider}.rollup(avg, 60)"
            ),
            "Dashboard should include provider response bytes."
        );
        assert!(
            !dashboard_json.contains("temper_startup_phase_duration_ms"),
            "Dashboard should not reference the stale startup phase duration metric."
        );
        assert!(
            !dashboard_json.contains("temper_startup_time_to_healthy_ms"),
            "Dashboard should not reference the stale startup healthy metric."
        );
        assert!(
            !dashboard_json.contains("temper_wasm_module_load_failures_total"),
            "Dashboard should not reference the stale WASM load failures metric."
        );
        assert!(
            !dashboard_json.contains("temper_wasm_module_skipped_total"),
            "Dashboard should not reference the stale WASM skipped metric."
        );
    }

    #[test]
    fn soul_lookup_filters_cover_current_and_legacy_names() {
        assert_eq!(
            soul_lookup_filters("Paw"),
            ["Name eq 'Paw'".to_string(), "name eq 'paw'".to_string()]
        );
        assert_eq!(
            soul_lookup_filters("SRE"),
            ["Name eq 'SRE'".to_string(), "name eq 'sre'".to_string()]
        );
    }

    #[test]
    fn temper_api_key_persists_and_env_overrides() {
        let tempdir = tempfile::tempdir().unwrap();
        let path = tempdir.path().join("api.key");

        let generated = load_or_create_temper_api_key(None, &path).unwrap();
        assert!(!generated.is_empty());
        assert!(path.exists());

        let reloaded = load_or_create_temper_api_key(None, &path).unwrap();
        assert_eq!(reloaded, generated);

        let explicit = load_or_create_temper_api_key(Some("env-token".to_string()), &path).unwrap();
        assert_eq!(explicit, "env-token");
    }

    #[test]
    fn paw_soul_content_personalization_detection_matches_non_default_content() {
        let default_content = crate::setup::default_paw_soul_content().expect("default content");

        assert!(!paw_soul_content_is_personalized(
            &default_content,
            &default_content
        ));
        assert!(paw_soul_content_is_personalized(
            "## Who I Am\nI am tailored for Arni.",
            &default_content
        ));
    }

    #[tokio::test]
    async fn bootstrap_soul_preserves_existing_personalized_paw_content() {
        #[derive(Clone, Default)]
        struct Seen {
            upload_attempted: Arc<Mutex<bool>>,
        }

        async fn handler(State(seen): State<Seen>, request: Request<Body>) -> impl IntoResponse {
            match (
                request.method(),
                request.uri().path(),
                request.uri().query(),
            ) {
                (&Method::GET, "/tdata/Agents('agent-1')", _) => (
                    StatusCode::OK,
                    axum::Json(serde_json::json!({
                        "fields": {
                            "soul_id": "soul-1"
                        }
                    })),
                )
                    .into_response(),
                (&Method::GET, "/tdata/Souls('soul-1')", _) => (
                    StatusCode::OK,
                    axum::Json(serde_json::json!({
                        "fields": {
                            "ContentFileId": "file-1"
                        }
                    })),
                )
                    .into_response(),
                (&Method::GET, "/tdata/Files('file-1')/$value", _) => (
                    StatusCode::OK,
                    "## Who I Am\nI am tailored for Arni.".to_string(),
                )
                    .into_response(),
                (&Method::PUT, "/tdata/Files('file-1')/$value", _) => {
                    *seen.upload_attempted.lock().unwrap() = true;
                    StatusCode::OK.into_response()
                }
                _ => StatusCode::NOT_FOUND.into_response(),
            }
        }

        let temp = tempfile::tempdir().unwrap();
        let soul_path = temp.path().join("SOUL.md");
        std::fs::write(&soul_path, "# Default soul").unwrap();

        let seen = Seen::default();
        let app = Router::new()
            .fallback(any(handler))
            .with_state(seen.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let soul_id = bootstrap_soul(
            &reqwest::Client::new(),
            &format!("http://{addr}"),
            "default",
            &None,
            "agent-1",
            "Paw",
            "Paw soul",
            &[soul_path.to_str().unwrap()],
            false,
        )
        .await
        .unwrap();

        assert_eq!(soul_id, "soul-1");
        assert!(!*seen.upload_attempted.lock().unwrap());
    }

    #[tokio::test]
    async fn spawn_runtime_server_accepts_requests_before_transport_boot() {
        use axum::{Router, routing::get};
        use std::time::Duration;

        let app = Router::new().route("/readyz", get(|| async { "ok" }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server_handle = spawn_runtime_server(listener, app);
        wait_for_runtime_server(
            format!("http://{addr}/readyz").as_str(),
            Duration::from_secs(2),
        )
        .await
        .expect("runtime server should be reachable before transport boot");

        server_handle.abort();
    }
}
