//! Open Paw 9-phase startup sequence.
//!
//! Replicates the Temper CLI's boot flow (`temper serve`) in an embedded context.
//! The daemon boots the Temper platform, installs Paw OS apps, seeds souls,
//! and starts the Discord transport.

use std::collections::HashSet;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use temper_platform::PlatformState;
use temper_platform::os_apps::get_os_app;
use temper_platform::recovery::{recover_cedar_policies, restore_installed_skills};
use temper_platform::router::build_platform_router;
use temper_runtime::scheduler::sim_now;
use temper_runtime::tenant::TenantId;
use temper_server::event_store::ServerEventStore;
use temper_server::registry::{EntityLevelSummary, EntityVerificationResult, VerificationStatus};
use temper_server::registry_bootstrap::restore_registry_from_turso;
use temper_store_turso::{TursoEventStore, TursoSpecVerificationUpdate};

use crate::config::Config;

/// Paw OS apps to install at startup.
const PAW_OS_APPS: &[&str] = &[
    "paw-agent",
    "paw-channels",
    "paw-fs",
    "paw-pm",
    "paw-compute",
    "paw-harness",
    "paw-heal",
    "paw-ingest",
    "paw-research",
    "paw-foresight",
    // Kotowari teaching platform apps
    "koto-learn",
    "koto-tutor",
    "koto-wiki",
    // Deep Sci-Fi reference apps (registered via add_os_apps_dir)
    "dsf-harness",
    // Katagami design language library
    "katagami-commons",
    "katagami-curation",
];

const DEFAULT_AGENT_TOOLS_ENABLED: &str = "temper_create,temper_get,temper_list,temper_action,temper_patch,temper_submit_specs,temper_show_spec,temper_specs,temper_upload_wasm,temper_get_trajectories,temper_get_insights,temper_get_decisions,temper_poll_decision,temper_approve_decision,temper_deny_decision,temper_submit_policy,temper_list_policies,temper_get_policy,temper_update_policy,temper_delete_policy,temper_install_app,temper_list_apps,temper_spawn_session,temper_list_sessions,temper_abort_session,temper_steer_session,temper_save_memory,temper_recall_memory,temper_write,temper_read,temper_run_coding_agent,temper_get_secret,temper_datadog_query,temper_railway,temper_vercel,temper_web_search,temper_web_fetch,read,write,edit,bash";
const DEFAULT_AGENT_WORKDIR: &str = "/workspace";

/// Run the Open Paw daemon.
///
/// If `force_soul_setup` is true, the soul personalization interview runs
/// after boot regardless of current configuration (used by `openpaw setup`).
pub async fn run(mut config: Config, force_soul_setup: bool) -> Result<()> {
    let port = config.port;
    let tenant = config.tenant.clone();
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    let data_dir = Path::new(&home).join(".local/share/openpaw");
    std::fs::create_dir_all(&data_dir)
        .with_context(|| format!("Failed to create data dir: {}", data_dir.display()))?;

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
    {
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
        let vault = temper_server::secrets::vault::SecretsVault::new(&key_bytes);
        state.server.secrets_vault = Some(Arc::new(vault));
    }

    // Phase 5b: Restore secrets from Turso (before env seeding so env vars take priority)
    if let Some(ref vault) = state.server.secrets_vault {
        restore_secrets_from_turso(vault, &turso_store, "default").await;
        if tenant != "default" {
            restore_secrets_from_turso(vault, &turso_store, &tenant).await;
        }
    }

    // Phase 5c: Seed secrets from env (env vars override Turso-stored values)
    //
    // Each secret is cached in-memory AND persisted to Turso so it survives
    // restarts even if the env var is later removed.
    if let Some(ref vault) = state.server.secrets_vault {
        /// Helper to seed a secret from an optional env value into both tenants.
        macro_rules! seed_secret {
            ($vault:expr, $store:expr, $tenant:expr, $key:expr, $value:expr) => {
                if let Some(ref val) = $value {
                    cache_and_persist_secret($vault, $store, "default", $key, val.clone()).await;
                    if $tenant != "default" {
                        cache_and_persist_secret($vault, $store, $tenant, $key, val.clone()).await;
                    }
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
            "modal_api_token",
            config.modal_api_token
        );
        seed_secret!(
            vault,
            &turso_store,
            &tenant,
            "modal_api_url",
            config.modal_api_url
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
            "vercel_token",
            config.vercel_token
        );

        // dd_site always has a value (defaults to "datadoghq.com")
        cache_and_persist_secret(
            vault,
            &turso_store,
            "default",
            "dd_site",
            config.dd_site.clone(),
        )
        .await;
        if tenant != "default" {
            cache_and_persist_secret(
                vault,
                &turso_store,
                &tenant,
                "dd_site",
                config.dd_site.clone(),
            )
            .await;
        }

        // temper_api_url — always set to local server
        let api_url = format!("http://127.0.0.1:{actual_port}");
        let _ = vault.cache_secret("default", "temper_api_url", api_url.clone());
        if tenant != "default" {
            let _ = vault.cache_secret(&tenant, "temper_api_url", api_url);
        }

        // Sandbox URL: explicit override for testing, otherwise Tensorlake provisions on demand.
        if let Some(sandbox_url) = std::env::var("SANDBOX_URL").ok().filter(|s| !s.is_empty()) {
            cache_and_persist_secret(
                vault,
                &turso_store,
                "default",
                "sandbox_url",
                sandbox_url.clone(),
            )
            .await;
            if tenant != "default" {
                cache_and_persist_secret(vault, &turso_store, &tenant, "sandbox_url", sandbox_url)
                    .await;
            }
        } else {
            let provider = config.sandbox_provider.as_deref().unwrap_or("tensorlake");
            match provider {
                "tensorlake" if config.tensorlake_api_key.is_some() => {
                    tracing::info!("Sandbox provider: tensorlake (API key configured)");
                }
                "modal" if config.modal_api_token.is_some() && config.modal_api_url.is_some() => {
                    tracing::info!(
                        "Sandbox provider: modal (bridge URL: {})",
                        config.modal_api_url.as_deref().unwrap_or("?")
                    );
                }
                "modal" => {
                    tracing::warn!(
                        "Sandbox provider is 'modal' but MODAL_API_TOKEN or MODAL_API_URL not set"
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
        let _ = vault.cache_secret("default", "blob_endpoint", blob_endpoint.clone());
        let _ = vault.cache_secret("default", "blob_bucket", blob_bucket.clone());
        if tenant != "default" {
            let _ = vault.cache_secret(&tenant, "blob_endpoint", blob_endpoint);
            let _ = vault.cache_secret(&tenant, "blob_bucket", blob_bucket);
        }

        // HMAC credentials for GCS (or any S3-compatible blob store).
        if let Ok(key) = std::env::var("BLOB_ACCESS_KEY") {
            cache_and_persist_secret(
                vault,
                &turso_store,
                "default",
                "blob_access_key",
                key.clone(),
            )
            .await;
            if tenant != "default" {
                cache_and_persist_secret(vault, &turso_store, &tenant, "blob_access_key", key)
                    .await;
            }
        }
        if let Ok(key) = std::env::var("BLOB_SECRET_KEY") {
            cache_and_persist_secret(
                vault,
                &turso_store,
                "default",
                "blob_secret_key",
                key.clone(),
            )
            .await;
            if tenant != "default" {
                cache_and_persist_secret(vault, &turso_store, &tenant, "blob_secret_key", key)
                    .await;
            }
        }
    }

    // Phase 6: Install Paw OS apps
    tracing::info!("Phase 6: Installing Paw OS apps...");
    for app_name in PAW_OS_APPS {
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

    if let Err(error) = build_and_register_local_wasm_modules(&state, &tenant, &os_apps_dir).await {
        tracing::error!(%error, "Failed to build/register local OS app WASM modules");
    }

    // Phase 7: Recovery (Cedar policies + WASM modules + secrets from store)
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
    for tenant_id in &tenant_ids {
        state.server.populate_index_from_store(tenant_id).await;
    }

    // Background: populate field index from snapshots for OData filter push-down.
    {
        let server = state.server.clone();
        let tenant_ids = tenant_ids.clone();
        tokio::spawn(async move {
            for tenant_id in tenant_ids {
                server
                    .populate_field_index_from_snapshots(&tenant_id)
                    .await;
            }
        });
    }

    // Phase 7b: Session recovery — recover or fail orphaned sessions (ADR-0025)
    {
        let terminal_states: HashSet<&str> =
            ["Completed", "Failed", "Cancelled"].into_iter().collect();
        let recoverable_states: HashSet<&str> =
            ["Thinking", "Executing", "Compacting", "Steering", "WaitingForApproval"]
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
    let router = build_platform_router(state.clone());
    let setup_state = crate::setup_api::SetupApiState {
        platform: state.clone(),
        turso_store: turso_store.clone(),
        transport_manager: transport_manager.clone(),
        tenant: tenant.clone(),
        agents_dir: PathBuf::from("os-apps/paw-agent/agents"),
    };
    let router = router.merge(crate::setup_api::router(setup_state));

    // Serve the dashboard SPA from dashboard/build if available.
    let router = if std::path::Path::new("dashboard/build").exists() {
        use tower_http::services::ServeDir;
        router.nest_service("/dashboard", ServeDir::new("dashboard/build"))
    } else {
        router
    };

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

            let interaction_url = transport_manager
                .connect_discord(crate::transport_manager::DiscordConnectParams {
                    bot_token: token,
                    public_key,
                    guild_id: guild_id.clone(),
                    feed_channel_id: feed_channel_id.clone(),
                    forum_channel_id: forum_channel_id.clone(),
                })
                .await?;
            tracing::info!(%interaction_url, "Discord transport ready");

            // Spawn Discord observer (SSE → Discord feed/forum).
            if feed_channel_id.is_some() || forum_channel_id.is_some() {
                let bot_token_for_observer = vault
                    .and_then(|v| v.get_secret(&tenant, "discord_bot_token"))
                    .unwrap_or_default();
                let observer_api = paw_transport::PawApiClient::new(paw_transport::PawApiConfig {
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
                        paw_transport::discord::run_observer(observer_api, observer_config).await
                    {
                        tracing::error!("Discord observer failed: {e}");
                    }
                });
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

    spawn_soul_bootstrap(actual_port, tenant.clone(), config.temper_api_key.clone());

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

        // Spawn server in background so OData is available during soul setup
        let serve_handle = tokio::spawn(async move { axum::serve(listener, router).await });

        // Give the server a moment to accept connections
        tokio::time::sleep(Duration::from_millis(500)).await;

        if let Err(e) =
            crate::setup::run_setup_soul(actual_port, &api_key, &provider_name, &tenant).await
        {
            tracing::warn!("Soul setup failed: {e}");
        }

        serve_handle.await??;
    } else {
        axum::serve(listener, router).await?;
    }

    Ok(())
}

/// Cache a secret in the in-memory vault AND persist it to Turso so it survives restarts.
async fn cache_and_persist_secret(
    vault: &temper_server::secrets::vault::SecretsVault,
    store: &TursoEventStore,
    tenant: &str,
    key: &str,
    value: String,
) {
    let _ = vault.cache_secret(tenant, key, value.clone());
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

/// Restore secrets from Turso into the in-memory vault.
async fn restore_secrets_from_turso(
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
                            let _ = vault.cache_secret(tenant, &key_name, value);
                            restored += 1;
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
fn spawn_soul_bootstrap(port: u16, tenant: String, api_key: Option<String>) {
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

        let default_config = default_agent_config(&api_url);

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
                    name,
                    description,
                    paths,
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
    name: &str,
    description: &str,
    paths: &[&str],
) -> Result<String> {
    let content = paths
        .iter()
        .map(|p| {
            std::fs::read_to_string(p).with_context(|| format!("Failed to read soul file: {p}"))
        })
        .collect::<Result<Vec<_>>>()?
        .join("\n\n");

    let escaped_name = name.replace('\'', "''");
    let filter = format!("name eq '{escaped_name}'");
    let list_url = format!("{api_url}/tdata/Souls?$filter={filter}");
    let resp = odata_get(client, &list_url, tenant, api_key).await?;
    let items = resp["value"].as_array();

    if let Some(items) = items {
        if let Some(existing) = items.first() {
            let id = entity_id_from_json(existing).unwrap_or("unknown");
            if let Some(file_id) = entity_field_str(existing, &["ContentFileId", "content_file_id"])
            {
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
                    repaired_agent_config(current_config, api_url, channel_id.is_empty())
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
                let agent_config = default_agent_config(api_url);
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

fn default_agent_config(api_url: &str) -> serde_json::Value {
    let default_model =
        std::env::var("LLM_MODEL").unwrap_or_else(|_| "claude-sonnet-4-6".to_string());
    let default_provider =
        std::env::var("LLM_PROVIDER").unwrap_or_else(|_| "anthropic".to_string());
    serde_json::json!({
        "model": default_model,
        "provider": default_provider,
        "tools_enabled": DEFAULT_AGENT_TOOLS_ENABLED,
        "workdir": DEFAULT_AGENT_WORKDIR,
        "max_turns": "24",
        "temper_api_url": api_url,
        "max_follow_ups": "8",
    })
}

fn repaired_agent_config(raw: &str, api_url: &str, is_global_route: bool) -> Option<String> {
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
    let defaults = default_agent_config(api_url);
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

async fn build_and_register_local_wasm_modules(
    state: &PlatformState,
    tenant: &str,
    os_apps_dir: &Path,
) -> Result<()> {
    build_missing_wasm_modules(os_apps_dir)?;

    let tenant_id = TenantId::new(tenant);
    let mut registered = 0usize;

    for module_dir in wasm_module_dirs(os_apps_dir)? {
        let module_name = module_dir
            .file_name()
            .and_then(OsStr::to_str)
            .unwrap_or_default();
        if module_name.is_empty() {
            continue;
        }

        let Some(wasm_path) = find_wasm_binary(&module_dir, module_name) else {
            continue;
        };

        let wasm_bytes = std::fs::read(&wasm_path)
            .with_context(|| format!("Failed to read WASM binary: {}", wasm_path.display()))?;
        let hash = state
            .server
            .wasm_engine
            .compile_and_cache(&wasm_bytes)
            .map_err(|error| {
                anyhow::anyhow!(
                    "Failed to compile WASM module '{module_name}' from {}: {error}",
                    wasm_path.display()
                )
            })?;
        state
            .server
            .upsert_wasm_module(tenant, module_name, &wasm_bytes, &hash)
            .await
            .map_err(|error| {
                anyhow::anyhow!("Failed to persist WASM module '{module_name}': {error}")
            })?;
        {
            let mut registry = state.server.wasm_module_registry.write().unwrap();
            registry.register(&tenant_id, module_name, &hash);
        }

        registered += 1;
        tracing::info!(
            module = module_name,
            path = %wasm_path.display(),
            hash = %hash,
            "Registered local WASM module"
        );
    }

    tracing::info!("Registered {registered} local WASM modules for tenant '{tenant}'");
    Ok(())
}

fn build_missing_wasm_modules(os_apps_dir: &Path) -> Result<()> {
    for build_script in wasm_build_scripts(os_apps_dir)? {
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

fn wasm_build_scripts(os_apps_dir: &Path) -> Result<Vec<PathBuf>> {
    let mut scripts = Vec::new();

    for app_entry in std::fs::read_dir(os_apps_dir)? {
        let app_dir = match app_entry {
            Ok(entry) if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) => entry.path(),
            _ => continue,
        };
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

fn wasm_module_dirs(os_apps_dir: &Path) -> Result<Vec<PathBuf>> {
    let mut dirs = Vec::new();

    for app_entry in std::fs::read_dir(os_apps_dir)? {
        let app_dir = match app_entry {
            Ok(entry) if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) => entry.path(),
            _ => continue,
        };
        let wasm_dir = app_dir.join("wasm");
        if !wasm_dir.is_dir() {
            continue;
        }

        for child in std::fs::read_dir(&wasm_dir)? {
            let child_dir = match child {
                Ok(entry) if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) => {
                    entry.path()
                }
                _ => continue,
            };
            if child_dir.join("Cargo.toml").is_file() {
                dirs.push(child_dir);
            }
        }
    }

    dirs.sort();
    Ok(dirs)
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

// Transport spawning is now handled by TransportManager (see transport_manager.rs).
