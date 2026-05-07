const DD_ACTION_RECORD_EVIDENCE_LABEL: &str = "TemperPaw.Patrol.RecordEvidence";
const DD_ACTION_COMPLETE_LABEL: &str = "TemperPaw.Patrol.Complete";
const DD_ACTION_ESCALATE_LABEL: &str = "TemperPaw.Patrol.Escalate";
const DATADOG_PATROL_RESULT_BEGIN: &str = "DATADOG_PATROL_RESULT_JSON_BEGIN";
const DATADOG_PATROL_RESULT_END: &str = "DATADOG_PATROL_RESULT_JSON_END";
const DATADOG_PATROL_REQUIRED_SURFACES: &[&str] = &[
    "monitors",
    "logs",
    "traces",
    "metrics",
    "incidents",
    "dashboards",
];

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
        "running Codex Datadog MCP Patrol"
    );

    let investigation = match investigate_datadog_with_codex(config, worker_run, patrol_run_id).await
    {
        Ok(investigation) => investigation,
        Err(error) => {
            post_entity_action(
                client,
                config,
                "PatrolRuns",
                patrol_run_id,
                "Escalate",
                json!({
                    "error_message": format!("Codex Datadog MCP investigation failed: {error}"),
                    "integration": DD_ACTION_ESCALATE_LABEL,
                }),
            )
            .await?;
            return Ok(format!(
                "Datadog Patrol escalated for PatrolRun {patrol_run_id}: Codex Datadog MCP investigation failed."
            ));
        }
    };

    let mut signal_ids = Vec::new();
    let mut finding_ids = Vec::new();
    let mut case_ids = Vec::new();
    let mut work_cycle_ids = Vec::new();
    let mut implementer_worker_run_ids = Vec::new();

    for finding in investigation.findings.iter().take(8) {
        let evidence = datadog_finding_evidence_json(patrol_run_id, &investigation, finding);
        let signal_id = create_tdata_entity(
            client,
            config,
            "Signals",
            json!({
                "fields": {
                    "source": "datadog_mcp",
                    "payload": evidence.to_string(),
                    "source_url": finding.source_url,
                    "severity": finding.severity
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
                "summary": finding.title,
                "severity": finding.severity,
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
                    "Datadog MCP Patrol found actionable {} evidence: {}",
                    finding.primary_surface(),
                    finding.title
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
                "title": finding.title,
                "severity": finding.severity,
                "risk_lane": finding.risk_lane,
                "source": "datadog_mcp",
                "datadog_monitor_id": finding.datadog_monitor_id,
                "evidence_json": evidence.to_string(),
                "affected_services": serde_json::to_string(&finding.affected_services)?,
                "fingerprint": finding.fingerprint,
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
                "summary": finding.title,
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
                "minimum_risk_lane": finding.risk_lane,
                "risk_floor_source": "datadog_patrol:mcp_agent_investigation",
                "risk_evidence": evidence.to_string(),
            }),
        )
        .await?;

        let work_cycle_id = create_tdata_entity(client, config, "WorkCycles", json!({})).await?;
        let task_detail = datadog_followup_task(patrol_run_id, finding, &investigation);
        post_entity_action(
            client,
            config,
            "WorkCycles",
            &work_cycle_id,
            "Configure",
            json!({
                "factory_case_id": case_id,
                "pm_issue_id": "",
                "task_summary": finding.work_summary,
                "task_detail": task_detail,
                "risk_lane": finding.risk_lane,
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
                "plan_summary": "Investigate the Datadog MCP evidence, reproduce or explain the runtime issue, then make the smallest Temper-native fix with red-green tests, targeted live/E2E evidence, independent review, evaluation gates, and a visual ProofPacket.",
            }),
        )
        .await?;
        if finding.requires_start_approval() {
            post_entity_action(
                client,
                config,
                "WorkCycles",
                &work_cycle_id,
                "RequestHumanStartApproval",
                json!({
                    "approval_summary": format!(
                        "Datadog MCP Patrol classified this as {} / {}; approve before code or deploy changes are queued. Finding: {}",
                        finding.severity, finding.risk_lane, finding.title
                    ),
                }),
            )
            .await?;
        } else {
            let implementer_worker_run_id =
                create_tdata_entity(client, config, "WorkerRuns", json!({})).await?;
            let branch_name = datadog_followup_branch_name(finding, &work_cycle_id);
            let worktree_path = datadog_followup_worktree_path(config, &branch_name);
            post_entity_action(
                client,
                config,
                "WorkCycles",
                &work_cycle_id,
                "StartWork",
                json!({}),
            )
            .await?;
            post_entity_action(
                client,
                config,
                "WorkerRuns",
                &implementer_worker_run_id,
                "Configure",
                json!({
                    "work_cycle_id": work_cycle_id,
                    "factory_case_id": case_id,
                    "risk_lane": finding.risk_lane,
                    "task": task_detail,
                    "branch_name": branch_name,
                    "worktree_path": worktree_path,
                    "runner_kind": "local_codex",
                    "allowed_worker_id": nonempty_string(&worker_run.allowed_worker_id, &config.worker_id),
                    "provider_id": nonempty_string(&worker_run.provider_id, "local-codex"),
                    "required_capabilities": "local_codex,repo_write,datadog_query",
                }),
            )
            .await?;
            post_entity_action(
                client,
                config,
                "WorkCycles",
                &work_cycle_id,
                "AttachWorkerRun",
                json!({ "implementer_worker_run_id": implementer_worker_run_id }),
            )
            .await?;
            implementer_worker_run_ids.push(implementer_worker_run_id);
        }
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
        "evidence_source": "codex_datadog_mcp_agent",
        "agent_contract": {
            "begin_marker": DATADOG_PATROL_RESULT_BEGIN,
            "end_marker": DATADOG_PATROL_RESULT_END,
            "required_surfaces": DATADOG_PATROL_REQUIRED_SURFACES,
        },
        "summary": investigation.summary,
        "evidence_scope": investigation.evidence_scope,
        "finding_count": investigation.findings.len(),
        "findings": investigation.findings,
        "residual_risks": investigation.residual_risks,
        "recommended_next_queries": investigation.recommended_next_queries,
        "created": {
            "signals": &signal_ids,
            "observability_findings": &finding_ids,
            "factory_cases": &case_ids,
            "work_cycles": &work_cycle_ids,
            "implementer_worker_runs": &implementer_worker_run_ids,
        }
    }))
    .context("serialize Datadog MCP Patrol evidence")?;

    post_entity_action(
        client,
        config,
        "PatrolRuns",
        patrol_run_id,
        "RecordEvidence",
        json!({
            "evidence_json": evidence_json.clone(),
            "observability_finding_ids": serde_json::to_string(&finding_ids)?,
            "signal_ids": serde_json::to_string(&signal_ids)?,
            "factory_case_ids": serde_json::to_string(&case_ids)?,
            "work_cycle_ids": serde_json::to_string(&work_cycle_ids)?,
        }),
    )
    .await?;

    let proof_packet_id = create_tdata_entity(client, config, "ProofPackets", json!({})).await?;
    let proof_input = DatadogProofSummaryInput {
        patrol_run_id,
        worker_run_id: &worker_run.id,
        investigation: &investigation,
        signal_ids: &signal_ids,
        finding_ids: &finding_ids,
        case_ids: &case_ids,
        work_cycle_ids: &work_cycle_ids,
        implementer_worker_run_ids: &implementer_worker_run_ids,
    };
    let proof_summary = datadog_proof_summary_markdown(&proof_input);
    let state_diagram = datadog_state_diagram_mermaid();
    let visual_summary_url =
        datadog_visual_summary_url(investigation.evidence_scope.len(), finding_ids.len(), work_cycle_ids.len());
    post_entity_action(
        client,
        config,
        "ProofPackets",
        &proof_packet_id,
        "AttachDraft",
        json!({
            "work_cycle_id": work_cycle_ids.first().cloned().unwrap_or_default(),
            "worker_run_id": &worker_run.id,
            "review_run_id": "",
            "evaluation_run_id": "",
            "summary_markdown": proof_summary,
            "proof_json": evidence_json.clone(),
            "visual_summary_url": visual_summary_url,
            "state_diagram_mermaid": state_diagram,
            "changed_files_map": "Datadog MCP Patrol does not edit repository files. Actionable findings become WorkCycles with their own implementation, review, evaluation, and proof loop.",
            "reviewer_verdict": "Patrol evidence packet generated from the local Codex agent's Datadog MCP investigation. Follow-up implementation WorkCycles require independent review before completion.",
            "residual_risks": investigation.residual_risks.join("\n"),
        }),
    )
    .await?;
    post_entity_action(
        client,
        config,
        "ProofPackets",
        &proof_packet_id,
        "MarkReady",
        json!({
            "summary_markdown": datadog_proof_summary_markdown(&proof_input),
            "proof_json": evidence_json.clone(),
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
                "Datadog MCP Patrol investigated {} surface(s), opened {} observability finding(s), and queued {} low-risk implementer run(s).",
                investigation.evidence_scope.len(),
                finding_ids.len(),
                implementer_worker_run_ids.len()
            ),
            "proof_packet_id": proof_packet_id,
            "completed_at": generated_at_label(),
        }),
    )
    .await?;

    Ok(format!(
        "Datadog MCP Patrol completed for PatrolRun {patrol_run_id}: {} surface(s), {} finding(s), {} FactoryCase(s), {} WorkCycle(s), {} low-risk implementer WorkerRun(s). Actions used: {DD_ACTION_RECORD_EVIDENCE_LABEL}, {DD_ACTION_COMPLETE_LABEL}.",
        investigation.evidence_scope.len(),
        finding_ids.len(),
        case_ids.len(),
        work_cycle_ids.len(),
        implementer_worker_run_ids.len()
    ))
}

async fn investigate_datadog_with_codex(
    config: &Config,
    worker_run: &WorkerRunState,
    patrol_run_id: &str,
) -> Result<DatadogPatrolInvestigation> {
    if !config.enable_execution {
        bail!(
            "Datadog MCP Patrol requires PAW_CODEX_ENABLE_EXECUTION=1 so Codex can use its authenticated Datadog MCP tools"
        );
    }

    let workdir = ensure_worktree(config, worker_run).await?;
    let prompt = datadog_mcp_patrol_prompt(patrol_run_id, worker_run);
    let output = run_codex_exec_command(config, &workdir, prompt, "run Codex Datadog MCP Patrol")
        .await?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !output.status.success() {
        bail!(
            "Codex Datadog MCP Patrol failed with status {:?}: {}{}stderr bytes={}",
            output.status.code(),
            truncate_middle(&stdout, 2_000),
            if stdout.trim().is_empty() { "" } else { "\n" },
            stderr.len()
        );
    }

    parse_datadog_patrol_investigation_output(&stdout)
}

fn datadog_mcp_patrol_prompt(patrol_run_id: &str, worker_run: &WorkerRunState) -> String {
    format!(
        r#"You are the Datadog MCP Risk Patrol agent for TemperPaw / OpenPaw.

PatrolRun: {patrol_run_id}
WorkerRun: {worker_run_id}
PatrolKind: datadog_observability

Use your authenticated Datadog MCP tools to actively investigate production observability. Do not read, echo, or print secret values. Do not edit files.
Keep each Datadog MCP call compact: use max_tokens <= 12000, aggregate before sampling raw events, and summarize the evidence instead of copying long result tables.

Required Datadog MCP investigation surfaces:
1. monitors: active alert/warn/no-data monitor states related to OpenPaw, Temper, TemperPaw, Railway, Discord, OData, WASM, Cedar, workers, and dashboards.
2. logs: recent production errors, raw trace leaks into Discord/user surfaces, worker failures, trigger failures, WASM panics, and OData/action errors.
3. traces: APM traces and spans for Discord DMs, webhook triggers, Temper actions, WASM integrations, worker claims/reports, dashboard/OData failures, and Railway runtime errors.
4. metrics: error-rate, latency, restart, memory, saturation, queue, and request-volume signals that suggest current regressions or instability.
5. incidents: open or recent Datadog incidents/events relevant to OpenPaw, Temper, TemperPaw, Discord, Railway, workers, or integrations.
6. dashboards: relevant OpenPaw/Temper/TemperPaw dashboards or queries that reveal current runtime health.

Create findings only for actionable issues that are present or strongly evidenced now. If a surface is unavailable through MCP, still include that surface in evidence_scope with result_summary explaining the limitation. High-risk or production-impacting fixes should set requires_human_approval=true.

Return exactly one JSON object between these markers, with no markdown inside the markers:
{begin}
{{
  "summary": "Concise human-readable patrol summary.",
  "evidence_scope": [
    {{"surface":"monitors","query":"what you asked Datadog MCP","result_summary":"what you learned","datadog_url":"optional Datadog URL"}},
    {{"surface":"logs","query":"...","result_summary":"...","datadog_url":""}},
    {{"surface":"traces","query":"...","result_summary":"...","datadog_url":""}},
    {{"surface":"metrics","query":"...","result_summary":"...","datadog_url":""}},
    {{"surface":"incidents","query":"...","result_summary":"...","datadog_url":""}},
    {{"surface":"dashboards","query":"...","result_summary":"...","datadog_url":""}}
  ],
  "findings": [
    {{
      "title": "Actionable issue title",
      "severity": "info|warn|error|critical",
      "risk_lane": "L0|L1|L2|L3",
      "source_url": "Datadog URL if available",
      "datadog_monitor_id": "optional monitor id",
      "fingerprint": "stable datadog:mcp:<surface>:<issue> id",
      "affected_services": ["service names or surfaces"],
      "evidence_json": {{"surface":"logs","facts":["short factual evidence"]}},
      "work_summary": "Smallest useful follow-up work title",
      "work_detail": "Implementation agent instructions with tests/live verification expected",
      "requires_human_approval": true
    }}
  ],
  "residual_risks": ["What the patrol could not prove"],
  "recommended_next_queries": ["Specific next Datadog MCP query if more proof is needed"]
}}
{end}

The JSON must be valid. Keep findings to at most eight."#,
        patrol_run_id = patrol_run_id,
        worker_run_id = worker_run.id,
        begin = DATADOG_PATROL_RESULT_BEGIN,
        end = DATADOG_PATROL_RESULT_END,
    )
}

fn parse_datadog_patrol_investigation_output(output: &str) -> Result<DatadogPatrolInvestigation> {
    let json_text = extract_datadog_result_json(output)?;
    let mut investigation: DatadogPatrolInvestigation = serde_json::from_str(json_text)
        .context("parse Codex Datadog MCP result JSON")?;
    investigation.normalize()?;
    Ok(investigation)
}

fn extract_datadog_result_json(output: &str) -> Result<&str> {
    let (_, after_begin) = output
        .split_once(DATADOG_PATROL_RESULT_BEGIN)
        .context("Codex Datadog MCP output was missing result begin marker")?;
    let (json_text, _) = after_begin
        .split_once(DATADOG_PATROL_RESULT_END)
        .context("Codex Datadog MCP output was missing result end marker")?;
    let json_text = json_text.trim();
    if json_text.is_empty() {
        bail!("Codex Datadog MCP result JSON was empty");
    }
    Ok(json_text)
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
struct DatadogPatrolInvestigation {
    summary: String,
    #[serde(default)]
    evidence_scope: Vec<DatadogEvidenceScope>,
    #[serde(default)]
    findings: Vec<DatadogPatrolFinding>,
    #[serde(default)]
    residual_risks: Vec<String>,
    #[serde(default)]
    recommended_next_queries: Vec<String>,
}

impl DatadogPatrolInvestigation {
    fn normalize(&mut self) -> Result<()> {
        self.summary = trimmed_or(
            &self.summary,
            "Datadog MCP Patrol completed without a summary.",
        );
        for scope in &mut self.evidence_scope {
            scope.normalize();
        }
        require_datadog_surfaces(&self.evidence_scope)?;

        for finding in &mut self.findings {
            finding.normalize();
        }
        self.findings
            .retain(|finding| !finding.title.trim().is_empty());
        if self.residual_risks.is_empty() {
            self.residual_risks
                .push("No residual risks were reported by the Datadog MCP Patrol agent.".to_string());
        } else {
            normalize_string_vec(&mut self.residual_risks);
        }
        normalize_string_vec(&mut self.recommended_next_queries);
        Ok(())
    }
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
struct DatadogEvidenceScope {
    surface: String,
    query: String,
    #[serde(default)]
    result_summary: String,
    #[serde(default)]
    datadog_url: String,
}

impl DatadogEvidenceScope {
    fn normalize(&mut self) {
        self.surface = self.surface.trim().to_ascii_lowercase();
        self.query = trimmed_or(&self.query, "(query not recorded)");
        self.result_summary = trimmed_or(&self.result_summary, "(result summary not recorded)");
        self.datadog_url = self.datadog_url.trim().to_string();
    }
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
struct DatadogPatrolFinding {
    title: String,
    severity: String,
    risk_lane: String,
    #[serde(default)]
    source_url: String,
    #[serde(default)]
    datadog_monitor_id: String,
    #[serde(default)]
    fingerprint: String,
    #[serde(default)]
    affected_services: Vec<String>,
    #[serde(default = "empty_json_object")]
    evidence_json: Value,
    #[serde(default)]
    work_summary: String,
    #[serde(default)]
    work_detail: String,
    #[serde(default = "default_true")]
    requires_human_approval: bool,
}

impl DatadogPatrolFinding {
    fn normalize(&mut self) {
        self.title = self.title.trim().to_string();
        self.severity = normalize_datadog_severity(&self.severity);
        self.risk_lane = normalize_risk_lane(&self.risk_lane, &self.severity);
        self.source_url = self.source_url.trim().to_string();
        self.datadog_monitor_id = self.datadog_monitor_id.trim().to_string();
        normalize_string_vec(&mut self.affected_services);
        if self.affected_services.is_empty() {
            self.affected_services.push("temperpaw".to_string());
        }
        self.work_summary = trimmed_or(&self.work_summary, &self.title);
        self.work_detail = trimmed_or(
            &self.work_detail,
            "Investigate the Datadog MCP evidence, make the smallest safe fix, and provide tests plus live/E2E proof.",
        );
        if self.fingerprint.trim().is_empty() {
            self.fingerprint = format!("datadog:mcp:{}", stable_slug(&self.title));
        } else {
            self.fingerprint = self.fingerprint.trim().to_string();
        }
        if matches!(self.risk_lane.as_str(), "L2" | "L3")
            || matches!(self.severity.as_str(), "error" | "critical")
        {
            self.requires_human_approval = true;
        }
    }

    fn primary_surface(&self) -> &str {
        self.evidence_json
            .get("surface")
            .and_then(Value::as_str)
            .unwrap_or("observability")
    }

    fn requires_start_approval(&self) -> bool {
        self.requires_human_approval
            || matches!(self.risk_lane.as_str(), "L2" | "L3")
            || matches!(self.severity.as_str(), "error" | "critical")
    }
}

fn percent_encode_data_url(input: &str) -> String {
    let mut encoded = String::new();
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            b' ' => encoded.push_str("%20"),
            _ => {
                encoded.push('%');
                encoded.push(HEX[(byte >> 4) as usize] as char);
                encoded.push(HEX[(byte & 0x0f) as usize] as char);
            }
        }
    }
    encoded
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

fn datadog_finding_evidence_json(
    patrol_run_id: &str,
    investigation: &DatadogPatrolInvestigation,
    finding: &DatadogPatrolFinding,
) -> Value {
    json!({
        "source": "datadog_mcp",
        "patrol_run_id": patrol_run_id,
        "summary": investigation.summary,
        "evidence_scope": investigation.evidence_scope,
        "finding": finding,
    })
}

fn datadog_followup_task(
    patrol_run_id: &str,
    finding: &DatadogPatrolFinding,
    investigation: &DatadogPatrolInvestigation,
) -> String {
    format!(
        "You are the local Codex implementer for a Paw Patrol Datadog MCP observability finding.\n\nPatrolRun: {patrol_run_id}\nPatrol kind: datadog_observability\nFinding: {}\nSeverity: {}\nRisk lane: {}\nSource URL: {}\n\nPatrol summary:\n{}\n\nEvidence JSON:\n{}\n\nRequired loop:\n1. Work in the assigned git worktree and branch only after this WorkCycle is allowed to start.\n2. Use Datadog MCP read-only evidence to reproduce or explain the issue; run extra Datadog MCP queries when needed.\n3. Keep all orchestration Temper-native: specs, WASM integrations, Cedar policies, dashboard views, and Temper actions.\n4. Make the smallest safe fix with red-green TDD, then run focused tests and live/E2E checks.\n5. Produce a visual ProofPacket with state diagrams, OData links, Datadog links, tests, residual risks, and reviewer/evaluator verdicts.\n\nAgent-provided work detail:\n{}",
        finding.title,
        finding.severity,
        finding.risk_lane,
        trimmed_or(&finding.source_url, "(not provided)"),
        investigation.summary,
        datadog_finding_evidence_json(patrol_run_id, investigation, finding),
        finding.work_detail,
    )
}

fn datadog_followup_branch_name(finding: &DatadogPatrolFinding, work_cycle_id: &str) -> String {
    let mut title = stable_slug(&finding.title);
    if title.len() > 40 {
        title.truncate(40);
        title = title.trim_matches('-').to_string();
    }
    if title.is_empty() {
        title = "observability".to_string();
    }
    let suffix = stable_slug(work_cycle_id)
        .chars()
        .take(8)
        .collect::<String>();
    format!("codex/paw-datadog-{title}-{suffix}")
}

fn datadog_followup_worktree_path(config: &Config, branch_name: &str) -> String {
    config
        .workspace_root
        .join(branch_name.replace('/', "-"))
        .display()
        .to_string()
}

fn require_datadog_surfaces(scopes: &[DatadogEvidenceScope]) -> Result<()> {
    let present = scopes
        .iter()
        .map(|scope| scope.surface.as_str())
        .collect::<std::collections::HashSet<_>>();
    let missing = DATADOG_PATROL_REQUIRED_SURFACES
        .iter()
        .copied()
        .filter(|surface| !present.contains(surface))
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        bail!(
            "Codex Datadog MCP result did not include required evidence_scope surface(s): {}",
            missing.join(", ")
        );
    }
    Ok(())
}

fn normalize_datadog_severity(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "critical" | "crit" => "critical".to_string(),
        "error" | "err" | "high" => "error".to_string(),
        "warn" | "warning" | "medium" => "warn".to_string(),
        "info" | "low" | "" => "info".to_string(),
        _ => "warn".to_string(),
    }
}

fn normalize_risk_lane(value: &str, severity: &str) -> String {
    match value.trim().to_ascii_uppercase().as_str() {
        "L0" | "L1" | "L2" | "L3" => value.trim().to_ascii_uppercase(),
        _ => match severity {
            "critical" => "L3".to_string(),
            "error" => "L2".to_string(),
            "warn" => "L1".to_string(),
            _ => "L0".to_string(),
        },
    }
}

fn empty_json_object() -> Value {
    json!({})
}

fn default_true() -> bool {
    true
}

fn trimmed_or(value: &str, fallback: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        fallback.to_string()
    } else {
        value.to_string()
    }
}

fn normalize_string_vec(values: &mut Vec<String>) {
    let mut seen = std::collections::HashSet::new();
    values.retain_mut(|value| {
        *value = value.trim().to_string();
        !value.is_empty() && seen.insert(value.clone())
    });
}

fn stable_slug(value: &str) -> String {
    let mut slug = String::new();
    let mut last_dash = false;
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            slug.push('-');
            last_dash = true;
        }
    }
    slug.trim_matches('-').to_string()
}

fn nonempty_string(value: &str, fallback: &str) -> String {
    let value = value.trim();
    if value.is_empty() {
        fallback.to_string()
    } else {
        value.to_string()
    }
}
