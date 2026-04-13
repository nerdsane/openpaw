//! Open Paw — Agent daemon built on Temper platform.
//!
//! Boots an embedded Temper platform, installs Paw OS apps,
//! seeds agent souls, and starts the Discord transport.

mod auth;
mod config;
mod deploy;
mod setup;
mod setup_api;
mod setup_llm;
mod startup;
mod transport_manager;

use clap::{Parser, Subcommand};
use std::io::IsTerminal;

#[derive(Parser)]
#[command(name = "openpaw", about = "Open Paw — agent platform")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Set up OpenPaw — API keys, messaging, soul, and optionally deploy to the cloud
    Setup {
        /// Add the Datadog collector sidecar service when deploying
        #[arg(long)]
        with_datadog: bool,
    },
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
        Some(Command::Setup { with_datadog }) => {
            // Phase A: collect API key + messaging config
            let result = setup::run_setup_config(&config).await?;
            setup::merge_setup_into_config(&mut config, result);

            // Ask: run locally or deploy to the cloud?
            if std::io::stdin().is_terminal() {
                let choice: &str = cliclack::select("What would you like to do?")
                    .item("local", "Run locally", "Boot the server on this machine")
                    .item("deploy", "Deploy to the cloud", "Provision infrastructure and deploy to Railway")
                    .interact()?;

                if choice == "deploy" {
                    deploy::run_deploy(config, with_datadog).await?;
                    return Ok(());
                }
            }

            true // local path: force soul personalization after boot
        }
        Some(Command::Doctor) => {
            setup::run_doctor(&data_dir, &config);
            return Ok(());
        }
        None => {
            // No subcommand: just boot the server.
            // If not configured yet, tell the user what to do.
            if std::io::stdin().is_terminal() && setup::needs_setup(&data_dir, &config) {
                eprintln!();
                eprintln!("  Open Paw is not configured yet.");
                eprintln!();
                eprintln!("  Run \x1b[1mopenpaw setup\x1b[0m to get started.");
                eprintln!();
                std::process::exit(1);
            }
            false
        }
    };

    // In a terminal, suppress noisy logs — only show warnings and errors.
    // Full debug logs when RUST_LOG is explicitly set or not in a terminal.
    let is_terminal = std::io::stderr().is_terminal();
    if std::env::var_os("RUST_LOG").is_none() {
        let level = if is_terminal { "warn" } else { "info,openpaw=debug" };
        unsafe { std::env::set_var("RUST_LOG", level); }
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

    // Print a clean banner in the terminal
    if is_terminal {
        let port = config.port;
        eprintln!();
        eprintln!("  \x1b[1mOpen Paw\x1b[0m is starting...");
        eprintln!();
        eprintln!("  Dashboard → \x1b[36mhttp://localhost:{port}/dashboard\x1b[0m");
        eprintln!("  API       → \x1b[36mhttp://localhost:{port}\x1b[0m");
        eprintln!();
        eprintln!("  Logs are suppressed. Set \x1b[1mRUST_LOG=info\x1b[0m for verbose output.");
        eprintln!();
    } else {
        tracing::info!("Open Paw starting...");
    }

    let result = startup::run(config, force_soul_setup).await;

    if let Some(guard) = otel_guard {
        guard.shutdown();
    }

    result
}
