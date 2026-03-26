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
];

/// Run the Open Paw daemon.
pub async fn run(config: Config) -> Result<()> {
    let port = config.port;
    let tenant = config.tenant.clone();

    // Phase 1: Storage backend (Turso)
    tracing::info!("Phase 1: Initializing storage...");
    // TODO: Initialize Turso storage from config.turso_url
    // For MVP, start with in-memory (no event store)

    // Phase 2: Build empty registry
    tracing::info!("Phase 2: Building spec registry...");
    let registry = temper_server::SpecRegistry::new();

    // Phase 3: Set OS apps directory
    tracing::info!("Phase 3: Loading OS apps from ./os-apps/...");
    let os_apps_dir = PathBuf::from("os-apps");
    if os_apps_dir.exists() {
        temper_platform::os_apps::set_os_apps_dir(os_apps_dir);
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
        if tenant != "default" {
            let _ = vault.cache_secret(&tenant, "temper_api_url", api_url);
        }

        // Local sandbox (auto-start if script exists)
        let sandbox_port = port + 10;
        let sandbox_url = if let Ok(url) = std::env::var("SANDBOX_URL") {
            url
        } else {
            let sandbox_script = Path::new("os-apps/paw-agent/sandbox/local_sandbox.py");
            let url = format!("http://127.0.0.1:{sandbox_port}");
            if sandbox_script.exists() {
                let _ = std::fs::create_dir_all("/tmp/paw-sandbox");
                let _ = std::fs::create_dir_all("/workspace");
                match std::process::Command::new("python3")
                    .arg(sandbox_script)
                    .arg("--port")
                    .arg(sandbox_port.to_string())
                    .arg("--workdir")
                    .arg("/tmp/paw-sandbox")
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn()
                {
                    Ok(_) => tracing::info!("Local sandbox: {url} (auto-started)"),
                    Err(e) => tracing::warn!("Failed to start local sandbox: {e}"),
                }
            }
            url
        };
        let _ = vault.cache_secret("default", "sandbox_url", sandbox_url.clone());
        if tenant != "default" {
            let _ = vault.cache_secret(&tenant, "sandbox_url", sandbox_url);
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

    tracing::info!("Open Paw listening on port {actual_port}");
    axum::serve(listener, router).await?;

    Ok(())
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
