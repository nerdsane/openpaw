//! Open Paw — Agent daemon built on Temper platform.
//!
//! Boots an embedded Temper platform, installs Paw OS apps,
//! seeds agent souls, and starts the Discord transport.

mod config;
mod setup;
mod setup_api;
mod setup_llm;
mod startup;
mod transport_manager;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "openpaw", about = "Open Paw — agent platform")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Configure API keys and messaging platforms interactively
    Setup,
    /// Diagnose configuration and show what's working
    Doctor,
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let mut config = config::Config::from_env()?;

    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    let data_dir = std::path::Path::new(&home).join(".local/share/openpaw");
    std::fs::create_dir_all(&data_dir)?;

    let force_soul_setup = match cli.command {
        Some(Command::Setup) => {
            // Phase A: config setup (always runs, even if already configured)
            let result = setup::run_setup_config(&config).await?;
            setup::merge_setup_into_config(&mut config, result);
            true // force soul personalization after boot
        }
        Some(Command::Doctor) => {
            setup::run_doctor(&data_dir, &config);
            return Ok(());
        }
        None => false,
    };

    // Build layered tracing subscriber
    if std::env::var_os("RUST_LOG").is_none() {
        unsafe {
            std::env::set_var("RUST_LOG", "info,openpaw=debug");
        }
    }
    if config.otel_enabled {
        let has_explicit_endpoint = std::env::var_os("OTLP_ENDPOINT").is_some()
            || std::env::var_os("OTEL_EXPORTER_OTLP_ENDPOINT").is_some();
        if !has_explicit_endpoint {
            unsafe {
                std::env::set_var("OTEL_EXPORTER_OTLP_ENDPOINT", &config.otel_endpoint);
            }
        }
    } else {
        unsafe {
            std::env::remove_var("OTLP_ENDPOINT");
            std::env::remove_var("OTEL_EXPORTER_OTLP_ENDPOINT");
        }
    }

    let otel_guard = temper_observe::otel::init_observability("openpaw");
    if config.otel_enabled {
        tracing::info!(
            "Open Paw starting (OpenTelemetry enabled → {})...",
            config.otel_endpoint
        );
    } else {
        tracing::info!("Open Paw starting...");
    }

    let result = startup::run(config, force_soul_setup).await;

    if let Some(guard) = otel_guard {
        guard.shutdown();
    }

    result
}
