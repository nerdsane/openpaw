//! Cloud deployment workflow for OpenPaw.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use anyhow::{Context, Result};

// ---------------------------------------------------------------------------
// Credential cache — persists tokens/keys between deploy runs
// ---------------------------------------------------------------------------

fn cache_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    PathBuf::from(home).join(".local/share/openpaw/deploy_cache.json")
}

fn load_cache() -> HashMap<String, String> {
    let path = cache_path();
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|data| serde_json::from_str(&data).ok())
        .unwrap_or_default()
}

fn save_cache(cache: &HashMap<String, String>) {
    let path = cache_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&path, serde_json::to_string_pretty(cache).unwrap_or_default());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
}

fn cache_get(cache: &HashMap<String, String>, key: &str) -> Option<String> {
    cache.get(key).filter(|v| !v.is_empty()).cloned()
}

fn cache_set(cache: &mut HashMap<String, String>, key: &str, value: &str) {
    cache.insert(key.to_string(), value.to_string());
    save_cache(cache);
}

pub async fn run_deploy(
    dd_api_key: Option<String>,
    dd_app_key: Option<String>,
    dd_site: String,
    with_datadog: bool,
) -> Result<()> {
    cliclack::intro("Open Paw Deploy")?;

    let mut cache = load_cache();

    cliclack::log::info("All services use free tiers — no credit card required.")?;

    cliclack::log::step("Checking prerequisites...")?;
    ensure_or_install("railway", &install_railway)?;
    ensure_or_install("turso", &install_turso)?;
    ensure_or_install("wrangler", &install_wrangler)?;

    ensure_auth_railway()?;
    ensure_auth_turso(&mut cache)?;
    ensure_auth_wrangler(&mut cache)?;

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

    let (blob_access_key, blob_secret_key, blob_endpoint) =
        collect_r2_credentials(&bucket_name, &mut cache)?;

    cliclack::log::step("Creating Railway project (free tier: 512 MB RAM, 1 vCPU)...")?;
    create_railway_project_idempotent(&project_name)?;

    let mut variables = vec![
        format!("TURSO_URL={turso_url}"),
        format!("TURSO_AUTH_TOKEN={turso_auth_token}"),
        format!("BLOB_ENDPOINT={blob_endpoint}"),
        format!("BLOB_BUCKET={bucket_name}"),
        format!("BLOB_ACCESS_KEY={blob_access_key}"),
        format!("BLOB_SECRET_KEY={blob_secret_key}"),
    ];

    if let Some(key) = &dd_api_key {
        variables.push(format!("DD_API_KEY={key}"));
    }
    if let Some(key) = &dd_app_key {
        variables.push(format!("DD_APP_KEY={key}"));
    }
    variables.push(format!("DD_SITE={dd_site}"));

    let mut set_args = vec![
        "variable".to_string(),
        "set".to_string(),
        "-s".to_string(),
        "openpaw".to_string(),
    ];
    set_args.extend(variables);
    run_interactive("railway", &as_str_slice(&set_args))?;

    // Get project/env IDs for all deploys
    let (project_id, env_id) = get_railway_ids()?;

    if with_datadog {
        cliclack::log::step("Deploying OTEL collector → Datadog...")?;

        // Ensure the otel-collector service exists before setting vars
        let _ = Command::new("railway")
            .args(["add", "--service", "otel-collector"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();

        // Set DD_API_KEY and DD_SITE on the otel-collector service
        let mut collector_vars: Vec<String> = Vec::new();
        if let Some(key) = &dd_api_key {
            collector_vars.push(format!("DD_API_KEY={key}"));
        }
        collector_vars.push(format!("DD_SITE={dd_site}"));
        let mut collector_set_args = vec![
            "variable".to_string(),
            "set".to_string(),
            "-s".to_string(),
            "otel-collector".to_string(),
        ];
        collector_set_args.extend(collector_vars);
        run_interactive("railway", &as_str_slice(&collector_set_args))?;

        deploy_otel_collector(&project_id, &env_id)?;

        // Set OTEL endpoint on the openpaw service to point at the collector
        // Railway private networking: <service>.railway.internal
        let otel_vars = vec![
            "OTEL_EXPORTER_OTLP_ENDPOINT=http://otel-collector.railway.internal:4318",
            "OTEL_ENABLED=true",
        ];
        let mut otel_args = vec!["variable", "set", "-s", "openpaw"];
        otel_args.extend(otel_vars);
        run_interactive("railway", &otel_args)?;
    } else {
        // No collector — disable OTEL so the server doesn't spam connection errors
        run_interactive(
            "railway",
            &["variable", "set", "-s", "openpaw", "OTEL_ENABLED=false"],
        )?;
    }

    cliclack::log::step("Deploying OpenPaw...")?;
    deploy_prebuilt_image(&project_id, &env_id)?;

    let domain_output = capture_trimmed("railway", &["domain", "--service", "openpaw", "--json"])?;
    let deploy_url = infer_domain(&domain_output).unwrap_or(domain_output.clone());

    cliclack::log::step("Waiting for health check...")?;
    match poll_health(&deploy_url).await {
        Ok(()) => {
            cliclack::outro(format!(
                "Paw is live → {deploy_url}/dashboard\n  \
                 Open the dashboard to create your account and finish setup."
            ))?;
        }
        Err(_) => {
            anyhow::bail!(
                "Health check timed out — the build may still be running.\n  \
                 Check Railway dashboard: https://railway.com/project → {project_name}\n  \
                 Once it's live, visit: {deploy_url}/dashboard"
            );
        }
    }
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

fn collect_r2_credentials(
    bucket_name: &str,
    cache: &mut HashMap<String, String>,
) -> Result<(String, String, String)> {
    if let (Some(ak), Some(sk), Some(ep)) = (
        cache_get(cache, "r2_access_key"),
        cache_get(cache, "r2_secret_key"),
        cache_get(cache, "r2_endpoint"),
    ) {
        cliclack::log::success("R2 credentials loaded from previous run ✓")?;
        return Ok((ak, sk, ep));
    }

    let cf_account_id = get_cloudflare_account_id();
    let r2_token_url = match &cf_account_id {
        Some(id) => format!("https://dash.cloudflare.com/{id}/r2/api-tokens/create?type=account"),
        None => "https://dash.cloudflare.com/?to=/:account/r2/api-tokens".to_string(),
    };
    cliclack::log::info(format!(
        "Create an R2 API token. Set these two values:\n  \
         Permissions: \x1b[1mObject Read & Write\x1b[0m\n  \
         Bucket: \x1b[1m{bucket_name}\x1b[0m\n  \
         Then paste the three values it gives you below."
    ))?;
    let _ = Command::new("open").arg(&r2_token_url).status();

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

    cache_set(cache, "r2_access_key", blob_access_key.trim());
    cache_set(cache, "r2_secret_key", blob_secret_key.trim());
    cache_set(cache, "r2_endpoint", blob_endpoint.trim());

    Ok((blob_access_key, blob_secret_key, blob_endpoint))
}

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

fn ensure_auth_turso(cache: &mut HashMap<String, String>) -> Result<()> {
    if is_cli_logged_in("turso", &["auth", "whoami"]) {
        cliclack::log::success("turso authenticated ✓")?;
        return Ok(());
    }

    if let Some(cached) = cache_get(cache, "turso_api_token") {
        unsafe { std::env::set_var("TURSO_API_TOKEN", &cached); }
        if is_cli_logged_in("turso", &["auth", "whoami"]) {
            cliclack::log::success("turso authenticated (cached token) ✓")?;
            return Ok(());
        }
    }

    cliclack::log::info(
        "Turso provides the database (free tier, no credit card).\n  \
         Opening Turso — go to API Tokens, create one, and paste it below."
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
    cache_set(cache, "turso_api_token", token.trim());
    cliclack::log::success("turso authenticated ✓")?;
    Ok(())
}

fn ensure_auth_wrangler(cache: &mut HashMap<String, String>) -> Result<()> {
    if is_cli_logged_in("wrangler", &["whoami"]) {
        cliclack::log::success("wrangler authenticated ✓")?;
        return Ok(());
    }

    if let Some(cached) = cache_get(cache, "cloudflare_api_token") {
        unsafe { std::env::set_var("CLOUDFLARE_API_TOKEN", &cached); }
        if is_cli_logged_in("wrangler", &["whoami"]) {
            cliclack::log::success("wrangler authenticated (cached token) ✓")?;
            return Ok(());
        }
    }

    let token_url = "https://dash.cloudflare.com/profile/api-tokens";
    cliclack::log::info(
        "Cloudflare R2 provides file storage (free tier: 10 GB, no credit card).\n  \
         Opening Cloudflare — create a Custom Token with permission:\n  \
         Account → Workers R2 Storage → Edit. Paste it below."
    )?;
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
    cache_set(cache, "cloudflare_api_token", token.trim());
    cliclack::log::success("wrangler authenticated ✓")?;
    Ok(())
}

/// Get Railway project and environment IDs from the linked project.
fn get_railway_ids() -> Result<(String, String)> {
    let status_output = Command::new("railway")
        .args(["status", "--json"])
        .output()
        .context("Failed to get Railway project status")?;
    let status_json: serde_json::Value =
        serde_json::from_slice(&status_output.stdout)
            .context("Failed to parse Railway status JSON")?;
    let project_id = status_json["id"]
        .as_str()
        .context("No project ID in Railway status")?
        .to_string();
    let env_id = status_json["environments"]["edges"]
        .as_array()
        .and_then(|edges| edges.first())
        .and_then(|e| e["node"]["id"].as_str())
        .context("No environment ID in Railway status")?
        .to_string();
    Ok((project_id, env_id))
}

/// Deploy the pre-built Docker image from GHCR instead of building from source.
fn deploy_prebuilt_image(project_id: &str, env_id: &str) -> Result<()> {
    let image = "ghcr.io/nerdsane/openpaw:latest";
    let tmp = std::env::temp_dir().join("openpaw-deploy");
    let _ = std::fs::create_dir_all(&tmp);
    std::fs::write(tmp.join("Dockerfile"), format!("FROM {image}\n"))?;
    std::fs::write(
        tmp.join("railway.toml"),
        "[build]\nbuilder = \"dockerfile\"\ndockerfilePath = \"Dockerfile\"\n\n\
         [deploy]\nhealthcheckPath = \"/healthz\"\nhealthcheckTimeout = 300\n\
         restartPolicyType = \"ON_FAILURE\"\nrestartPolicyMaxRetries = 3\n",
    )?;

    let status = Command::new("railway")
        .args(["up", "-s", "openpaw", "-p", project_id, "-e", env_id])
        .current_dir(&tmp)
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status()
        .context("Failed to run railway up")?;

    let _ = std::fs::remove_dir_all(&tmp);

    if !status.success() {
        anyhow::bail!("Railway deploy failed");
    }
    Ok(())
}

/// Deploy the OTEL collector as a Railway service with the Datadog config baked in.
/// The service must already exist (created by the caller).
fn deploy_otel_collector(project_id: &str, env_id: &str) -> Result<()> {
    let tmp = std::env::temp_dir().join("openpaw-otel-deploy");
    let _ = std::fs::create_dir_all(&tmp);

    // Dockerfile that bakes in the OTEL collector config
    std::fs::write(
        tmp.join("Dockerfile"),
        "FROM otel/opentelemetry-collector-contrib:latest\n\
         COPY otel-config.yaml /etc/otelcol-contrib/config.yaml\n",
    )?;

    // The OTEL collector config — OTLP receiver → Datadog exporter
    std::fs::write(
        tmp.join("otel-config.yaml"),
        "receivers:\n\
         \x20 otlp:\n\
         \x20   protocols:\n\
         \x20     grpc:\n\
         \x20       endpoint: 0.0.0.0:4317\n\
         \x20     http:\n\
         \x20       endpoint: 0.0.0.0:4318\n\
         \n\
         processors:\n\
         \x20 batch:\n\
         \x20   send_batch_size: 1000\n\
         \x20   timeout: 5s\n\
         \n\
         exporters:\n\
         \x20 datadog:\n\
         \x20   api:\n\
         \x20     key: ${env:DD_API_KEY}\n\
         \x20     site: ${env:DD_SITE}\n\
         \n\
         service:\n\
         \x20 pipelines:\n\
         \x20   traces:\n\
         \x20     receivers: [otlp]\n\
         \x20     processors: [batch]\n\
         \x20     exporters: [datadog]\n\
         \x20   metrics:\n\
         \x20     receivers: [otlp]\n\
         \x20     processors: [batch]\n\
         \x20     exporters: [datadog]\n\
         \x20   logs:\n\
         \x20     receivers: [otlp]\n\
         \x20     processors: [batch]\n\
         \x20     exporters: [datadog]\n",
    )?;

    std::fs::write(
        tmp.join("railway.toml"),
        "[build]\nbuilder = \"dockerfile\"\ndockerfilePath = \"Dockerfile\"\n\n\
         [deploy]\nrestartPolicyType = \"ON_FAILURE\"\nrestartPolicyMaxRetries = 3\n",
    )?;

    let status = Command::new("railway")
        .args(["up", "-s", "otel-collector", "-p", project_id, "-e", env_id])
        .current_dir(&tmp)
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status()
        .context("Failed to deploy OTEL collector")?;

    let _ = std::fs::remove_dir_all(&tmp);

    if !status.success() {
        anyhow::bail!("OTEL collector deploy failed");
    }

    cliclack::log::success("OTEL collector deployed ✓")?;
    Ok(())
}

fn create_railway_project_idempotent(project_name: &str) -> Result<()> {
    let linked = Command::new("railway")
        .args(["status", "--json"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if linked {
        cliclack::log::success("Railway project already linked ✓")?;
    } else {
        let status = Command::new("railway")
            .args(["init", "--name", project_name])
            .stdin(std::process::Stdio::inherit())
            .stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit())
            .status()
            .context("Failed to run railway init")?;

        if !status.success() {
            let link_status = Command::new("railway")
                .args(["link"])
                .stdin(std::process::Stdio::inherit())
                .stdout(std::process::Stdio::inherit())
                .stderr(std::process::Stdio::inherit())
                .status()
                .context("Failed to run railway link")?;

            if !link_status.success() {
                anyhow::bail!("Could not create or link to Railway project \"{project_name}\".");
            }
        }
    }

    let _ = Command::new("railway")
        .args(["add", "--service", "openpaw"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();

    Ok(())
}

fn get_cloudflare_account_id() -> Option<String> {
    let output = Command::new("wrangler")
        .args(["whoami"])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    for line in text.lines() {
        for word in line.split_whitespace() {
            let clean = word.trim_matches('│').trim_matches('|').trim();
            if clean.len() == 32 && clean.chars().all(|c| c.is_ascii_hexdigit()) {
                return Some(clean.to_string());
            }
        }
    }
    None
}

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

fn capture_trimmed(command: &str, args: &[&str]) -> Result<String> {
    let output = Command::new(command)
        .args(args)
        .stdin(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .output()
        .with_context(|| format!("Failed to run `{command} {}`", args.join(" ")))?;

    if !output.status.success() {
        anyhow::bail!("`{command} {}` failed", args.join(" "));
    }

    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if text.is_empty() {
        anyhow::bail!("`{command} {}` returned no output", args.join(" "));
    }
    Ok(text.lines().last().unwrap_or_default().trim().to_string())
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

    for _ in 0..90 {
        if let Ok(response) = client.get(&health_url).send().await {
            if response.status().is_success() {
                return Ok(());
            }
        }
        tokio::time::sleep(Duration::from_secs(10)).await;
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
