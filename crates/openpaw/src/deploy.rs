//! Cloud deployment workflow for OpenPaw.

use std::process::Command;
use std::time::Duration;

use anyhow::{Context, Result};

use crate::config::Config;

pub async fn run_deploy(config: Config, with_datadog: bool) -> Result<()> {
    cliclack::intro("Open Paw Deploy")?;

    cliclack::log::info("All services use free tiers — no credit card required.")?;

    cliclack::log::step("Checking prerequisites...")?;
    ensure_or_install("railway", &install_railway)?;
    ensure_or_install("turso", &install_turso)?;
    ensure_or_install("wrangler", &install_wrangler)?;

    ensure_auth_railway()?;
    ensure_auth_turso()?;
    ensure_auth_wrangler()?;

    let owner = slugify(
        &std::env::var("USER")
            .or_else(|_| std::env::var("USERNAME"))
            .unwrap_or_else(|_| "openpaw".to_string()),
    );
    let project_name = format!("openpaw-{owner}");
    let database_name = format!("openpaw-{owner}");
    let bucket_name = format!("openpaw-fs-{owner}");

    cliclack::log::step("Provisioning Turso database (free tier: 9 GB, 500M rows)...")?;
    create_turso_db_idempotent(&database_name)?;
    let turso_url = capture_trimmed("turso", &["db", "show", &database_name, "--url"])?;
    let turso_auth_token = capture_trimmed("turso", &["db", "tokens", "create", &database_name])?;

    cliclack::log::step("Provisioning R2 bucket (free tier: 10 GB storage)...")?;
    create_r2_bucket_idempotent(&bucket_name)?;

    // R2 API token credentials can't be created via CLI.
    let r2_token_url = "https://dash.cloudflare.com/?to=/:account/r2/api-tokens";
    cliclack::log::info(format!(
        "Now create R2 credentials so Paw can read/write files.\n  \
         A browser tab is opening to the R2 API tokens page.\n\n  \
         1. Click \"Create API token\"\n  \
         2. Token name: openpaw\n  \
         3. Permissions: Object Read & Write\n  \
         4. Under \"Specify bucket(s)\": Apply to specific buckets only\n     \
            → choose \"{bucket_name}\"\n  \
         5. TTL: leave as \"No expiration\"\n  \
         6. Click \"Create API Token\"\n  \
         7. You'll see an Access Key ID and a Secret Access Key\n  \
         8. The endpoint URL is also shown (looks like\n     \
            https://abc123.r2.cloudflarestorage.com)\n  \
         9. Paste all three values below"
    ))?;
    let _ = Command::new("open").arg(r2_token_url).status();

    let blob_access_key: String = cliclack::input("R2 Access Key ID")
        .validate(|input: &String| {
            if input.trim().is_empty() {
                Err("Required — shown on the token page you just created")
            } else {
                Ok(())
            }
        })
        .interact()?;
    let blob_secret_key: String = cliclack::password("R2 Secret Access Key")
        .mask('•')
        .validate(|input: &String| {
            if input.trim().is_empty() {
                Err("Required — shown right below the Access Key ID")
            } else {
                Ok(())
            }
        })
        .interact()?;
    let blob_endpoint: String = cliclack::input("R2 endpoint URL")
        .placeholder("https://<account-id>.r2.cloudflarestorage.com")
        .validate(|input: &String| {
            if input.trim().is_empty() {
                Err("Required — shown on the same token page, or on the R2 bucket overview")
            } else {
                Ok(())
            }
        })
        .interact()?;

    cliclack::log::step("Creating Railway project (free tier: 512 MB RAM, 1 vCPU)...")?;
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
        anyhow::bail!(
            "wrangler requires npm. Install Node.js first: https://nodejs.org"
        )
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

fn is_cli_logged_in(command: &str, check_args: &[&str]) -> bool {
    Command::new(command)
        .args(check_args)
        .output()
        .map(|output| {
            if !output.status.success() {
                return false;
            }
            let combined = format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr),
            );
            !combined.contains("not logged in") && !combined.contains("please login")
        })
        .unwrap_or(false)
}

/// Railway: check existing session, fall back to `railway login --browserless`
/// which prints a pairing code — no browser callback needed.
fn ensure_auth_railway() -> Result<()> {
    if is_cli_logged_in("railway", &["whoami"]) {
        cliclack::log::success("railway authenticated ✓")?;
        return Ok(());
    }

    cliclack::log::info("Not logged in to Railway. Pairing with a code...")?;
    let status = Command::new("railway")
        .args(["login", "--browserless"])
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status()
        .context("Failed to run railway login")?;

    if !status.success() || !is_cli_logged_in("railway", &["whoami"]) {
        anyhow::bail!("Railway login failed. Run `railway login` manually and retry.");
    }
    cliclack::log::success("railway authenticated ✓")?;
    Ok(())
}

/// Turso (database): check existing session, fall back to token paste.
fn ensure_auth_turso() -> Result<()> {
    if is_cli_logged_in("turso", &["auth", "whoami"]) {
        cliclack::log::success("turso authenticated ✓")?;
        return Ok(());
    }

    cliclack::log::info(
        "Turso provides the database (free tier, no credit card).\n  \
         A browser tab is opening to Turso.\n\n  \
         1. Sign up or log in if prompted\n  \
         2. Go to API Tokens (the URL ends with /api-tokens)\n  \
         3. Click \"Create Token\" → name it \"openpaw\" → Create\n  \
         4. Copy the token and paste it below"
    )?;
    let _ = Command::new("open")
        .arg("https://app.turso.tech")
        .status();

    let token: String = cliclack::password("Paste Turso API token")
        .mask('•')
        .validate(|input: &String| {
            if input.trim().is_empty() {
                Err("Required — paste the token you just created on turso.tech")
            } else {
                Ok(())
            }
        })
        .interact()?;

    unsafe { std::env::set_var("TURSO_API_TOKEN", token.trim()); }

    if !is_cli_logged_in("turso", &["auth", "whoami"]) {
        anyhow::bail!(
            "That token didn't work. Go to https://app.turso.tech → API Tokens, create a new one, and retry."
        );
    }
    cliclack::log::success("turso authenticated ✓")?;
    Ok(())
}

/// Cloudflare (file storage): check existing session, fall back to token paste.
fn ensure_auth_wrangler() -> Result<()> {
    if is_cli_logged_in("wrangler", &["whoami"]) {
        cliclack::log::success("wrangler authenticated ✓")?;
        return Ok(());
    }

    let token_url = "https://dash.cloudflare.com/profile/api-tokens";
    cliclack::log::info(format!(
        "Cloudflare R2 provides file storage (free tier: 10 GB, no credit card).\n  \
         A browser tab is opening to create an API token.\n\n  \
         1. Sign up or log in if prompted\n  \
         2. Click \"Create Token\"\n  \
         3. Scroll to \"Create Custom Token\" at the bottom → click \"Get started\"\n  \
         4. Token name: openpaw\n  \
         5. Under Permissions, select:\n     \
            Account → Workers R2 Storage → Edit\n  \
         6. Click \"Continue to summary\" → \"Create Token\"\n  \
         7. Copy the token and paste it below"
    ))?;
    let _ = Command::new("open").arg(token_url).status();

    let token: String = cliclack::password("Paste Cloudflare API token")
        .mask('•')
        .validate(|input: &String| {
            if input.trim().is_empty() {
                Err("Required — paste the token you just created on Cloudflare")
            } else {
                Ok(())
            }
        })
        .interact()?;

    unsafe { std::env::set_var("CLOUDFLARE_API_TOKEN", token.trim()); }

    if !is_cli_logged_in("wrangler", &["whoami"]) {
        anyhow::bail!(
            "That token didn't work. Make sure you selected \"Workers R2 Storage → Edit\".\n  \
             Go to {token_url}, create a new token, and retry."
        );
    }
    cliclack::log::success("wrangler authenticated ✓")?;
    Ok(())
}

/// Create a Turso database, treating "already exists" as success.
fn create_turso_db_idempotent(database_name: &str) -> Result<()> {
    let output = Command::new("turso")
        .args(["db", "create", database_name, "--wait"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .output()
        .context("Failed to run turso db create")?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let combined = format!("{stdout}{stderr}");
    if combined.contains("already exists") {
        cliclack::log::success(format!("Database {database_name} already exists ✓"))?;
        return Ok(());
    }

    anyhow::bail!("Failed to create Turso database: {combined}");
}

/// Create an R2 bucket, treating "already exists" as success.
fn create_r2_bucket_idempotent(bucket_name: &str) -> Result<()> {
    let output = Command::new("wrangler")
        .args(["r2", "bucket", "create", bucket_name])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .output()
        .context("Failed to run wrangler r2 bucket create")?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    if stderr.contains("already exists") || stderr.contains("10004") {
        cliclack::log::success(format!("Bucket {bucket_name} already exists ✓"))?;
        return Ok(());
    }

    anyhow::bail!("Failed to create R2 bucket: {stderr}");
}

/// Run a command with full terminal access (stdin/stdout/stderr inherited).
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
