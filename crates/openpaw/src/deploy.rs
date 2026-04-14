//! Cloud deployment workflow for OpenPaw.

use std::process::Command;
use std::time::Duration;

use anyhow::{Context, Result};

use crate::config::Config;

pub async fn run_deploy(config: Config, with_datadog: bool) -> Result<()> {
    cliclack::intro("Open Paw Deploy")?;

    cliclack::log::step("Checking prerequisites...")?;
    ensure_or_install("railway", &install_railway)?;
    ensure_or_install("turso", &install_turso)?;
    ensure_or_install("wrangler", &install_wrangler)?;

    ensure_logged_in(
        "railway",
        &["whoami"],
        &["railway", "login", "--browserless"],
    )?;
    ensure_logged_in("turso", &["auth", "whoami"], &["turso", "auth", "login"])?;
    ensure_logged_in("wrangler", &["whoami"], &["wrangler", "login"])?;

    let owner = slugify(
        &std::env::var("USER")
            .or_else(|_| std::env::var("USERNAME"))
            .unwrap_or_else(|_| "openpaw".to_string()),
    );
    let project_name = format!("openpaw-{owner}");
    let database_name = format!("openpaw-{owner}");
    let bucket_name = format!("openpaw-fs-{owner}");

    cliclack::log::step("Provisioning Turso database...")?;
    run_interactive("turso", &["db", "create", &database_name, "--wait"])?;
    let turso_url = capture_trimmed("turso", &["db", "show", &database_name, "--url"])?;
    let turso_auth_token = capture_trimmed("turso", &["db", "tokens", "create", &database_name])?;

    cliclack::log::step("Provisioning R2 bucket...")?;
    run_interactive("wrangler", &["r2", "bucket", "create", &bucket_name])?;
    let blob_access_key: String = cliclack::input("Cloudflare R2 access key ID")
        .placeholder("Create an R2 token in Cloudflare first")
        .interact()?;
    let blob_secret_key: String = cliclack::password("Cloudflare R2 secret access key")
        .mask('•')
        .interact()?;
    let blob_endpoint: String = cliclack::input("Cloudflare R2 S3 endpoint")
        .placeholder("https://<account-id>.r2.cloudflarestorage.com")
        .interact()?;

    cliclack::log::step("Creating Railway project...")?;
    run_interactive("railway", &["init", "--name", &project_name])?;
    let _ = run_interactive("railway", &["add", "--service", "openpaw"]);

    let mut variables = vec![
        format!("TURSO_URL={turso_url}"),
        format!("TURSO_AUTH_TOKEN={turso_auth_token}"),
        format!("BLOB_ENDPOINT={blob_endpoint}"),
        format!("BLOB_BUCKET={bucket_name}"),
        format!("BLOB_ACCESS_KEY={blob_access_key}"),
        format!("BLOB_SECRET_KEY={blob_secret_key}"),
    ];

    if let Some(dd_api_key) = config.dd_api_key.clone() {
        variables.push(format!("DD_API_KEY={dd_api_key}"));
    }
    if let Some(dd_app_key) = config.dd_app_key.clone() {
        variables.push(format!("DD_APP_KEY={dd_app_key}"));
    }
    variables.push(format!("DD_SITE={}", config.dd_site));

    let mut set_args = vec![
        "variable".to_string(),
        "set".to_string(),
        "-s".to_string(),
        "openpaw".to_string(),
    ];
    set_args.extend(variables);
    run_interactive("railway", &as_str_slice(&set_args))?;

    if with_datadog {
        let _ = run_interactive(
            "railway",
            &[
                "add",
                "--service",
                "otel-collector",
                "--image",
                "otel/opentelemetry-collector-contrib:latest",
            ],
        );
    }

    cliclack::log::step("Deploying OpenPaw...")?;
    run_interactive("railway", &["up", "-s", "openpaw", "-d"])?;
    let domain_output = capture_trimmed("railway", &["domain", "--service", "openpaw", "--json"])?;
    let deploy_url = infer_domain(&domain_output).unwrap_or(domain_output);

    cliclack::log::step("Waiting for health check...")?;
    poll_health(&deploy_url).await?;

    cliclack::outro(format!(
        "Paw is live → {deploy_url}/dashboard\n  \
         Open the dashboard to create your account and finish setup."
    ))?;
    Ok(())
}

fn cli_exists(command: &str) -> bool {
    Command::new(command)
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn npm_exists() -> bool {
    cli_exists("npm")
}

fn ensure_or_install(command: &str, installer: &dyn Fn() -> Result<()>) -> Result<()> {
    if cli_exists(command) {
        cliclack::log::success(format!("{command} ✓"))?;
        return Ok(());
    }

    let spinner = cliclack::spinner();
    spinner.start(format!("Installing {command}..."));
    match installer() {
        Ok(()) => {
            if cli_exists(command) {
                spinner.stop(format!("{command} installed ✓"));
                Ok(())
            } else {
                spinner.stop(format!("{command} install finished but command not found"));
                anyhow::bail!(
                    "{command} installed but not on PATH. \
                     You may need to restart your shell and try again."
                );
            }
        }
        Err(e) => {
            spinner.stop(format!("{command} install failed"));
            Err(e.context(format!("Failed to auto-install {command}")))
        }
    }
}

fn install_railway() -> Result<()> {
    if npm_exists() {
        run_install(&["npm", "install", "-g", "@railway/cli"])
    } else {
        // Shell installer fallback
        run_shell_install("bash <(curl -fsSL cli.new)")
    }
}

fn install_turso() -> Result<()> {
    run_shell_install("curl -sSfL https://get.tur.so/install.sh | bash")
}

fn install_wrangler() -> Result<()> {
    if npm_exists() {
        run_install(&["npm", "install", "-g", "wrangler"])
    } else {
        anyhow::bail!("wrangler requires npm. Install Node.js first: https://nodejs.org")
    }
}

fn run_install(args: &[&str]) -> Result<()> {
    let status = Command::new(args[0])
        .args(&args[1..])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .with_context(|| format!("Failed to run `{}`", args.join(" ")))?;
    if !status.success() {
        anyhow::bail!("`{}` exited with {}", args.join(" "), status);
    }
    Ok(())
}

fn run_shell_install(script: &str) -> Result<()> {
    let status = Command::new("sh")
        .arg("-c")
        .arg(script)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .with_context(|| format!("Failed to run: {script}"))?;
    if !status.success() {
        anyhow::bail!("Install script failed: {script}");
    }
    Ok(())
}

fn ensure_logged_in(command: &str, check_args: &[&str], login_cmd: &[&str]) -> Result<()> {
    let output = Command::new(command)
        .args(check_args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();

    match output {
        Ok(status) if status.success() => {
            cliclack::log::success(format!("{command} authenticated ✓"))?;
            return Ok(());
        }
        _ => {}
    }

    cliclack::log::info(format!("Not logged in to {command}. Opening login flow..."))?;

    // Run login interactively (needs user input / browser)
    let status = Command::new(login_cmd[0])
        .args(&login_cmd[1..])
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status()
        .with_context(|| format!("Failed to run `{}`", login_cmd.join(" ")))?;

    if !status.success() {
        anyhow::bail!(
            "Login to {command} failed. Run `{}` manually and try again.",
            login_cmd.join(" ")
        );
    }

    Ok(())
}

/// Run a command with full terminal access (stdin/stdout/stderr inherited).
/// Use for commands that need to detect a TTY (e.g. wrangler).
fn run_interactive(command: &str, args: &[&str]) -> Result<()> {
    let status = Command::new(command)
        .args(args)
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status()
        .with_context(|| format!("Failed to run `{command} {}`", args.join(" ")))?;

    if !status.success() {
        anyhow::bail!("`{command} {}` failed (exit {})", args.join(" "), status);
    }

    Ok(())
}

/// Run a command and capture its stdout. Stderr goes to the terminal.
fn run_checked(command: &str, args: &[&str]) -> Result<String> {
    let output = Command::new(command)
        .args(args)
        .stdin(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .output()
        .with_context(|| format!("Failed to run `{command} {}`", args.join(" ")))?;

    if !output.status.success() {
        anyhow::bail!("`{command} {}` failed", args.join(" "));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn capture_trimmed(command: &str, args: &[&str]) -> Result<String> {
    let output = run_checked(command, args)?;
    if output.is_empty() {
        anyhow::bail!("`{command} {}` returned no output", args.join(" "));
    }
    Ok(output.lines().last().unwrap_or_default().trim().to_string())
}

fn infer_domain(raw: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(raw)
        .ok()
        .and_then(|json| {
            json.get("domain")
                .and_then(|value| value.as_str())
                .map(|domain| format!("https://{domain}"))
        })
}

async fn poll_health(base_url: &str) -> Result<()> {
    let client = reqwest::Client::new();
    let health_url = format!("{base_url}/healthz");

    for _ in 0..30 {
        if let Ok(response) = client.get(&health_url).send().await {
            if response.status().is_success() {
                return Ok(());
            }
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }

    anyhow::bail!("Timed out waiting for {health_url}");
}

fn slugify(input: &str) -> String {
    let slug: String = input
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    slug.trim_matches('-')
        .split('-')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn as_str_slice(values: &[String]) -> Vec<&str> {
    values.iter().map(String::as_str).collect()
}

#[cfg(test)]
mod tests {
    use super::{infer_domain, slugify};

    #[test]
    fn slugify_normalizes_owner_names() {
        assert_eq!(slugify("Seshendra Nalla"), "seshendra-nalla");
        assert_eq!(slugify("OPENPAW_dev"), "openpaw-dev");
    }

    #[test]
    fn infer_domain_reads_railway_json() {
        let value = infer_domain(r#"{"domain":"openpaw-production.up.railway.app"}"#);
        assert_eq!(
            value.as_deref(),
            Some("https://openpaw-production.up.railway.app")
        );
    }
}
