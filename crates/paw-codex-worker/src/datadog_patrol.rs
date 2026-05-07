const DD_ACTION_OPEN_FINDING_LABEL: &str = "TemperPaw.Patrol.OpenFinding";
const DD_ACTION_RECORD_EVIDENCE_LABEL: &str = "TemperPaw.Patrol.RecordEvidence";
const DD_ACTION_COMPLETE_LABEL: &str = "TemperPaw.Patrol.Complete";
const DD_ACTION_ESCALATE_LABEL: &str = "TemperPaw.Patrol.Escalate";

async fn run_datadog_patrol(
    client: &reqwest::Client,
    config: &Config,
    worker_run: &WorkerRunState,
    patrol_run_id: &str,
) -> Result<String> {
    info!(
        worker_run_id = %worker_run.id,
        patrol_run_id,
        patrol_kind = "datadog_observability",
        "running Datadog Patrol"
    );

    let dd = match datadog_config_from_env() {
        Ok(dd) => dd,
        Err(error) => {
            post_entity_action(
                client,
                config,
                "PatrolRuns",
                patrol_run_id,
                "Escalate",
                json!({
                    "error_message": error.to_string(),
                    "integration": DD_ACTION_ESCALATE_LABEL,
                }),
            )
            .await?;
            return Ok(format!(
                "Datadog Patrol escalated for PatrolRun {patrol_run_id}: Datadog read-only keys were not available to the worker."
            ));
        }
    };

    let monitor_search = query_datadog_monitor_search(client, &dd).await?;
    let codex_analysis = analyze_datadog_evidence_with_codex(config, worker_run, &monitor_search)
        .await
        .unwrap_or_else(|error| format!("Codex Datadog analysis was unavailable: {error}"));
    let active_monitors = active_datadog_monitors(&monitor_search);
    let mut signal_ids = Vec::new();
    let mut finding_ids = Vec::new();
    let mut case_ids = Vec::new();
    let mut work_cycle_ids = Vec::new();

    for monitor in active_monitors.iter().take(5) {
        let evidence = json!({
            "source": "datadog_observability",
            "patrol_run_id": patrol_run_id,
            "monitor": monitor,
            "monitor_search_endpoint": "/api/v1/monitor/search"
        });
        let signal_id = create_tdata_entity(
            client,
            config,
            "Signals",
            json!({
                "fields": {
                    "source": "datadog",
                    "payload": evidence.to_string(),
                    "source_url": monitor.url,
                    "severity": monitor.severity
                }
            }),
        )
        .await?;
        post_entity_action(
            client,
            config,
            "Signals",
            &signal_id,
            "Normalize",
            json!({
                "summary": monitor.summary(),
                "severity": monitor.severity,
            }),
        )
        .await?;
        post_entity_action(
            client,
            config,
            "Signals",
            &signal_id,
            "Triage",
            json!({
                "summary": format!(
                    "Datadog Patrol found active monitor state {} for {}.",
                    monitor.status, monitor.name
                ),
            }),
        )
        .await?;

        let finding_id = create_tdata_entity(client, config, "ObservabilityFindings", json!({})).await?;
        post_entity_action(
            client,
            config,
            "ObservabilityFindings",
            &finding_id,
            "OpenFinding",
            json!({
                "title": monitor.summary(),
                "severity": monitor.severity,
                "risk_lane": monitor.risk_lane(),
                "source": "datadog",
                "datadog_monitor_id": monitor.id,
                "evidence_json": evidence.to_string(),
                "affected_services": monitor.tags_json(),
                "fingerprint": monitor.fingerprint(),
                "patrol_run_id": patrol_run_id,
                "signal_id": signal_id,
            }),
        )
        .await?;

        let case_id = create_tdata_entity(client, config, "FactoryCases", json!({})).await?;
        post_entity_action(
            client,
            config,
            "FactoryCases",
            &case_id,
            "Open",
            json!({
                "summary": monitor.summary(),
                "signal_id": signal_id,
                "patrol_request_id": "",
                "work_request_id": "",
            }),
        )
        .await?;
        post_entity_action(
            client,
            config,
            "FactoryCases",
            &case_id,
            "SetRiskFloor",
            json!({
                "minimum_risk_lane": monitor.risk_lane(),
                "risk_floor_source": "datadog_patrol:active_monitor",
                "risk_evidence": evidence.to_string(),
            }),
        )
        .await?;

        let work_cycle_id = create_tdata_entity(client, config, "WorkCycles", json!({})).await?;
        let task_detail = datadog_followup_task(patrol_run_id, monitor, &evidence);
        post_entity_action(
            client,
            config,
            "WorkCycles",
            &work_cycle_id,
            "Configure",
            json!({
                "factory_case_id": case_id,
                "pm_issue_id": "",
                "task_summary": monitor.summary(),
                "task_detail": task_detail,
                "risk_lane": monitor.risk_lane(),
            }),
        )
        .await?;
        post_entity_action(
            client,
            config,
            "WorkCycles",
            &work_cycle_id,
            "LinkSource",
            json!({
                "source_entity_type": "ObservabilityFinding",
                "source_entity_id": finding_id,
            }),
        )
        .await?;
        post_entity_action(
            client,
            config,
            "WorkCycles",
            &work_cycle_id,
            "WritePlan",
            json!({
                "plan_summary": "Investigate Datadog evidence, reproduce or explain the active alert, then make the smallest Temper-native fix with tests, live evidence, reviewer approval, and a visual ProofPacket. Production-impacting changes pause before code/deploy work.",
            }),
        )
        .await?;
        post_entity_action(
            client,
            config,
            "WorkCycles",
            &work_cycle_id,
            "RequestHumanStartApproval",
            json!({
                "approval_summary": format!(
                    "Datadog monitor {} is currently {}; approve before code or deploy changes are queued.",
                    monitor.name, monitor.status
                ),
            }),
        )
        .await?;
        post_entity_action(
            client,
            config,
            "FactoryCases",
            &case_id,
            "OpenWorkCycle",
            json!({ "work_cycle_id": work_cycle_id }),
        )
        .await?;
        post_entity_action(
            client,
            config,
            "Signals",
            &signal_id,
            "AttachCase",
            json!({ "factory_case_id": case_id }),
        )
        .await?;

        signal_ids.push(signal_id);
        finding_ids.push(finding_id);
        case_ids.push(case_id);
        work_cycle_ids.push(work_cycle_id);
    }

    let evidence_json = serde_json::to_string(&json!({
        "kind": "datadog_observability",
        "datadog_endpoint": "/api/v1/monitor/search",
        "monitor_count": datadog_monitor_count(&monitor_search),
        "active_monitor_count": active_monitors.len(),
        "active_monitors": &active_monitors,
        "codex_analysis": codex_analysis,
        "created": {
            "signals": &signal_ids,
            "observability_findings": &finding_ids,
            "factory_cases": &case_ids,
            "work_cycles": &work_cycle_ids,
        }
    }))
    .context("serialize Datadog Patrol evidence")?;

    post_entity_action(
        client,
        config,
        "PatrolRuns",
        patrol_run_id,
        "RecordEvidence",
        json!({
            "evidence_json": evidence_json,
            "observability_finding_ids": serde_json::to_string(&finding_ids)?,
            "signal_ids": serde_json::to_string(&signal_ids)?,
            "factory_case_ids": serde_json::to_string(&case_ids)?,
            "work_cycle_ids": serde_json::to_string(&work_cycle_ids)?,
        }),
    )
    .await?;
    post_entity_action(
        client,
        config,
        "PatrolRuns",
        patrol_run_id,
        "Complete",
        json!({
            "summary": format!(
                "Datadog Patrol checked monitors and opened {} observability finding(s).",
                finding_ids.len()
            ),
            "proof_packet_id": "",
            "completed_at": generated_at_label(),
        }),
    )
    .await?;

    Ok(format!(
        "Datadog Patrol completed for PatrolRun {patrol_run_id}: {} active monitor(s), {} finding(s), {} FactoryCase(s), {} WorkCycle(s). Actions used: {DD_ACTION_OPEN_FINDING_LABEL}, {DD_ACTION_RECORD_EVIDENCE_LABEL}, {DD_ACTION_COMPLETE_LABEL}.",
        active_monitors.len(),
        finding_ids.len(),
        case_ids.len(),
        work_cycle_ids.len()
    ))
}

fn extract_datadog_patrol_run_id(task: &str) -> Option<String> {
    if !task.contains("datadog_observability") {
        return None;
    }
    task.lines().find_map(|line| {
        line.trim()
            .strip_prefix("PatrolRun:")
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    })
}

async fn analyze_datadog_evidence_with_codex(
    config: &Config,
    worker_run: &WorkerRunState,
    monitor_search: &Value,
) -> Result<String> {
    if !config.enable_execution {
        return Ok(
            "dry-run: deterministic Datadog evidence was collected; Codex analysis is disabled until PAW_CODEX_ENABLE_EXECUTION=1."
                .to_string(),
        );
    }

    let workdir = ensure_worktree(config, worker_run).await?;
    let prompt = format!(
        "You are the Datadog Risk Patrol analyst for TemperPaw.\n\nReview this read-only Datadog monitor evidence and return a concise, human-readable triage memo. Include:\n- highest priority issue\n- likely affected service or surface\n- whether code work should be opened now or should wait for human approval\n- any extra Datadog queries you would run with the authenticated Datadog MCP if needed\n\nEvidence JSON:\n{}\n\nDo not edit files. Do not print secrets.",
        truncate_middle(&monitor_search.to_string(), 8_000)
    );
    let output = run_codex_exec_command(
        config,
        &workdir,
        prompt,
        "run Datadog Patrol Codex analysis",
    )
    .await?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !output.status.success() {
        bail!(
            "Datadog Patrol Codex analysis failed with status {:?}: {}{}",
            output.status.code(),
            truncate_middle(&stdout, 2_000),
            truncate_middle(&stderr, 2_000)
        );
    }
    Ok(truncate_middle(
        format!("{}\n{}", stdout.trim(), stderr.trim()).trim(),
        4_000,
    ))
}

#[derive(Clone, Debug)]
struct DatadogConfig {
    api_key: String,
    app_key: String,
    site: String,
}

fn datadog_config_from_env() -> Result<DatadogConfig> {
    let mut missing = Vec::new();
    let api_key = env::var("DD_API_KEY").unwrap_or_default();
    if api_key.trim().is_empty() {
        missing.push("DD_API_KEY");
    }
    let app_key = env::var("DD_APP_KEY").unwrap_or_default();
    if app_key.trim().is_empty() {
        missing.push("DD_APP_KEY");
    }
    if !missing.is_empty() {
        bail!("missing required Datadog secret(s): {}", missing.join(", "));
    }
    let site = env::var("DD_SITE")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "datadoghq.com".to_string());
    Ok(DatadogConfig {
        api_key,
        app_key,
        site,
    })
}

async fn query_datadog_monitor_search(
    client: &reqwest::Client,
    dd: &DatadogConfig,
) -> Result<Value> {
    let url = format!("{}/api/v1/monitor/search", datadog_api_base(&dd.site));
    let response = client
        .get(&url)
        .header(ACCEPT, "application/json")
        .header("DD-API-KEY", &dd.api_key)
        .header("DD-APPLICATION-KEY", &dd.app_key)
        .query(&[
            ("query", ""),
            ("page", "0"),
            ("per_page", "50"),
        ])
        .send()
        .await
        .context("query Datadog monitor search")?;
    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        bail!(
            "Datadog /api/v1/monitor/search returned {status}: {}",
            truncate_middle(&text, 1_000)
        );
    }
    response.json().await.context("parse Datadog monitor search")
}

fn datadog_api_base(site: &str) -> String {
    let site = site
        .trim()
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/');
    let host = if site.starts_with("api.") {
        site.to_string()
    } else if let Some(rest) = site.strip_prefix("app.") {
        format!("api.{rest}")
    } else {
        format!("api.{site}")
    };
    format!("https://{host}")
}

#[derive(Clone, Debug, serde::Serialize)]
struct ActiveDatadogMonitor {
    id: String,
    name: String,
    status: String,
    severity: String,
    url: String,
    tags: Vec<String>,
}

impl ActiveDatadogMonitor {
    fn summary(&self) -> String {
        format!("Datadog monitor active: {} ({})", self.name, self.status)
    }

    fn risk_lane(&self) -> &'static str {
        match self.status.to_ascii_lowercase().as_str() {
            "alert" | "triggered" => "L2",
            "warn" | "warning" | "no data" => "L1",
            _ => "L1",
        }
    }

    fn tags_json(&self) -> String {
        serde_json::to_string(&self.tags).unwrap_or_else(|_| "[]".to_string())
    }

    fn fingerprint(&self) -> String {
        format!("datadog:monitor:{}", self.id)
    }
}

fn active_datadog_monitors(search: &Value) -> Vec<ActiveDatadogMonitor> {
    search
        .get("monitors")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
        .iter()
        .filter_map(active_datadog_monitor)
        .collect()
}

fn active_datadog_monitor(value: &Value) -> Option<ActiveDatadogMonitor> {
    let status = string_value_any(
        value,
        &[
            "status",
            "overall_state",
            "overall_state_modified",
            "state",
            "monitor_status",
        ],
    );
    if !is_active_datadog_status(&status) {
        return None;
    }
    let id = value
        .get("id")
        .or_else(|| value.get("monitor_id"))
        .map(|id| match id {
            Value::String(text) => text.clone(),
            Value::Number(number) => number.to_string(),
            other => other.to_string(),
        })
        .unwrap_or_else(|| "unknown".to_string());
    let name = string_value_any(value, &["name", "title"])
        .trim()
        .to_string();
    let url = string_value_any(value, &["url", "link"]);
    let tags = value
        .get("tags")
        .and_then(Value::as_array)
        .map(|tags| {
            tags.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    Some(ActiveDatadogMonitor {
        id,
        name: if name.is_empty() {
            "unnamed Datadog monitor".to_string()
        } else {
            name
        },
        severity: datadog_status_severity(&status).to_string(),
        status,
        url,
        tags,
    })
}

fn is_active_datadog_status(status: &str) -> bool {
    matches!(
        status.trim().to_ascii_lowercase().as_str(),
        "alert" | "triggered" | "warn" | "warning" | "no data" | "no_data"
    )
}

fn datadog_status_severity(status: &str) -> &'static str {
    match status.trim().to_ascii_lowercase().as_str() {
        "alert" | "triggered" => "error",
        "warn" | "warning" => "warn",
        "no data" | "no_data" => "warn",
        _ => "info",
    }
}

fn datadog_monitor_count(search: &Value) -> usize {
    search
        .get("monitors")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or_default()
}

fn string_value_any(value: &Value, keys: &[&str]) -> String {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
        .unwrap_or("")
        .to_string()
}

async fn create_tdata_entity(
    client: &reqwest::Client,
    config: &Config,
    entity_set: &str,
    body: Value,
) -> Result<String> {
    let response = client
        .post(format!("{}/tdata/{entity_set}", config.temper_url))
        .headers(headers(config)?)
        .header(CONTENT_TYPE, "application/json")
        .json(&body)
        .send()
        .await
        .with_context(|| format!("create {entity_set}"))?;
    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        bail!("create {entity_set} returned {status}: {text}");
    }
    let value: Value = response
        .json()
        .await
        .with_context(|| format!("parse {entity_set} create response"))?;
    let fields = value.get("fields").cloned().unwrap_or_else(|| json!({}));
    let entity_id = first_string(&value, &fields, &["entity_id", "id", "Id"], &["id", "Id"]);
    if entity_id.is_empty() {
        bail!("create {entity_set} response was missing entity_id");
    }
    Ok(entity_id)
}

fn datadog_followup_task(
    patrol_run_id: &str,
    monitor: &ActiveDatadogMonitor,
    evidence: &Value,
) -> String {
    format!(
        "You are the local Codex implementer for a Paw Patrol Datadog observability finding.\n\nPatrolRun: {patrol_run_id}\nPatrol kind: datadog_observability\nDatadog monitor: {} ({})\nRisk lane: {}\n\nEvidence JSON:\n{}\n\nRequired loop:\n1. Work in the assigned git worktree and branch only after the WorkCycle is approved to start.\n2. Use Datadog read-only evidence to reproduce or explain the issue.\n3. Keep all orchestration Temper-native: specs, WASM integrations, Cedar policies, and dashboard views.\n4. Make the smallest safe fix with red-green TDD, then run focused tests and live/E2E checks.\n5. Produce a visual ProofPacket with state diagrams, OData links, Datadog links, tests, residual risks, and reviewer/evaluator verdicts.",
        monitor.name,
        monitor.status,
        monitor.risk_lane(),
        evidence
    )
}
