use anyhow::{Context, Result, bail};
use futures_util::StreamExt;
use repo_health::{extract_repo_sweep_snapshot_id, repo_sweep_summary_markdown, scan_repo_health};
use reqwest::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue};
use serde::Deserialize;
use serde_json::{Value, json};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Output, Stdio};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::process::Command;
use tokio::time::{sleep, timeout};
use tracing::{debug, error, info, warn};

mod repo_health;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            env::var("RUST_LOG").unwrap_or_else(|_| "paw_codex_worker=info,info".to_string()),
        )
        .init();

    let command = parse_worker_command(env::args().skip(1));
    let config = Config::from_env()?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(60))
        .build()?;

    if command == WorkerCommand::Doctor {
        return run_doctor(&client, &config).await;
    }
    if command == WorkerCommand::LaunchdPlist {
        let worker_bin = launchd_worker_binary_path();
        let eval_commands = env::var("PAW_CODEX_EVAL_COMMANDS").ok();
        println!(
            "{}",
            render_launchd_plist(&config, &worker_bin, eval_commands.as_deref())
        );
        return Ok(());
    }

    info!(
        worker_id = %config.worker_id,
        temper_url = %config.temper_url,
        tenant = %config.tenant,
        max_concurrent_runs = config.max_concurrent_runs,
        enable_execution = config.enable_execution,
        "paw-codex-worker starting"
    );

    if config.poll_on_start {
        claim_boot_queued_runs(&client, &config).await?;
        claim_boot_requested_review_runs(&client, &config).await?;
        claim_boot_queued_evaluation_runs(&client, &config).await?;
    }

    loop {
        if config.poll_on_start {
            claim_boot_queued_runs(&client, &config).await?;
            claim_boot_requested_review_runs(&client, &config).await?;
            claim_boot_queued_evaluation_runs(&client, &config).await?;
        }
        match timeout(Duration::from_secs(10), watch_events(&client, &config)).await {
            Ok(Ok(())) => {}
            Ok(Err(error)) => warn!(%error, "Temper event stream disconnected"),
            Err(_) => debug!("Temper event stream poll window elapsed; using OData fallback"),
        }
        sleep(Duration::from_secs(1)).await;
    }
}

include!("worker_types.rs");
include!("boot_watch.rs");
include!("doctor.rs");
include!("event_loop.rs");
include!("temper_api.rs");
include!("cli.rs");
include!("doctor_report.rs");
include!("launchd.rs");
include!("doctor_helpers.rs");
include!("execution.rs");
include!("http_headers.rs");
include!("tests.rs");
