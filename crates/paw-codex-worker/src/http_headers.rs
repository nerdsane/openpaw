fn extract_sse_data(frame: &str) -> Option<String> {
    let lines: Vec<&str> = frame
        .lines()
        .filter_map(|line| line.strip_prefix("data:").map(str::trim))
        .collect();
    if lines.is_empty() {
        None
    } else {
        Some(lines.join("\n"))
    }
}

fn headers(config: &Config) -> Result<HeaderMap> {
    let mut headers = HeaderMap::new();
    let principal_kind =
        env::var("PAW_CODEX_PRINCIPAL_KIND").unwrap_or_else(|_| "agent".to_string());
    let agent_type = env::var("PAW_CODEX_AGENT_TYPE").unwrap_or_else(|_| "worker".to_string());
    headers.insert(
        "x-tenant-id",
        HeaderValue::from_str(&config.tenant).context("invalid TEMPER_TENANT")?,
    );
    headers.insert(
        "x-temper-principal-id",
        HeaderValue::from_str(&config.worker_id).context("invalid WORKER_ID")?,
    );
    headers.insert(
        "x-temper-principal-kind",
        HeaderValue::from_str(&principal_kind).context("invalid PAW_CODEX_PRINCIPAL_KIND")?,
    );
    headers.insert(
        "x-temper-agent-type",
        HeaderValue::from_str(&agent_type).context("invalid PAW_CODEX_AGENT_TYPE")?,
    );
    headers.insert(
        "x-agent-id",
        HeaderValue::from_str(&config.worker_id).context("invalid WORKER_ID")?,
    );
    if let Some(token) = &config.worker_token {
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {token}")).context("invalid WORKER_TOKEN")?,
        );
    }
    Ok(headers)
}

fn event_stream_headers(config: &Config) -> Result<HeaderMap> {
    let mut headers = HeaderMap::new();
    headers.insert(
        "x-tenant-id",
        HeaderValue::from_str(&config.tenant).context("invalid TEMPER_TENANT")?,
    );
    if let Ok(kind) = env::var("PAW_CODEX_EVENT_STREAM_PRINCIPAL_KIND")
        && !kind.trim().is_empty()
    {
        let principal_id = env::var("PAW_CODEX_EVENT_STREAM_PRINCIPAL_ID")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| config.worker_id.clone());
        headers.insert(
            "x-temper-principal-kind",
            HeaderValue::from_str(kind.trim())
                .context("invalid PAW_CODEX_EVENT_STREAM_PRINCIPAL_KIND")?,
        );
        headers.insert(
            "x-temper-principal-id",
            HeaderValue::from_str(principal_id.trim())
                .context("invalid PAW_CODEX_EVENT_STREAM_PRINCIPAL_ID")?,
        );
    }
    if let Some(token) = &config.worker_token {
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {token}")).context("invalid WORKER_TOKEN")?,
        );
    } else {
        headers.insert("x-temper-principal-kind", HeaderValue::from_static("admin"));
        headers.insert(
            "x-temper-principal-id",
            HeaderValue::from_static("paw-codex-worker-event-stream"),
        );
    }
    Ok(headers)
}

fn required_env(key: &str) -> Result<String> {
    env::var(key).with_context(|| format!("{key} is required"))
}
