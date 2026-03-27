//! Open Paw — Agent daemon built on Temper platform.
//!
//! Boots an embedded Temper platform, installs Paw OS apps,
//! seeds agent souls, and starts the Discord transport.

mod config;
mod startup;
mod webhooks;

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,openpaw=debug".into()),
        )
        .init();

    tracing::info!("Open Paw starting...");

    let config = config::Config::from_env()?;
    startup::run(config).await
}
