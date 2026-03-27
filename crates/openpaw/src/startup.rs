//! Open Paw 9-phase startup sequence.
//!
//! Replicates the Temper CLI's boot flow (`temper serve`) in an embedded context.
//! The daemon boots the Temper platform, installs Paw OS apps, seeds souls,
//! and starts the Discord transport.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};
use temper_platform::PlatformState;
use temper_platform::router::build_platform_router;

use crate::config::Config;

/// Paw OS apps to install at startup.
const PAW_OS_APPS: &[&str] = &[
    "paw-agent",
    "paw-channels",
    "paw-fs",
    "paw-pm",
    "paw-harness",
    "paw-heal",
];

/// Run the Open Paw daemon.
pub async fn run(config: Config) -> Result<()> {
    let port = config.port;
    let tenant = config.tenant.clone();

    // Phase 1: Storage backend (Turso local SQLite)
    tracing::info!("Phase 1: Initializing storage...");
    let turso_url = config.turso_url.clone().unwrap_or_else(|| {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
        let db_path = std::path::Path::new(&home).join(".local/share/openpaw/paw.db");
        if let Some(parent) = db_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        format!("file:{}", db_path.display())
    });
    let turso_token = config.turso_auth_token.clone();
    let turso_store = temper_store_turso::TursoEventStore::new(
        &turso_url,
        turso_token.as_deref(),
    ).await.context("Failed to connect to Turso/libSQL")?;
    tracing::info!("Storage: turso ({turso_url})");
    let event_store = Some(temper_server::event_store::ServerEventStore::Turso(turso_store));

    // Phase 2: Build empty registry
    tracing::info!("Phase 2: Building spec registry...");
    let registry = temper_server::SpecRegistry::new();

    // Phase 3: Set OS apps directory
    let os_apps_dir = std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("os-apps");
    tracing::info!("Phase 3: Loading OS apps from {}", os_apps_dir.display());
    if os_apps_dir.exists() {
        temper_platform::os_apps::set_os_apps_dir(os_apps_dir.clone());
    } else {
        tracing::warn!("os-apps/ directory not found — OS apps will not be available");
    }

    // Phase 4: Assemble PlatformState
    tracing::info!("Phase 4: Assembling platform state...");
    let mut state = PlatformState::with_registry(registry, config.anthropic_api_key.clone());
    state.api_token = config.temper_api_key.clone();

    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    let data_dir = Path::new(&home).join(".local/share/openpaw");
    state.server.data_dir = data_dir.clone();

    // Wire event store
    if let Some(store) = event_store {
        state.server.event_store = Some(Arc::new(store));
    }

    // Phase 5: Secrets vault
    tracing::info!("Phase 5: Configuring secrets vault...");
    {
        let key_bytes: [u8; 32] = if let Some(ref key_b64) = config.vault_key {
            use base64::Engine as _;
            let decoded = base64::engine::general_purpose::STANDARD
                .decode(key_b64)
                .context("TEMPER_VAULT_KEY must be valid base64")?;
            anyhow::ensure!(decoded.len() == 32, "TEMPER_VAULT_KEY must be 32 bytes");
            decoded.try_into().unwrap()
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

        let api_url = format!("http://127.0.0.1:{port}");
        let _ = vault.cache_secret("default", "temper_api_url", api_url.clone());

        // blob_endpoint — internal blob store for TemperFS file content
        let blob_url = format!("http://127.0.0.1:{port}/_internal/blobs");
        let _ = vault.cache_secret("default", "blob_endpoint", blob_url.clone());
        if tenant != "default" {
            let _ = vault.cache_secret(&tenant, "blob_endpoint", blob_url);
        }
        if tenant != "default" {
            let _ = vault.cache_secret(&tenant, "temper_api_url", api_url);
        }

        // Sandbox URL — when SANDBOX_URL is set, use it (and auto-start local sandbox if needed).
        // When not set, sandbox_provisioner WASM will use E2B API.
        if let Ok(sandbox_url) = std::env::var("SANDBOX_URL") {
            tracing::info!("Sandbox: {sandbox_url} (from SANDBOX_URL)");

            // Auto-start local sandbox if URL points to localhost
            if sandbox_url.contains("127.0.0.1") || sandbox_url.contains("localhost") {
                let sandbox_script = std::env::current_dir()
                    .unwrap_or_default()
                    .join("os-apps/paw-agent/sandbox/local_sandbox.py");
                if sandbox_script.exists() {
                    // Extract port from URL
                    let sandbox_port = sandbox_url
                        .rsplit(':')
                        .next()
                        .and_then(|p| p.parse::<u16>().ok())
                        .unwrap_or(3478);
                    let _ = std::fs::create_dir_all("/tmp/paw-workspace");
                    match std::process::Command::new("python3")
                        .arg(&sandbox_script)
                        .arg("--port")
                        .arg(sandbox_port.to_string())
                        .arg("--workdir")
                        .arg("/tmp/paw-workspace")
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .spawn()
                    {
                        Ok(_) => tracing::info!("  Local sandbox auto-started on port {sandbox_port}"),
                        Err(e) => tracing::warn!("  Failed to start local sandbox: {e}"),
                    }
                }
            }

            let _ = vault.cache_secret("default", "sandbox_url", sandbox_url.clone());
            if tenant != "default" {
                let _ = vault.cache_secret(&tenant, "sandbox_url", sandbox_url);
            }
        } else {
            tracing::info!("Sandbox: E2B (no SANDBOX_URL set, will use e2b_api_key)");
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

        // E2B API key
        if let Some(ref key) = config.e2b_api_key {
            let _ = vault.cache_secret("default", "e2b_api_key", key.clone());
            if tenant != "default" {
                let _ = vault.cache_secret(&tenant, "e2b_api_key", key.clone());
            }
        }

        // GitHub token
        if let Some(ref token) = config.github_token {
            let _ = vault.cache_secret("default", "github_token", token.clone());
            if tenant != "default" {
                let _ = vault.cache_secret(&tenant, "github_token", token.clone());
            }
        }

        // Logfire tokens
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
    }

    // Phase 6: Install Paw OS apps
    tracing::info!("Phase 6: Installing Paw OS apps...");
    for app_name in PAW_OS_APPS {
        match temper_platform::install_os_app(&state, &tenant, app_name).await {
            Ok(result) => tracing::info!("  Installed {app_name}: {result:?}"),
            Err(e) => tracing::error!("  Failed to install {app_name}: {e}"),
        }
    }

    // Phase 7: Recovery (Cedar policies + WASM modules + secrets from store)
    tracing::info!("Phase 7: Recovery...");
    // TODO: Recover from event store when persistence is wired
    state.server.rebuild_reaction_dispatcher();

    // Phase 8: Banner
    tracing::info!("Phase 8: Bootstrap complete");
    println!();
    println!("  Open Paw Data API: http://localhost:{port}/tdata");
    println!("  Tenant: {tenant}");
    println!();

    // Phase 9: Bind + start transports + serve
    tracing::info!("Phase 9: Starting server...");
    let router = build_platform_router(state.clone());
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}"))
        .await
        .with_context(|| format!("Failed to bind to port {port}"))?;
    let actual_port = listener.local_addr()?.port();
    let _ = state.server.listen_port.set(actual_port);

    // Start Discord transport if token is available
    if let Some(ref token) = config.discord_bot_token {
        spawn_discord_transport(token.clone(), &tenant, actual_port, config.temper_api_key.clone());
    } else {
        tracing::warn!("No DISCORD_BOT_TOKEN — Discord transport not started");
    }

    // Spawn background loops
    // TODO: spawn_optimization_loop, spawn_actor_passivation_loop

    // Spawn soul bootstrap (needs OData API running, so runs after bind)
    spawn_soul_bootstrap(actual_port, tenant.clone(), config.temper_api_key.clone());

    tracing::info!("Open Paw listening on port {actual_port}");
    axum::serve(listener, router).await?;

    Ok(())
}

/// Bootstrap Paw souls into the entity system.
///
/// Reads soul files from `souls/` directory, creates TemperFS File entities
/// for the content, and registers Soul entities. Runs once on first boot;
/// skips if souls already exist.
fn spawn_soul_bootstrap(port: u16, tenant: String, api_key: Option<String>) {
    tokio::spawn(async move {
        // Give the server a moment to be ready
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        let api_url = format!("http://127.0.0.1:{port}");
        let client = reqwest::Client::new();

        let souls = [
            ("Paw", "Paw project manager agent", "souls/paw.md"),
            ("Developer", "Software developer agent", "souls/developer.md"),
            ("Scout", "Monitoring and triage agent", "souls/scout.md"),
        ];

        for (name, description, path) in &souls {
            match bootstrap_soul(&client, &api_url, &tenant, &api_key, name, description, path).await {
                Ok(soul_id) => tracing::info!("  Soul '{name}' ready: {soul_id}"),
                Err(e) => tracing::error!("  Failed to bootstrap soul '{name}': {e}"),
            }
        }

        // Set Paw as the default soul for the Discord channel route
        if let Err(e) = set_default_soul(&client, &api_url, &tenant, &api_key, "Paw").await {
            tracing::warn!("Could not set default soul on AgentRoute: {e}");
        }
    });
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
    // Check if soul already exists (query all, filter client-side)
    let list_resp = odata_get(client, &format!("{api_url}/tdata/Souls"), tenant, api_key).await?;
    if let Some(items) = list_resp["value"].as_array() {
        if let Some(existing) = items.iter().find(|s| {
            s.get("fields").and_then(|f| f.get("Name")).and_then(|n| n.as_str()) == Some(name)
        }) {
            let id = existing["entity_id"].as_str()
                .or_else(|| existing["fields"]["Id"].as_str())
                .unwrap_or("unknown");
            tracing::info!("  Soul '{name}' already exists: {id}");
            return Ok(id.to_string());
        }
    }

    // Read soul content from disk
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read soul file: {path}"))?;

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
    ).await?;
    // OData response puts id at "entity_id" (top level) or "fields.Id"
    let file_id = file_resp["entity_id"]
        .as_str()
        .or_else(|| file_resp["fields"]["Id"].as_str())
        .or_else(|| file_resp["Id"].as_str())
        .context("File creation did not return Id")?
        .to_string();

    // Upload content to the file
    let upload_url = format!("{api_url}/tdata/Files('{file_id}')/$value");
    let mut req = client.put(&upload_url)
        .header("x-tenant-id", tenant)
        .header("x-temper-principal-kind", "admin")
        .header("content-type", "text/markdown")
        .body(content);
    if let Some(key) = api_key {
        req = req.header("authorization", format!("Bearer {key}"));
    }
    req.send().await.context("Failed to upload soul content")?;

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
    ).await?;
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
    ).await?;

    Ok(soul_id)
}

/// Set the Paw soul as the default on any existing AgentRoute.
async fn set_default_soul(
    client: &reqwest::Client,
    api_url: &str,
    tenant: &str,
    api_key: &Option<String>,
    soul_name: &str,
) -> Result<()> {
    // Find the soul by name (query all, filter client-side)
    let resp = odata_get(
        client,
        &format!("{api_url}/tdata/Souls"),
        tenant,
        api_key,
    ).await?;
    let soul_id = resp["value"]
        .as_array()
        .and_then(|arr| arr.iter().find(|s| {
            s.get("fields")
                .and_then(|f| f.get("Name"))
                .and_then(|n| n.as_str())
                == Some(soul_name)
        }))
        .and_then(|s| s["entity_id"].as_str().or_else(|| s["fields"]["Id"].as_str()))
        .context("Paw soul not found")?
        .to_string();

    // Find AgentRoutes and update their soul_id.
    // Retry up to 10 times with 2-second delays because the Discord transport
    // creates the AgentRoute concurrently and it may not exist yet.
    let mut found_routes = false;
    for attempt in 1..=10 {
        let routes_resp = odata_get(
            client,
            &format!("{api_url}/tdata/AgentRoutes"),
            tenant,
            api_key,
        ).await?;

        if let Some(routes) = routes_resp["value"].as_array() {
            if !routes.is_empty() {
                for route in routes {
                    let route_id = route["entity_id"].as_str()
                        .or_else(|| route["fields"]["Id"].as_str())
                        .unwrap_or("");
                    let current_soul = route["fields"]["soul_id"].as_str()
                        .or_else(|| route["SoulId"].as_str())
                        .unwrap_or("");
                    if !route_id.is_empty() {
                        // Update route with Paw soul + full agent config
                        let paw_config = r#"{"model":"claude-sonnet-4-20250514","provider":"anthropic","tools_enabled":"temper_create,temper_action,temper_list,read_entity,save_memory,spawn_agent,logfire_query","max_turns":"100","max_follow_ups":"0"}"#;
                        odata_post(
                            client,
                            &format!("{api_url}/tdata/AgentRoutes('{route_id}')/OpenPaw.AgentRoute.Update"),
                            tenant,
                            api_key,
                            serde_json::json!({ "soul_id": soul_id, "agent_config": paw_config }),
                        ).await.ok(); // Best effort
                        tracing::info!("  Set soul '{soul_name}' on AgentRoute {route_id}");
                    }
                }
                found_routes = true;
                break;
            }
        }

        tracing::debug!("  No AgentRoutes found yet (attempt {attempt}/10), retrying in 2s...");
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }

    if !found_routes {
        tracing::warn!("  No AgentRoutes found after 10 attempts — soul binding skipped");
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
    let mut req = client.get(url)
        .header("x-tenant-id", tenant)
        .header("x-temper-principal-kind", "admin");
    if let Some(key) = api_key {
        req = req.header("authorization", format!("Bearer {key}"));
    }
    let resp = req.send().await.context("OData GET failed")?;
    let body = resp.text().await.context("Failed to read response")?;
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
    let mut req = client.post(url)
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

/// Spawn the Discord channel transport.
fn spawn_discord_transport(
    bot_token: String,
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
            intents: intents::DEFAULT,
            webhook_port: 0, // Auto-assign
        };
        let transport = DiscordTransport::new(config, api);
        if let Err(e) = transport.run().await {
            tracing::error!("Discord transport fatal error: {e}");
        }
    });
}
