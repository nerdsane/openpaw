mod deploy;

use clap::{Parser, Subcommand};
use std::process::Command as Cmd;

#[derive(Parser)]
#[command(name = "openpaw", about = "Open Paw — agent platform")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run OpenPaw locally — configure API keys, messaging, and boot the server
    Run,
    /// Deploy OpenPaw to the cloud (Railway + Turso + R2)
    Deploy {
        /// Add the Datadog collector sidecar service
        #[arg(long)]
        with_datadog: bool,
    },
    /// Diagnose configuration and show what's working
    Doctor,
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Run => {
            run_server(&["run"])?;
        }
        Command::Deploy { with_datadog } => {
            let dd_api_key = optional_env("DD_API_KEY");
            let dd_app_key = optional_env("DD_APP_KEY");
            let dd_site =
                std::env::var("DD_SITE").unwrap_or_else(|_| "datadoghq.com".to_string());
            deploy::run_deploy(dd_api_key, dd_app_key, dd_site, with_datadog).await?;
        }
        Command::Doctor => {
            run_server(&["doctor"])?;
        }
    }

    Ok(())
}

/// Find and exec openpaw-server, forwarding the subcommand.
fn run_server(args: &[&str]) -> anyhow::Result<()> {
    // Look for openpaw-server next to this binary first, then on PATH
    let self_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()));

    let server_bin = self_dir
        .as_ref()
        .map(|d| d.join("openpaw-server"))
        .filter(|p| p.exists())
        .unwrap_or_else(|| "openpaw-server".into());

    let status = Cmd::new(&server_bin)
        .args(args)
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status();

    match status {
        Ok(s) if s.success() => Ok(()),
        Ok(s) => std::process::exit(s.code().unwrap_or(1)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            eprintln!();
            eprintln!("  openpaw-server not found.");
            eprintln!();
            eprintln!("  Build it first:  \x1b[1mcargo build -p openpaw --release\x1b[0m");
            eprintln!();
            std::process::exit(1);
        }
        Err(e) => anyhow::bail!("Failed to start openpaw-server: {e}"),
    }
}

fn optional_env(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}
