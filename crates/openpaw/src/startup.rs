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
];

/// Run the Open Paw daemon.
pub async fn run(config: Config) -> Result<()> {
    let port = config.port;
    let tenant = config.tenant.clone();
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    let data_dir = Path::new(&home).join(".local/share/openpaw");
    std::fs::create_dir_all(&data_dir)
        .with_context(|| format!("Failed to create data dir: {}", data_dir.display()))?;

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

    // Phase 4: Assemble PlatformState
    tracing::info!("Phase 4: Assembling platform state...");
    let mut state = PlatformState::with_registry(registry, config.anthropic_api_key.clone());
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
        let sys_hashes =
            temper_platform::bootstrap_system_tenant(&state, &sys_cache);
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
        let key_bytes: [u8; 32] = if let Some(ref key_b64) = config.vault_key {
            use base64::Engine as _;

            match base64::engine::general_purpose::STANDARD.decode(key_b64) {
                Ok(decoded) if decoded.len() == 32 => decoded.try_into().unwrap(),
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
        } else {
            let mut key = [0u8; 32];
            rand::fill(&mut key);
            tracing::warn!("No TEMPER_VAULT_KEY set — using ephemeral vault key");
            key
        };
        let vault = temper_server::secrets::vault::SecretsVault::new(&key_bytes);
        state.server.secrets_vault = Some(Arc::new(vault));
    }

    // Seed secrets from env
    if let Some(ref vault) = state.server.secrets_vault {
        if let Some(ref key) = config.anthropic_api_key {
            let _ = vault.cache_secret("default", "anthropic_api_key", key.clone());
            if tenant != "default" {
                let _ = vault.cache_secret(&tenant, "anthropic_api_key", key.clone());
            }
        }

        if let Some(ref key) = config.tensorlake_api_key {
            let _ = vault.cache_secret("default", "tensorlake_api_key", key.clone());
            if tenant != "default" {
                let _ = vault.cache_secret(&tenant, "tensorlake_api_key", key.clone());
            }
        }

        if let Some(ref token) = config.github_token {
            let _ = vault.cache_secret("default", "github_token", token.clone());
            if tenant != "default" {
                let _ = vault.cache_secret(&tenant, "github_token", token.clone());
            }
        }

        if let Some(ref token) = config.dd_api_key {
            let _ = vault.cache_secret("default", "dd_api_key", token.clone());
            if tenant != "default" {
                let _ = vault.cache_secret(&tenant, "dd_api_key", token.clone());
            }
        }

        if let Some(ref token) = config.dd_app_key {
            let _ = vault.cache_secret("default", "dd_app_key", token.clone());
            if tenant != "default" {
                let _ = vault.cache_secret(&tenant, "dd_app_key", token.clone());
            }
        }

        {
            let _ = vault.cache_secret("default", "dd_site", config.dd_site.clone());
            if tenant != "default" {
                let _ = vault.cache_secret(&tenant, "dd_site", config.dd_site.clone());
            }
        }

        if let Some(ref key) = config.exa_api_key {
            let _ = vault.cache_secret("default", "exa_api_key", key.clone());
            if tenant != "default" {
                let _ = vault.cache_secret(&tenant, "exa_api_key", key.clone());
            }
        }

        let api_url = format!("http://127.0.0.1:{port}");
        let _ = vault.cache_secret("default", "temper_api_url", api_url.clone());
        if tenant != "default" {
            let _ = vault.cache_secret(&tenant, "temper_api_url", api_url);
        }

        // Sandbox URL: explicit override for testing, otherwise Tensorlake provisions on demand.
        if let Some(sandbox_url) = std::env::var("SANDBOX_URL").ok().filter(|s| !s.is_empty()) {
            let _ = vault.cache_secret("default", "sandbox_url", sandbox_url.clone());
            if tenant != "default" {
                let _ = vault.cache_secret(&tenant, "sandbox_url", sandbox_url);
            }
        } else if config.tensorlake_api_key.is_some() {
            tracing::info!(
                "Tensorlake API key configured; sandbox_provisioner will create sandboxes on demand"
            );
        } else {
            tracing::warn!("No TL_API_KEY or SANDBOX_URL — agent sandbox provisioning will fail");
        }

        // Local blob store for TemperFS content uploads/downloads.
        let blob_store_port = port + 20;
        let blob_endpoint = if let Ok(url) = std::env::var("BLOB_ENDPOINT") {
            url
        } else {
            let blob_script = Path::new("os-apps/paw-fs/sandbox/local_blob_store.py");
            let url = format!("http://127.0.0.1:{blob_store_port}");
            if blob_script.exists() {
                let _ = std::fs::create_dir_all("/tmp/openpaw-blobs");
                match std::process::Command::new("python3")
                    .arg(blob_script)
                    .arg("--port")
                    .arg(blob_store_port.to_string())
                    .arg("--dir")
                    .arg("/tmp/openpaw-blobs")
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn()
                {
                    Ok(_) => tracing::info!("Local blob store: {url} (auto-started)"),
                    Err(e) => tracing::warn!("Failed to start local blob store: {e}"),
                }
            }
            url
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
            let _ = vault.cache_secret("default", "blob_access_key", key.clone());
            if tenant != "default" {
                let _ = vault.cache_secret(&tenant, "blob_access_key", key);
            }
        }
        if let Ok(key) = std::env::var("BLOB_SECRET_KEY") {
            let _ = vault.cache_secret("default", "blob_secret_key", key.clone());
            if tenant != "default" {
                let _ = vault.cache_secret(&tenant, "blob_secret_key", key);
            }
        }

        if let Some(ref token) = config.discord_bot_token {
            let _ = vault.cache_secret("default", "discord_bot_token", token.clone());
            if tenant != "default" {
                let _ = vault.cache_secret(&tenant, "discord_bot_token", token.clone());
            }
        }

        if let Some(ref token) = config.slack_bot_token {
            let _ = vault.cache_secret("default", "slack_bot_token", token.clone());
            if tenant != "default" {
                let _ = vault.cache_secret(&tenant, "slack_bot_token", token.clone());
            }
        }
        if let Some(ref token) = config.slack_app_token {
            let _ = vault.cache_secret("default", "slack_app_token", token.clone());
            if tenant != "default" {
                let _ = vault.cache_secret(&tenant, "slack_app_token", token.clone());
            }
        }

        if let Some(ref token) = config.fly_api_token {
            let _ = vault.cache_secret("default", "fly_api_token", token.clone());
            if tenant != "default" {
                let _ = vault.cache_secret(&tenant, "fly_api_token", token.clone());
            }
        }

        if let Some(ref token) = config.railway_token {
            let _ = vault.cache_secret("default", "railway_token", token.clone());
            if tenant != "default" {
                let _ = vault.cache_secret(&tenant, "railway_token", token.clone());
            }
        }

        if let Some(ref token) = config.vercel_token {
            let _ = vault.cache_secret("default", "vercel_token", token.clone());
            if tenant != "default" {
                let _ = vault.cache_secret(&tenant, "vercel_token", token.clone());
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

    // Phase 8: Banner
    tracing::info!("Phase 8: Bootstrap complete");
    println!();
    println!("  Open Paw Data API: http://localhost:{port}/tdata");
    println!("  Tenant: {tenant}");
    println!();

    // Phase 9: Bind + start transports + serve
    tracing::info!("Phase 9: Starting server...");
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}"))
        .await
        .with_context(|| format!("Failed to bind to port {port}"))?;
    let actual_port = listener.local_addr()?.port();
    let _ = state.server.listen_port.set(actual_port);
    let router = build_platform_router(state.clone());

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

    if let Some(ref token) = config.discord_bot_token {
        spawn_discord_transport(
            token.clone(),
            config.discord_public_key.clone().unwrap_or_default(),
            config.discord_guild_id.clone(),
            config.discord_feed_channel_id.clone(),
            config.discord_forum_channel_id.clone(),
            &tenant,
            actual_port,
            config.temper_api_key.clone(),
        );
    } else {
        tracing::warn!("No DISCORD_BOT_TOKEN — Discord transport not started");
    }

    if let (Some(app_token), Some(bot_token)) =
        (&config.slack_app_token, &config.slack_bot_token)
    {
        spawn_slack_transport(
            app_token.clone(),
            bot_token.clone(),
            config.slack_signing_secret.clone().unwrap_or_default(),
            &tenant,
            actual_port,
            config.temper_api_key.clone(),
        );
    } else {
        tracing::warn!("No SLACK_APP_TOKEN/SLACK_BOT_TOKEN — Slack transport not started");
    }

    // Spawn Discord observer (SSE → Discord feed/forum).
    if config.discord_feed_channel_id.is_some() || config.discord_forum_channel_id.is_some() {
        let observer_api = paw_transport::PawApiClient::new(paw_transport::PawApiConfig {
            base_url: format!("http://127.0.0.1:{actual_port}"),
            tenant: tenant.clone(),
            api_key: config.temper_api_key.clone(),
        });
        let observer_config = paw_transport::discord::ObserverConfig {
            bot_token: config.discord_bot_token.clone().unwrap_or_default(),
            feed_channel_id: config.discord_feed_channel_id.clone(),
            forum_channel_id: config.discord_forum_channel_id.clone(),
        };
        tokio::spawn(async move {
            // Give the server a moment to start accepting connections.
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            if let Err(e) = paw_transport::discord::run_observer(observer_api, observer_config).await {
                tracing::error!("Discord observer failed: {e}");
            }
        });
    }

    // Spawn background loops
    state.server.spawn_runtime_metrics_loop();

    spawn_soul_bootstrap(actual_port, tenant.clone(), config.temper_api_key.clone());

    tracing::info!("Open Paw listening on port {actual_port}");


    axum::serve(listener, router).await?;

    Ok(())
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
/// Reads soul files from `souls/` directory, creates TemperFS File entities
/// for the content, and registers Soul entities. Runs once on first boot;
/// skips if souls already exist.
fn spawn_soul_bootstrap(port: u16, tenant: String, api_key: Option<String>) {
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(500)).await;

        let api_url = format!("http://127.0.0.1:{port}");
        let client = reqwest::Client::new();

        let souls: &[(&str, &str, &[&str])] = &[
            (
                "Paw",
                "Paw chief of staff agent",
                &[
                    "souls/paw/SOUL.md",
                    "souls/paw/STYLE.md",
                    "souls/paw/SKILL.md",
                ],
            ),
            ("SWE", "Software developer agent", &["souls/swe/SKILL.md"]),
            (
                "SRE",
                "Site reliability engineering agent",
                &["souls/sre/SKILL.md"],
            ),
            (
                "Probe",
                "Foresight probe agent for projecting product futures",
                &["souls/probe.md"],
            ),
        ];

        for (name, description, paths) in souls {
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
                Ok(soul_id) => tracing::info!("  Soul '{name}' ready: {soul_id}"),
                Err(e) => tracing::error!("  Failed to bootstrap soul '{name}': {e}"),
            }
        }

        // Bootstrap project-lead reference skills
        let skills: &[(&str, &str, &str, &str)] = &[
            (
                "Project Lead Schema",
                "Dimensions Paw fills when crafting a project lead soul",
                "souls/project-lead/SCHEMA.md",
                "Paw",
            ),
            (
                "Project Lead Playbook",
                "Shared operational playbook for all project leads",
                "souls/project-lead/SKILL.md",
                "project-lead",
            ),
        ];

        for (name, description, path, scope) in skills {
            match bootstrap_skill(&client, &api_url, &tenant, &api_key, name, description, path, scope)
                .await
            {
                Ok(skill_id) => tracing::info!("  Skill '{name}' ready: {skill_id}"),
                Err(e) => tracing::error!("  Failed to bootstrap skill '{name}': {e}"),
            }
        }

        // ── Skill scope migration ──────────────────────────────────────
        // Fix skills with scope="soul" — these are invisible to agents because
        // the LLM caller queries by soul name, not the literal "soul" string.
        // Migrate: scope = agent_filter when scope == "soul" and agent_filter is set.
        migrate_skill_scopes(&client, &api_url, &tenant, &api_key).await;

        if let Err(e) = set_default_soul(&client, &api_url, &tenant, &api_key, "Paw").await {
            tracing::warn!("Could not set default soul on AgentRoute: {e}");
        }
    });
}

/// Migrate broken skill scopes and clean up ghost skills.
///
/// - Skills with `scope="soul"` and a non-empty `agent_filter` get their scope
///   updated to the `agent_filter` value (the actual soul name the LLM caller
///   uses for filtering).
/// - Skills with no name are logged as ghosts (artifacts of failed Register actions).
async fn migrate_skill_scopes(
    client: &reqwest::Client,
    api_url: &str,
    tenant: &str,
    api_key: &Option<String>,
) {
    // Query all skills
    let url = format!("{api_url}/tdata/Skills");
    let resp = match odata_get(client, &url, tenant, api_key).await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("skill scope migration: failed to list skills: {e}");
            return;
        }
    };

    let items = match resp["value"].as_array() {
        Some(arr) => arr.clone(),
        None => return,
    };

    for item in &items {
        let id = entity_id_from_json(item).unwrap_or("unknown");
        let name = entity_field_str(item, &["Name", "name"]).unwrap_or("");
        let scope = entity_field_str(item, &["Scope", "scope"]).unwrap_or("");
        let agent_filter = entity_field_str(item, &["agent_filter"]).unwrap_or("");

        // Ghost skill: no name — log warning
        if name.is_empty() {
            tracing::warn!(
                skill_id = id,
                "skill scope migration: ghost skill with no name (failed Register?)"
            );
            continue;
        }

        // Fix broken scope: "soul" → agent_filter value
        if scope == "soul" && !agent_filter.is_empty() {
            tracing::info!(
                skill_id = id,
                name,
                old_scope = scope,
                new_scope = agent_filter,
                "skill scope migration: fixing scope"
            );
            // Use Register action to update scope (Register is idempotent on Active skills)
            let content_file_id = entity_field_str(item, &["ContentFileId", "content_file_id"])
                .unwrap_or("")
                .to_string();
            let description = entity_field_str(item, &["Description", "description"])
                .unwrap_or("")
                .to_string();
            if let Err(e) = odata_post(
                client,
                &format!("{api_url}/tdata/Skills('{id}')/OpenPaw.Register"),
                tenant,
                api_key,
                serde_json::json!({
                    "name": name,
                    "description": description,
                    "content_file_id": content_file_id,
                    "scope": agent_filter,
                    "agent_filter": agent_filter,
                }),
            )
            .await
            {
                tracing::warn!(
                    skill_id = id,
                    name,
                    error = %e,
                    "skill scope migration: failed to update scope"
                );
            }
        }
    }
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
            std::fs::read_to_string(p)
                .with_context(|| format!("Failed to read soul file: {p}"))
        })
        .collect::<Result<Vec<_>>>()?
        .join("\n\n");

    let filter = format!("Name eq '{name}'");
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

/// Create or find a Skill entity for a reference file.
async fn bootstrap_skill(
    client: &reqwest::Client,
    api_url: &str,
    tenant: &str,
    api_key: &Option<String>,
    name: &str,
    description: &str,
    path: &str,
    scope: &str,
) -> Result<String> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read skill file: {path}"))?;

    // Check if skill already exists by name
    let filter = format!("Name eq '{name}'");
    let list_url = format!("{api_url}/tdata/Skills?$filter={filter}");
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
                .with_context(|| {
                    format!("Failed to refresh existing skill content for '{name}'")
                })?;
            }
            tracing::info!("  Skill '{name}' already exists: {id}");
            return Ok(id.to_string());
        }
    }

    // Create TemperFS file
    let file_resp = odata_post(
        client,
        &format!("{api_url}/tdata/Files"),
        tenant,
        api_key,
        serde_json::json!({
            "Name": format!("{}.skill.md", name.to_lowercase().replace(' ', "-")),
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

    // Create Skill entity
    let skill_resp = odata_post(
        client,
        &format!("{api_url}/tdata/Skills"),
        tenant,
        api_key,
        serde_json::json!({}),
    )
    .await?;
    let skill_id = skill_resp["entity_id"]
        .as_str()
        .or_else(|| skill_resp["fields"]["Id"].as_str())
        .or_else(|| skill_resp["Id"].as_str())
        .context("Skill creation did not return Id")?
        .to_string();

    // Register the skill with metadata
    odata_post(
        client,
        &format!("{api_url}/tdata/Skills('{skill_id}')/OpenPaw.Register"),
        tenant,
        api_key,
        serde_json::json!({
            "name": name,
            "description": description,
            "content_file_id": file_id,
            "scope": scope,
            "agent_filter": ""
        }),
    )
    .await?;

    Ok(skill_id)
}

/// Set the Paw soul as the default on any existing AgentRoute.
async fn set_default_soul(
    client: &reqwest::Client,
    api_url: &str,
    tenant: &str,
    api_key: &Option<String>,
    soul_name: &str,
) -> Result<()> {
    let souls_resp = odata_get(
        client,
        &format!("{api_url}/tdata/Souls?$filter=Status eq 'Active'"),
        tenant,
        api_key,
    )
    .await?;
    let souls = souls_resp["value"]
        .as_array()
        .context("Failed to list active souls")?;

    let mut known_refs = HashSet::new();
    let mut target_exists = false;
    for soul in souls {
        if let Some(id) = entity_id_from_json(soul) {
            known_refs.insert(id.to_string());
        }
        if let Some(name) = entity_field_str(soul, &["Name", "name"]) {
            if name == soul_name {
                target_exists = true;
            }
            known_refs.insert(name.to_string());
        }
    }

    if !target_exists {
        anyhow::bail!("Soul '{soul_name}' not found");
    }

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
            let current_soul = entity_field_str(route, &["SoulId", "soul_id"]).unwrap_or("");
            let channel_id = entity_field_str(route, &["ChannelId", "channel_id"]).unwrap_or("");
            let needs_repair = current_soul.is_empty() || !known_refs.contains(current_soul);
            if needs_repair && !route_id.is_empty() {
                odata_post(
                    client,
                    &format!("{api_url}/tdata/AgentRoutes('{route_id}')/Paw.Channel.Update"),
                    tenant,
                    api_key,
                    serde_json::json!({ "soul_id": soul_name }),
                )
                .await
                .ok();
                tracing::info!("  Repaired soul '{soul_name}' on AgentRoute {route_id}");
            }
            if channel_id.is_empty() {
                has_global_route = true;
            }
        }
    }

    // Ensure a global fallback AgentRoute exists so Discord (and any other
    // channel) gets routed to the Paw soul with the full tool set.
    if !has_global_route {
        tracing::info!("  No global AgentRoute found — creating one with soul '{soul_name}'");
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
                let agent_config = serde_json::json!({
                    "model": "claude-sonnet-4-6",
                    "provider": "anthropic",
                    "tools_enabled": "temper_create,temper_get,temper_list,temper_action,read_entity,save_memory,spawn_agent",
                    "max_turns": "24",
                    "temper_api_url": api_url,
                    "max_follow_ups": "8",
                });
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
                        "soul_id": soul_name,
                    }),
                )
                .await
                .ok();
                tracing::info!("  Created global AgentRoute {route_id} with soul '{soul_name}'");
            }
        }
    }

    // Also repair Channels with missing default_agent_config
    let channels_resp = odata_get(
        client,
        &format!("{api_url}/tdata/Channels?$filter=Status eq 'Connected' or Status eq 'Disconnected'"),
        tenant,
        api_key,
    )
    .await?;

    if let Some(channels) = channels_resp["value"].as_array() {
        let soul_config = serde_json::json!({ "soul_id": soul_name }).to_string();
        for channel in channels {
            let channel_id = entity_id_from_json(channel).unwrap_or("");
            let current_config =
                entity_field_str(channel, &["DefaultAgentConfig", "default_agent_config"])
                    .unwrap_or("");

            // Parse config and check if soul_id is set
            let config: serde_json::Value =
                serde_json::from_str(current_config).unwrap_or(serde_json::json!({}));
            let has_soul = config
                .get("soul_id")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|s| !s.is_empty());

            if !has_soul && !channel_id.is_empty() {
                odata_post(
                    client,
                    &format!("{api_url}/tdata/Channels('{channel_id}')/Paw.Channel.UpdateConfig"),
                    tenant,
                    api_key,
                    serde_json::json!({ "default_agent_config": soul_config }),
                )
                .await
                .ok();
                tracing::info!(
                    "  Set default soul '{soul_name}' on Channel {channel_id}"
                );
            }
        }
    }

    Ok(())
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
            .map_err(|error| anyhow::anyhow!("Failed to persist WASM module '{module_name}': {error}"))?;
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
        let config = WebhookTriggerConfig {
            port: trigger_port,
        };
        let trigger = WebhookTrigger::new(config, api);
        if let Err(e) = trigger.run().await {
            tracing::error!("Webhook trigger fatal error: {e}");
        }
    });
}

/// Spawn the Discord channel transport.
fn spawn_discord_transport(
    bot_token: String,
    public_key: String,
    guild_id: Option<String>,
    feed_channel_id: Option<String>,
    forum_channel_id: Option<String>,
    tenant: &str,
    port: u16,
    api_key: Option<String>,
) {
    use paw_transport::PawApiConfig;
    use paw_transport::discord::types::intents;
    use paw_transport::discord::{DiscordConfig, DiscordTransport};

    let tenant = tenant.to_string();
    let api_url = format!("http://127.0.0.1:{port}");
    tracing::info!("Discord transport: connecting (tenant={tenant})...");

    tokio::spawn(async move {
        let api = paw_transport::PawApiClient::new(PawApiConfig {
            base_url: api_url,
            tenant,
            api_key,
        });
        let config = DiscordConfig {
            bot_token,
            public_key,
            intents: intents::DEFAULT,
            webhook_port: 3488,
            guild_id,
            feed_channel_id,
            forum_channel_id,
        };
        let transport = DiscordTransport::new(config, api);
        if let Err(e) = transport.run().await {
            tracing::error!("Discord transport fatal error: {e}");
        }
    });
}

/// Spawn the Slack channel transport.
fn spawn_slack_transport(
    app_token: String,
    bot_token: String,
    signing_secret: String,
    tenant: &str,
    port: u16,
    api_key: Option<String>,
) {
    use paw_transport::PawApiConfig;
    use paw_transport::slack::{SlackConfig, SlackTransport};

    let tenant = tenant.to_string();
    let api_url = format!("http://127.0.0.1:{port}");
    tracing::info!("Slack transport: connecting (tenant={tenant})...");

    tokio::spawn(async move {
        let api = paw_transport::PawApiClient::new(PawApiConfig {
            base_url: api_url,
            tenant,
            api_key,
        });
        let config = SlackConfig {
            app_token,
            bot_token,
            webhook_port: 3489,
            signing_secret,
        };
        let transport = SlackTransport::new(config, api);
        if let Err(e) = transport.run().await {
            tracing::error!("Slack transport fatal error: {e}");
        }
    });
}
