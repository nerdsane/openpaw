const GITHUB_ACTION_RECORD_EVIDENCE_LABEL: &str = "TemperPaw.Patrol.RecordEvidence";
const GITHUB_ACTION_ESCALATE_LABEL: &str = "TemperPaw.Patrol.Escalate";
const GITHUB_PATROL_RESULT_BEGIN: &str = "GITHUB_PATROL_RESULT_JSON_BEGIN";
const GITHUB_PATROL_RESULT_END: &str = "GITHUB_PATROL_RESULT_JSON_END";

async fn run_github_patrol(
    client: &reqwest::Client,
    config: &Config,
    worker_run: &WorkerRunState,
    patrol_run_id: &str,
) -> Result<String> {
    info!(
        worker_run_id = %worker_run.id,
        patrol_run_id,
        patrol_kind = "github_repository",
        "running Codex GitHub Patrol"
    );

    let investigation = match investigate_github_with_codex(config, worker_run, patrol_run_id).await
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
                    "error_message": format!("Codex GitHub repository investigation failed: {error}"),
                    "integration": GITHUB_ACTION_ESCALATE_LABEL,
                }),
            )
            .await?;
            return Ok(format!(
                "GitHub Patrol escalated for PatrolRun {patrol_run_id}: Codex repository investigation failed."
            ));
        }
    };

    let evidence_json = serde_json::to_string(&json!({
        "kind": "github_repository",
        "evidence_source": "codex_github_agent",
        "agent_contract": {
            "begin_marker": GITHUB_PATROL_RESULT_BEGIN,
            "end_marker": GITHUB_PATROL_RESULT_END,
        },
        "summary": investigation.summary,
        "evidence_scope": investigation.evidence_scope,
        "finding_count": investigation.findings.len(),
        "findings": investigation.findings,
        "residual_risks": investigation.residual_risks,
        "recommended_next_queries": investigation.recommended_next_queries
    }))
    .context("serialize GitHub Patrol evidence")?;

    post_entity_action(
        client,
        config,
        "PatrolRuns",
        patrol_run_id,
        "RecordEvidence",
        json!({
            "evidence_json": evidence_json.clone(),
            "observability_finding_ids": "[]",
            "signal_ids": "[]",
            "factory_case_ids": "[]",
            "work_cycle_ids": "[]",
        }),
    )
    .await?;

    Ok(format!(
        "GitHub Patrol reported agent evidence for PatrolRun {patrol_run_id}: {} evidence item(s), {} finding(s). PatrolRun.RecordEvidence now triggers paw-patrol WASM fan-out and completion. Action used: {GITHUB_ACTION_RECORD_EVIDENCE_LABEL}.",
        investigation.evidence_scope.len(),
        investigation.findings.len()
    ))
}

async fn investigate_github_with_codex(
    config: &Config,
    worker_run: &WorkerRunState,
    patrol_run_id: &str,
) -> Result<GitHubPatrolInvestigation> {
    if !config.enable_execution {
        bail!(
            "GitHub Patrol requires PAW_CODEX_ENABLE_EXECUTION=1 so Codex can use authenticated repository tools"
        );
    }

    let workdir = ensure_worktree(config, worker_run).await?;
    let prompt = github_patrol_prompt(patrol_run_id, worker_run);
    let output = run_codex_exec_command(config, &workdir, prompt, "run Codex GitHub Patrol")
        .await?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !output.status.success() {
        bail!(
            "Codex GitHub Patrol failed with status {:?}: {}{}stderr bytes={}",
            output.status.code(),
            truncate_middle(&stdout, 2_000),
            if stdout.trim().is_empty() { "" } else { "\n" },
            stderr.len()
        );
    }

    parse_github_patrol_investigation_output(&stdout)
}

fn github_patrol_prompt(patrol_run_id: &str, worker_run: &WorkerRunState) -> String {
    format!(
        r##"You are the GitHub repository Risk Patrol agent for TemperPaw.

PatrolRun: {patrol_run_id}
WorkerRun: {worker_run_id}
PatrolKind: github_repository
Repository: nerdsane/temperpaw

Use your authenticated GitHub tools from this Codex environment to actively investigate repository maintenance health. Do not edit files and do not mutate GitHub state.
Use intelligent judgment: inspect open issues, open pull requests, checks, reviews, CI/actions, labels, milestones, stale/blocking conversations, duplicate reports, security-relevant reports, and anomalies that should become Patrol work or a daily brief item.
Do not convert every stale or noisy item into work. Create findings only when the evidence is actionable now or when a PR/issue anomaly needs human or agent attention.
For production-impacting, deployment, secrets, policy, security, or user-facing work, set risk_lane to L2/L3 and requires_human_approval=true.

Return exactly one JSON object between these markers, with no markdown inside the markers:
{begin}
{{
  "summary": "Concise human-readable repository patrol summary.",
  "evidence_scope": [
    {{"surface":"open issues","query":"what you inspected","result_summary":"what you learned","github_url":"optional URL"}},
    {{"surface":"open pull requests","query":"what you inspected","result_summary":"what you learned","github_url":"optional URL"}},
    {{"surface":"checks","query":"what you inspected","result_summary":"what you learned","github_url":"optional URL"}},
    {{"surface":"reviews","query":"what you inspected","result_summary":"what you learned","github_url":"optional URL"}},
    {{"surface":"anomalies","query":"what you inspected","result_summary":"what you learned","github_url":"optional URL"}}
  ],
  "findings": [
    {{
      "title": "Actionable issue or PR title",
      "severity": "info|warn|error|critical",
      "risk_lane": "L0|L1|L2|L3",
      "source_url": "GitHub issue, PR, run, or search URL",
      "source_kind": "issue|pull_request|check|review|discussion|repository",
      "fingerprint": "stable github:<kind>:<id-or-topic> id",
      "affected_refs": ["#123", "branch/name", "workflow name"],
      "evidence_json": {{"facts":["short factual evidence"]}},
      "work_summary": "Smallest useful follow-up work title",
      "work_detail": "Implementation/review/triage agent instructions with tests/live verification expected",
      "requires_human_approval": true
    }}
  ],
  "residual_risks": ["What the patrol could not prove"],
  "recommended_next_queries": ["Specific next GitHub query if more proof is needed"]
}}
{end}

The JSON must be valid. Keep findings to at most eight."##,
        patrol_run_id = patrol_run_id,
        worker_run_id = worker_run.id,
        begin = GITHUB_PATROL_RESULT_BEGIN,
        end = GITHUB_PATROL_RESULT_END,
    )
}

fn parse_github_patrol_investigation_output(output: &str) -> Result<GitHubPatrolInvestigation> {
    let json_text = extract_github_result_json(output)?;
    let mut investigation: GitHubPatrolInvestigation =
        serde_json::from_str(json_text).context("parse Codex GitHub Patrol result JSON")?;
    investigation.normalize()?;
    Ok(investigation)
}

fn extract_github_result_json(output: &str) -> Result<&str> {
    let (_, after_begin) = output
        .split_once(GITHUB_PATROL_RESULT_BEGIN)
        .context("Codex GitHub Patrol output was missing result begin marker")?;
    let (json_text, _) = after_begin
        .split_once(GITHUB_PATROL_RESULT_END)
        .context("Codex GitHub Patrol output was missing result end marker")?;
    let json_text = json_text.trim();
    if json_text.is_empty() {
        bail!("Codex GitHub Patrol result JSON was empty");
    }
    Ok(json_text)
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
struct GitHubPatrolInvestigation {
    summary: String,
    #[serde(default)]
    evidence_scope: Vec<GitHubEvidenceScope>,
    #[serde(default)]
    findings: Vec<GitHubPatrolFinding>,
    #[serde(default)]
    residual_risks: Vec<String>,
    #[serde(default)]
    recommended_next_queries: Vec<String>,
}

impl GitHubPatrolInvestigation {
    fn normalize(&mut self) -> Result<()> {
        self.summary = trimmed_or(
            &self.summary,
            "GitHub repository Patrol completed without a summary.",
        );
        for scope in &mut self.evidence_scope {
            scope.normalize();
        }
        if self.evidence_scope.is_empty() {
            self.residual_risks.push(
                "GitHub Patrol returned no evidence_scope; treat this as incomplete agent evidence."
                    .to_string(),
            );
        }

        for finding in &mut self.findings {
            finding.normalize();
        }
        self.findings
            .retain(|finding| !finding.title.trim().is_empty());
        if self.residual_risks.is_empty() {
            self.residual_risks
                .push("No residual risks were reported by the GitHub Patrol agent.".to_string());
        } else {
            normalize_string_vec(&mut self.residual_risks);
        }
        normalize_string_vec(&mut self.recommended_next_queries);
        Ok(())
    }
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
struct GitHubEvidenceScope {
    surface: String,
    query: String,
    #[serde(default)]
    result_summary: String,
    #[serde(default)]
    github_url: String,
}

impl GitHubEvidenceScope {
    fn normalize(&mut self) {
        self.surface = self.surface.trim().to_ascii_lowercase();
        self.query = trimmed_or(&self.query, "(query not recorded)");
        self.result_summary = trimmed_or(&self.result_summary, "(result summary not recorded)");
        self.github_url = self.github_url.trim().to_string();
    }
}

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
struct GitHubPatrolFinding {
    title: String,
    severity: String,
    risk_lane: String,
    #[serde(default)]
    source_url: String,
    #[serde(default)]
    source_kind: String,
    #[serde(default)]
    fingerprint: String,
    #[serde(default)]
    affected_refs: Vec<String>,
    #[serde(default = "empty_json_object")]
    evidence_json: Value,
    #[serde(default)]
    work_summary: String,
    #[serde(default)]
    work_detail: String,
    #[serde(default)]
    requires_human_approval: bool,
}

impl GitHubPatrolFinding {
    fn normalize(&mut self) {
        self.title = self.title.trim().to_string();
        self.severity = normalize_datadog_severity(&self.severity);
        self.risk_lane = normalize_risk_lane(&self.risk_lane, &self.severity);
        self.source_url = self.source_url.trim().to_string();
        self.source_kind = trimmed_or(&self.source_kind, "repository")
            .trim()
            .to_ascii_lowercase();
        normalize_string_vec(&mut self.affected_refs);
        self.work_summary = trimmed_or(&self.work_summary, &self.title);
        self.work_detail = trimmed_or(
            &self.work_detail,
            "Investigate the GitHub evidence, make or request the smallest safe follow-up, and provide tests plus live/E2E proof.",
        );
        if self.fingerprint.trim().is_empty() {
            self.fingerprint = format!("github:{}:{}", self.source_kind, stable_slug(&self.title));
        } else {
            self.fingerprint = self.fingerprint.trim().to_string();
        }
        if matches!(self.risk_lane.as_str(), "L2" | "L3")
            || matches!(self.severity.as_str(), "error" | "critical")
        {
            self.requires_human_approval = true;
        }
    }
}

fn extract_github_patrol_run_id(task: &str) -> Option<String> {
    let task = task.trim_start();
    if !task.starts_with("You are the local Codex GitHub Patrol agent")
        || !task.contains("PatrolKind: github_repository")
    {
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
