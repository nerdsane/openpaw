use crate::{WorkerRunState, worker_run_branch_label};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::json;

const REPO_HEALTH_RESULT_JSON_BEGIN: &str = "REPO_HEALTH_PATROL_RESULT_JSON_BEGIN";
const REPO_HEALTH_RESULT_JSON_END: &str = "REPO_HEALTH_PATROL_RESULT_JSON_END";

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct RepoSweepGraph {
    pub(crate) quality_findings: Vec<QualityFindingRecord>,
    pub(crate) security_findings: Vec<SecurityFindingRecord>,
    pub(crate) summary: RepoSweepSummary,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct QualityFindingRecord {
    #[serde(default)]
    pub(crate) fingerprint: String,
    #[serde(default)]
    pub(crate) title: String,
    #[serde(default)]
    pub(crate) severity: String,
    #[serde(default)]
    pub(crate) evidence: String,
    #[serde(default)]
    pub(crate) affected_paths: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct SecurityFindingRecord {
    #[serde(default)]
    pub(crate) fingerprint: String,
    #[serde(default)]
    pub(crate) title: String,
    #[serde(default)]
    pub(crate) severity: String,
    #[serde(default)]
    pub(crate) risk_lane: String,
    #[serde(default)]
    pub(crate) evidence: String,
    #[serde(default)]
    pub(crate) affected_paths: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub(crate) struct RepoSweepSummary {
    pub(crate) scanned_files: usize,
    pub(crate) scanned_lines: usize,
    pub(crate) giant_modules: usize,
    pub(crate) todo_hack_hits: usize,
    pub(crate) duplicate_logic_candidates: usize,
    pub(crate) broad_cedar_policies: usize,
    pub(crate) dependency_risk_hits: usize,
    pub(crate) rust_orchestration_hits: usize,
    pub(crate) polling_loop_hits: usize,
    pub(crate) missing_test_coverage_hits: usize,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct RepoHealthAgentOutput {
    pub(crate) summary_markdown: String,
    pub(crate) graph: RepoSweepGraph,
    pub(crate) evidence_scope: Vec<RepoHealthEvidenceScope>,
    pub(crate) residual_risks: Vec<String>,
    pub(crate) recommended_next_actions: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct RepoHealthEvidenceScope {
    #[serde(default)]
    pub(crate) surface: String,
    #[serde(default)]
    pub(crate) query_or_command: String,
    #[serde(default)]
    pub(crate) result_summary: String,
}

pub(crate) fn extract_repo_sweep_snapshot_id(task: &str) -> Option<String> {
    task.lines().find_map(|line| {
        line.trim()
            .strip_prefix("RepoGraphSnapshot:")
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    })
}

pub(crate) fn repo_health_agent_prompt(snapshot_id: &str, worker_run: &WorkerRunState) -> String {
    format!(
        "You are the local Codex repo-health Patrol agent for TemperPaw.\n\nRepoGraphSnapshot: {snapshot_id}\nWorkerRun: {}\nBranch: {}\n\nOriginal task from Temper:\n{}\n\nRequired loop:\n1. Work in this assigned worktree. Do not edit files during the patrol scan.\n2. Actively investigate the TemperPaw and deeply coupled Temper surface with agent judgment. Use repo tools such as rg, git, cargo metadata, cargo tree, npm scripts, and targeted source reads.\n3. Inspect these surfaces: codebase graph, giant modules/mixed concerns, duplicate logic, TODO/HACK/band-aids, WASM modules, IOA specs, Cedar policies, Rust orchestration, polling loops, dependencies, security drift, tests, proofs, dashboard breakage, and agent/human readability.\n4. Findings must be actionable and evidenced. Do not turn every heuristic hit into work; use judgment.\n5. Return one JSON object between {REPO_HEALTH_RESULT_JSON_BEGIN} and {REPO_HEALTH_RESULT_JSON_END}. The worker validates that JSON and writes RepoGraphSnapshot.ScanComplete through Temper.\n6. Do not print secrets.\n\nRequired JSON shape:\n{{\n  \"summary_markdown\": \"human-readable markdown with a Mermaid diagram when useful\",\n  \"evidence_scope\": [\n    {{\"surface\":\"codebase_graph\",\"query_or_command\":\"what you ran/read\",\"result_summary\":\"what you learned\"}},\n    {{\"surface\":\"wasm_modules\",\"query_or_command\":\"...\",\"result_summary\":\"...\"}},\n    {{\"surface\":\"specs_policies\",\"query_or_command\":\"...\",\"result_summary\":\"...\"}},\n    {{\"surface\":\"dependencies\",\"query_or_command\":\"...\",\"result_summary\":\"...\"}},\n    {{\"surface\":\"tests_proofs\",\"query_or_command\":\"...\",\"result_summary\":\"...\"}},\n    {{\"surface\":\"security_readability\",\"query_or_command\":\"...\",\"result_summary\":\"...\"}}\n  ],\n  \"quality_findings\": [\n    {{\"fingerprint\":\"quality:<stable-id>\",\"title\":\"...\",\"severity\":\"low|medium|high\",\"evidence\":\"cite paths and concrete reason\",\"affected_paths\":[\"path\"]}}\n  ],\n  \"security_findings\": [\n    {{\"fingerprint\":\"security:<stable-id>\",\"title\":\"...\",\"severity\":\"low|medium|high\",\"risk_lane\":\"L1|L2|L3\",\"evidence\":\"cite paths and concrete reason\",\"affected_paths\":[\"path\"]}}\n  ],\n  \"summary\": {{\n    \"scanned_files\": 0,\n    \"scanned_lines\": 0,\n    \"giant_modules\": 0,\n    \"todo_hack_hits\": 0,\n    \"duplicate_logic_candidates\": 0,\n    \"broad_cedar_policies\": 0,\n    \"dependency_risk_hits\": 0,\n    \"rust_orchestration_hits\": 0,\n    \"polling_loop_hits\": 0,\n    \"missing_test_coverage_hits\": 0\n  }},\n  \"residual_risks\": [\"...\"],\n  \"recommended_next_actions\": [\"...\"]\n}}\n\nIf there are no findings, return empty finding arrays and explain the evidence you checked.",
        worker_run.id,
        worker_run_branch_label(worker_run),
        if worker_run.task.is_empty() {
            "(no task text recorded)"
        } else {
            worker_run.task.as_str()
        }
    )
}

pub(crate) fn parse_repo_health_agent_output(output: &str) -> Result<RepoHealthAgentOutput> {
    let json_text = extract_repo_health_result_json(output)?;
    let raw: RepoHealthRawOutput =
        serde_json::from_str(json_text).context("parse repo health Codex JSON")?;
    raw.into_agent_output()
}

pub(crate) fn repo_sweep_summary_markdown(output: &RepoHealthAgentOutput) -> String {
    let graph = &output.graph;
    let evidence = if output.evidence_scope.is_empty() {
        "- No evidence scope recorded.".to_string()
    } else {
        output
            .evidence_scope
            .iter()
            .map(|scope| {
                format!(
                    "- {}: {}",
                    empty_fallback(&scope.surface, "unknown"),
                    empty_fallback(&scope.result_summary, "no summary")
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    let residual = if output.residual_risks.is_empty() {
        "None recorded.".to_string()
    } else {
        output.residual_risks.join("; ")
    };

    format!(
        "{}\n\n## Structured Counts\n\n- Quality findings: {}\n- Security findings: {}\n- Scanned files: {}\n- Scanned lines: {}\n- Giant modules: {}\n- Duplicate logic candidates: {}\n- TODO/HACK/band-aid hits: {}\n- Cedar policy risks: {}\n- Dependency risk hits: {}\n- Rust orchestration hits: {}\n- Polling loop hits: {}\n- Missing test/proof coverage hits: {}\n\n## Evidence Scope\n\n{}\n\n## Residual Risks\n\n{}",
        empty_fallback(
            &output.summary_markdown,
            "# Repo Health Patrol\n\nCodex completed an agent-led repo health scan."
        ),
        graph.quality_findings.len(),
        graph.security_findings.len(),
        graph.summary.scanned_files,
        graph.summary.scanned_lines,
        graph.summary.giant_modules,
        graph.summary.duplicate_logic_candidates,
        graph.summary.todo_hack_hits,
        graph.summary.broad_cedar_policies,
        graph.summary.dependency_risk_hits,
        graph.summary.rust_orchestration_hits,
        graph.summary.polling_loop_hits,
        graph.summary.missing_test_coverage_hits,
        evidence,
        residual
    )
}

fn extract_repo_health_result_json(output: &str) -> Result<&str> {
    let start = output
        .find(REPO_HEALTH_RESULT_JSON_BEGIN)
        .with_context(|| format!("missing {REPO_HEALTH_RESULT_JSON_BEGIN} marker"))?;
    let after_start = &output[start + REPO_HEALTH_RESULT_JSON_BEGIN.len()..];
    let end = after_start
        .find(REPO_HEALTH_RESULT_JSON_END)
        .with_context(|| format!("missing {REPO_HEALTH_RESULT_JSON_END} marker"))?;
    let json_text = after_start[..end].trim();
    if json_text.is_empty() {
        bail!("repo health Codex result JSON was empty");
    }
    Ok(json_text)
}

#[derive(Debug, Deserialize)]
struct RepoHealthRawOutput {
    #[serde(default)]
    summary_markdown: String,
    #[serde(default)]
    evidence_scope: Vec<RepoHealthEvidenceScope>,
    #[serde(default)]
    quality_findings: Vec<QualityFindingRecord>,
    #[serde(default)]
    security_findings: Vec<SecurityFindingRecord>,
    #[serde(default)]
    summary: RepoSweepSummary,
    #[serde(default)]
    residual_risks: Vec<String>,
    #[serde(default)]
    recommended_next_actions: Vec<String>,
}

impl RepoHealthRawOutput {
    fn into_agent_output(mut self) -> Result<RepoHealthAgentOutput> {
        require_repo_health_surfaces(&self.evidence_scope)?;
        for finding in &mut self.quality_findings {
            finding.normalize("quality");
        }
        for finding in &mut self.security_findings {
            finding.normalize("security");
        }

        Ok(RepoHealthAgentOutput {
            summary_markdown: self.summary_markdown.trim().to_string(),
            graph: RepoSweepGraph {
                quality_findings: self.quality_findings,
                security_findings: self.security_findings,
                summary: self.summary,
            },
            evidence_scope: self.evidence_scope,
            residual_risks: self.residual_risks,
            recommended_next_actions: self.recommended_next_actions,
        })
    }
}

impl QualityFindingRecord {
    fn normalize(&mut self, prefix: &str) {
        self.title = self.title.trim().to_string();
        self.severity = normalize_severity(&self.severity);
        self.evidence = self.evidence.trim().to_string();
        self.affected_paths = normalized_paths(&self.affected_paths);
        if self.fingerprint.trim().is_empty() {
            self.fingerprint = stable_fingerprint(prefix, &self.title, &self.affected_paths);
        }
    }
}

impl SecurityFindingRecord {
    fn normalize(&mut self, prefix: &str) {
        self.title = self.title.trim().to_string();
        self.severity = normalize_severity(&self.severity);
        self.risk_lane = normalize_risk_lane(&self.risk_lane);
        self.evidence = self.evidence.trim().to_string();
        self.affected_paths = normalized_paths(&self.affected_paths);
        if self.fingerprint.trim().is_empty() {
            self.fingerprint = stable_fingerprint(prefix, &self.title, &self.affected_paths);
        }
    }
}

fn require_repo_health_surfaces(scopes: &[RepoHealthEvidenceScope]) -> Result<()> {
    let required = [
        "codebase_graph",
        "wasm_modules",
        "specs_policies",
        "dependencies",
        "tests_proofs",
        "security_readability",
    ];
    let present = scopes
        .iter()
        .map(|scope| scope.surface.trim().to_ascii_lowercase())
        .collect::<Vec<_>>();
    let missing = required
        .iter()
        .filter(|surface| !present.iter().any(|value| value == **surface))
        .copied()
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        bail!(
            "repo health Codex output missing evidence surfaces: {}",
            missing.join(", ")
        );
    }
    Ok(())
}

fn normalize_severity(value: &str) -> String {
    match value.trim().to_ascii_lowercase().as_str() {
        "low" | "info" | "warn" | "warning" => "low".to_string(),
        "high" | "critical" | "error" => "high".to_string(),
        _ => "medium".to_string(),
    }
}

fn normalize_risk_lane(value: &str) -> String {
    match value.trim().to_ascii_uppercase().as_str() {
        "L1" | "L2" | "L3" => value.trim().to_ascii_uppercase(),
        _ => "L2".to_string(),
    }
}

fn normalized_paths(paths: &[String]) -> Vec<String> {
    let mut values = paths
        .iter()
        .map(|path| path.trim().trim_start_matches("./").to_string())
        .filter(|path| !path.is_empty())
        .collect::<Vec<_>>();
    values.sort();
    values.dedup();
    values
}

fn stable_fingerprint(kind: &str, title: &str, affected_paths: &[String]) -> String {
    let material = json!({
        "kind": kind,
        "title": title,
        "affected_paths": affected_paths,
    })
    .to_string();
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in material.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{kind}:{hash:016x}")
}

fn empty_fallback<'a>(value: &'a str, fallback: &'a str) -> &'a str {
    if value.trim().is_empty() {
        fallback
    } else {
        value
    }
}
