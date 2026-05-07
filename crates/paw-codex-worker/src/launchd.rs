fn launchd_worker_binary_path() -> PathBuf {
    if let Ok(path) = env::var("PAW_CODEX_WORKER_BIN") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }

    env::current_exe().unwrap_or_else(|_| PathBuf::from("paw-codex-worker"))
}

fn render_launchd_plist(config: &Config, worker_bin: &Path, eval_commands: Option<&str>) -> String {
    let mut env_vars = vec![
        ("TEMPER_URL", config.temper_url.as_str()),
        ("TEMPER_TENANT", config.tenant.as_str()),
        ("WORKER_ID", config.worker_id.as_str()),
        ("WORKSPACE_ROOT", path_str(&config.workspace_root)),
        ("REPO_ROOT", path_str(&config.repo_root)),
        ("CODEX_BIN", config.codex_bin.as_str()),
        ("PATH", launchd_path().leak()),
        ("MAX_CONCURRENT_RUNS", "1"),
        (
            "PAW_CODEX_ENABLE_EXECUTION",
            if config.enable_execution { "1" } else { "0" },
        ),
        (
            "PAW_CODEX_POLL_ON_START",
            if config.poll_on_start { "1" } else { "0" },
        ),
        (
            "PAW_CODEX_DOCTOR_EXEC_SMOKE",
            if config.codex_exec_smoke { "1" } else { "0" },
        ),
        (
            "PAW_CODEX_EXEC_TIMEOUT_SECS",
            config.codex_exec_timeout.as_secs().to_string().leak(),
        ),
        (
            "PAW_CODEX_WORKER_CAPABILITIES",
            env::var("PAW_CODEX_WORKER_CAPABILITIES")
                .ok()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| {
                    "local_codex,repo_write,review,evaluation,datadog_query".to_string()
                })
                .leak(),
        ),
        (
            "PAW_CODEX_WORKER_ENV_FILE",
            env::var("PAW_CODEX_WORKER_ENV_FILE")
                .ok()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| {
                    "/Users/openclaw/.config/temperpaw/paw-codex-worker.env".to_string()
                })
                .leak(),
        ),
        (
            "RUST_LOG",
            env::var("RUST_LOG")
                .ok()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| "paw_codex_worker=info,info".to_string())
                .leak(),
        ),
    ];

    if env::var("PAW_CODEX_ALLOW_SECRET_PLIST")
        .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
        && let Some(token) = &config.worker_token
    {
        env_vars.push(("WORKER_TOKEN", token.as_str()));
    }
    if let Ok(path) = env::var("TEMPER_EVENTS_PATH")
        && !path.trim().is_empty()
    {
        env_vars.push(("TEMPER_EVENTS_PATH", path.leak()));
    }
    if let Some(commands) = eval_commands.filter(|value| !value.trim().is_empty()) {
        env_vars.push(("PAW_CODEX_EVAL_COMMANDS", commands));
    }

    let environment = env_vars
        .into_iter()
        .map(|(key, value)| {
            format!(
                "    <key>{}</key>\n    <string>{}</string>",
                escape_plist(key),
                escape_plist(value)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>com.temperpaw.paw-codex-worker</string>
  <key>ProgramArguments</key>
  <array>
    <string>{}</string>
    <string>run</string>
  </array>
  <key>RunAtLoad</key>
  <true/>
  <key>KeepAlive</key>
  <true/>
  <key>EnvironmentVariables</key>
  <dict>
{}
  </dict>
  <key>StandardOutPath</key>
  <string>/tmp/paw-codex-worker.out.log</string>
  <key>StandardErrorPath</key>
  <string>/tmp/paw-codex-worker.err.log</string>
</dict>
</plist>"#,
        escape_plist(&worker_bin.display().to_string()),
        environment
    )
}

fn launchd_path() -> String {
    env::var("PAW_CODEX_LAUNCHD_PATH")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| {
            "/opt/homebrew/bin:/opt/homebrew/sbin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin:/Users/openclaw/.cargo/bin"
                .to_string()
        })
}

fn path_str(path: &Path) -> &str {
    path.to_str().unwrap_or("")
}

fn escape_plist(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn doctor_status_label(status: DoctorStatus) -> &'static str {
    match status {
        DoctorStatus::Pass => "pass",
        DoctorStatus::Warn => "warn",
        DoctorStatus::Fail => "fail",
    }
}
