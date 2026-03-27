//! Open Paw 9-phase startup sequence.
//!
//! Replicates the Temper CLI's boot flow (`temper serve`) in an embedded context.
//! The daemon boots the Temper platform, installs Paw OS apps, seeds souls,
//! and starts the Discord transport.

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
use tokio::sync::oneshot;

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
];

const DEFAULT_PAW_AGENT_CONFIG_PATH: &str = "config/paw_agent_config.json";
const LOCAL_SANDBOX_WORKDIR: &str = "/tmp/paw-sandbox";

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

    // Phase 3: Set OS apps directory
    tracing::info!("Phase 3: Loading OS apps from ./os-apps/...");
    let os_apps_dir = PathBuf::from("os-apps");
    if os_apps_dir.exists() {
        temper_platform::os_apps::set_os_apps_dir(os_apps_dir.clone());
    } else {
        tracing::warn!("os-apps/ directory not found — OS apps will not be available");
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
            // Ephemeral key — secrets lost on restart.
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

        if let Some(ref key) = config.e2b_api_key {
            let _ = vault.cache_secret("default", "e2b_api_key", key.clone());
            if tenant != "default" {
                let _ = vault.cache_secret(&tenant, "e2b_api_key", key.clone());
            }
        }

        if let Some(ref token) = config.github_token {
            let _ = vault.cache_secret("default", "github_token", token.clone());
            if tenant != "default" {
                let _ = vault.cache_secret(&tenant, "github_token", token.clone());
            }
        }

        if let Some(ref token) = config.logfire_read_token {
            let _ = vault.cache_secret("default", "logfire_read_token", token.clone());
            if tenant != "default" {
                let _ = vault.cache_secret(&tenant, "logfire_read_token", token.clone());
            }
        }

        if let Some(ref token) = config.logfire_write_token {
            let _ = vault.cache_secret("default", "logfire_write_token", token.clone());
            if tenant != "default" {
                let _ = vault.cache_secret(&tenant, "logfire_write_token", token.clone());
            }
        }

        // Discord bot token
        if let Some(ref token) = config.discord_bot_token {
            let _ = vault.cache_secret("default", "discord_bot_token", token.clone());
            if tenant != "default" {
                let _ = vault.cache_secret(&tenant, "discord_bot_token", token.clone());
            }
        }

        // Fly.io API token
        if let Some(ref token) = config.fly_api_token {
            let _ = vault.cache_secret("default", "fly_api_token", token.clone());
            if tenant != "default" {
                let _ = vault.cache_secret(&tenant, "fly_api_token", token.clone());
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

    // Phase 9: Bind + start server + bootstrap runtime entities + transports
    tracing::info!("Phase 9: Starting server...");
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}"))
        .await
        .with_context(|| format!("Failed to bind to port {port}"))?;
    let actual_port = listener.local_addr()?.port();
    let _ = state.server.listen_port.set(actual_port);
    seed_runtime_secrets(&state, &config, &tenant, actual_port).await?;

    let api_key = config.temper_api_key.clone();
    let router = build_platform_router(state.clone()).merge(crate::webhooks::router(
        crate::webhooks::WebhookConfig::new(
            format!("http://127.0.0.1:{actual_port}"),
            tenant.clone(),
            api_key.clone(),
        ),
    ));
    let (ready_tx, ready_rx) = oneshot::channel();
    let server = tokio::spawn(async move {
        let _ = ready_tx.send(());
        axum::serve(listener, router).await
    });
    ready_rx
        .await
        .map_err(|_| anyhow::anyhow!("server readiness signal dropped before startup bootstrap"))?;

    if let Err(error) =
        bootstrap_runtime_entities(actual_port, tenant.clone(), api_key.clone()).await
    {
        server.abort();
        return Err(error);
    }

    if let Some(ref token) = config.discord_bot_token {
        spawn_discord_transport(token.clone(), &tenant, actual_port, api_key);
    } else {
        tracing::warn!("No DISCORD_BOT_TOKEN — Discord transport not started");
    }

    // Spawn background loops
    // TODO: spawn_optimization_loop, spawn_actor_passivation_loop

    tracing::info!("Open Paw listening on port {actual_port}");
    match server.await {
        Ok(Ok(())) => Ok(()),
        Ok(Err(error)) => Err(error.into()),
        Err(error) => Err(anyhow::anyhow!("Open Paw server task failed: {error}")),
    }
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

/// Bootstrap runtime entities that depend on the OData API being live.
async fn bootstrap_runtime_entities(
    port: u16,
    tenant: String,
    api_key: Option<String>,
) -> Result<()> {
    let api_url = format!("http://127.0.0.1:{port}");
    let client = reqwest::Client::new();

    let souls = [
        ("Paw", "Paw project manager agent", "souls/paw.md"),
        (
            "Developer",
            "Software developer agent",
            "souls/developer.md",
        ),
        ("Scout", "Monitoring and triage agent", "souls/scout.md"),
    ];

    for (name, description, path) in &souls {
        match bootstrap_soul(
            &client,
            &api_url,
            &tenant,
            &api_key,
            name,
            description,
            path,
        )
        .await
        {
            Ok(soul_id) => tracing::info!("  Soul '{name}' ready: {soul_id}"),
            Err(e) => tracing::error!("  Failed to bootstrap soul '{name}': {e}"),
        }
    }

    ensure_default_agent_route(&client, &api_url, &tenant, &api_key, "Paw").await
}

/// Create or find a Soul entity for the given soul file.
async fn bootstrap_soul(
    client: &reqwest::Client,
    api_url: &str,
    tenant: &str,
    api_key: &Option<String>,
    name: &str,
    description: &str,
    path: &str,
) -> Result<String> {
    // Read soul content from disk so existing souls can be refreshed in place.
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read soul file: {path}"))?;

    // Check if soul already exists
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

    // Create TemperFS File for the soul content
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
    // OData response puts id at "entity_id" (top level) or "fields.Id"
    let file_id = file_resp["entity_id"]
        .as_str()
        .or_else(|| file_resp["fields"]["Id"].as_str())
        .or_else(|| file_resp["Id"].as_str())
        .context("File creation did not return Id")?
        .to_string();

    // Upload content to the file
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

    // Create Soul entity
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

    // Publish the soul (Draft → Active)
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

/// Ensure there is one active fallback AgentRoute for the Paw soul.
async fn ensure_default_agent_route(
    client: &reqwest::Client,
    api_url: &str,
    tenant: &str,
    api_key: &Option<String>,
    soul_name: &str,
) -> Result<()> {
    let souls_resp = odata_get(client, &format!("{api_url}/tdata/Souls"), tenant, api_key).await?;
    let souls = souls_resp["value"]
        .as_array()
        .context("Failed to list souls for default AgentRoute bootstrap")?;
    let target_exists = souls.iter().any(|soul| {
        entity_field_str(soul, &["Name", "name"]) == Some(soul_name)
            && entity_field_str(soul, &["Status", "status"]) == Some("Active")
    });
    if !target_exists {
        anyhow::bail!("Soul '{soul_name}' not found");
    }

    let agent_config = load_seed_json(DEFAULT_PAW_AGENT_CONFIG_PATH)?;
    let routes_resp = odata_get(
        client,
        &format!("{api_url}/tdata/AgentRoutes"),
        tenant,
        api_key,
    )
    .await?;
    let existing_fallback_route_id = routes_resp["value"].as_array().and_then(|routes| {
        routes.iter().find_map(|route| {
            let status = entity_field_str(route, &["Status", "status"]).unwrap_or("");
            let binding_tier =
                entity_field_str(route, &["BindingTier", "binding_tier"]).unwrap_or("");
            let channel_id = entity_field_str(route, &["ChannelId", "channel_id"]).unwrap_or("");
            if status == "Active" && binding_tier == "channel" && channel_id.is_empty() {
                entity_id_from_json(route).map(ToString::to_string)
            } else {
                None
            }
        })
    });

    if let Some(route_id) = existing_fallback_route_id {
        odata_post(
            client,
            &format!("{api_url}/tdata/AgentRoutes('{route_id}')/Paw.Channel.Update"),
            tenant,
            api_key,
            serde_json::json!({
                "agent_config": agent_config,
                "soul_id": soul_name,
            }),
        )
        .await?;
        tracing::info!("  Refreshed fallback AgentRoute {route_id} for soul '{soul_name}'");
        return Ok(());
    }

    let route_resp = odata_post(
        client,
        &format!("{api_url}/tdata/AgentRoutes"),
        tenant,
        api_key,
        serde_json::json!({}),
    )
    .await?;
    let route_id = route_resp["entity_id"]
        .as_str()
        .or_else(|| route_resp["fields"]["Id"].as_str())
        .or_else(|| route_resp["Id"].as_str())
        .context("AgentRoute creation did not return Id")?;
    odata_post(
        client,
        &format!("{api_url}/tdata/AgentRoutes('{route_id}')/Paw.Channel.Register"),
        tenant,
        api_key,
        serde_json::json!({
            "binding_tier": "channel",
            "channel_id": "",
            "guild_id": "",
            "match_pattern": "",
            "agent_config": agent_config,
            "soul_id": soul_name,
        }),
    )
    .await?;
    tracing::info!("  Created fallback AgentRoute {route_id} for soul '{soul_name}'");
    Ok(())
}

fn load_seed_json(path: &str) -> Result<String> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read seed JSON: {path}"))?;
    let parsed: serde_json::Value =
        serde_json::from_str(&content).with_context(|| format!("Invalid seed JSON: {path}"))?;
    serde_json::to_string(&parsed).context("Failed to serialize seed JSON")
}

async fn seed_runtime_secrets(
    state: &PlatformState,
    config: &Config,
    tenant: &str,
    port: u16,
) -> Result<()> {
    let Some(vault) = state.server.secrets_vault.as_ref() else {
        anyhow::bail!("secrets vault was not initialized");
    };

    let api_url = format!("http://127.0.0.1:{port}");
    let blob_endpoint =
        std::env::var("BLOB_ENDPOINT").unwrap_or_else(|_| format!("{api_url}/_internal/blobs"));
    let blob_bucket = std::env::var("BLOB_BUCKET").unwrap_or_else(|_| "temper-fs".into());
    let _ = vault.cache_secret("default", "temper_api_url", api_url.clone());
    let _ = vault.cache_secret("default", "blob_endpoint", blob_endpoint.clone());
    let _ = vault.cache_secret("default", "blob_bucket", blob_bucket.clone());
    if tenant != "default" {
        let _ = vault.cache_secret(tenant, "temper_api_url", api_url.clone());
        let _ = vault.cache_secret(tenant, "blob_endpoint", blob_endpoint);
        let _ = vault.cache_secret(tenant, "blob_bucket", blob_bucket);
    }

    let explicit_sandbox_url = std::env::var("SANDBOX_URL").ok();
    let local_sandbox_url = format!("http://127.0.0.1:{}", port + 10);
    let default_sandbox_url = explicit_sandbox_url.or_else(|| {
        if config.e2b_api_key.is_some() {
            tracing::info!(
                "E2B_API_KEY is configured; leaving sandbox_url unset so provisioning can use E2B by default"
            );
            None
        } else {
            Some(local_sandbox_url)
        }
    });

    if let Some(sandbox_url) = default_sandbox_url {
        if is_local_sandbox_url(&sandbox_url) {
            // This subprocess is only for local development. Production agent execution
            // should use E2B sandboxes provisioned by the sandbox_provisioner module.
            ensure_local_sandbox(&sandbox_url).await?;
        }
        let _ = vault.cache_secret("default", "sandbox_url", sandbox_url.clone());
        if tenant != "default" {
            let _ = vault.cache_secret(tenant, "sandbox_url", sandbox_url);
        }
    }

    Ok(())
}

fn is_local_sandbox_url(url: &str) -> bool {
    url.contains("127.0.0.1") || url.contains("localhost")
}

async fn ensure_local_sandbox(url: &str) -> Result<()> {
    if local_sandbox_healthy(url).await {
        tracing::info!("Local sandbox already healthy at {url}");
        return Ok(());
    }

    let sandbox_script = Path::new("os-apps/paw-agent/sandbox/local_sandbox.py");
    if !sandbox_script.exists() {
        anyhow::bail!(
            "Local sandbox requested at {url}, but {} is missing",
            sandbox_script.display()
        );
    }

    let sandbox_port = url
        .rsplit(':')
        .next()
        .and_then(|value| value.parse::<u16>().ok())
        .context("Local sandbox URL is missing a valid port")?;
    std::fs::create_dir_all(LOCAL_SANDBOX_WORKDIR).with_context(|| {
        format!("Failed to create local sandbox workdir: {LOCAL_SANDBOX_WORKDIR}")
    })?;

    let child = std::process::Command::new("python3")
        .arg(sandbox_script)
        .arg("--port")
        .arg(sandbox_port.to_string())
        .arg("--workdir")
        .arg(LOCAL_SANDBOX_WORKDIR)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .with_context(|| format!("Failed to start local sandbox at {url}"))?;
    tracing::info!(
        pid = child.id(),
        workdir = LOCAL_SANDBOX_WORKDIR,
        "Started local sandbox dev server at {url}"
    );

    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        if local_sandbox_healthy(url).await {
            tracing::info!("Local sandbox passed health check at {url}");
            return Ok(());
        }
        if tokio::time::Instant::now() >= deadline {
            anyhow::bail!("Local sandbox did not become healthy within 5s at {url}");
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

async fn local_sandbox_healthy(url: &str) -> bool {
    let health_url = format!("{url}/health");
    match reqwest::Client::builder()
        .timeout(Duration::from_millis(500))
        .build()
    {
        Ok(client) => client
            .get(&health_url)
            .send()
            .await
            .map(|response| response.status().is_success())
            .unwrap_or(false),
        Err(_) => false,
    }
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

    let release_dir = module_dir.join("target/wasm32-unknown-unknown/release");
    let candidates = [
        release_dir.join(format!("{module_name}.wasm")),
        release_dir.join(format!("{}.wasm", module_name.replace('_', "-"))),
        module_dir.join(format!("{module_name}.wasm")),
        module_dir.join(format!("{}.wasm", module_name.replace('_', "-"))),
    ];

    candidates.into_iter().find(|path| path.is_file())
}

/// Spawn the Discord channel transport.
fn spawn_discord_transport(bot_token: String, tenant: &str, port: u16, api_key: Option<String>) {
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
            intents: intents::DEFAULT,
            webhook_port: 0, // Auto-assign
        };
        let transport = DiscordTransport::new(config, api);
        if let Err(e) = transport.run().await {
            tracing::error!("Discord transport fatal error: {e}");
        }
    });
}
