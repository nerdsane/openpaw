struct DatadogProofSummaryInput<'a> {
    patrol_run_id: &'a str,
    worker_run_id: &'a str,
    investigation: &'a DatadogPatrolInvestigation,
    signal_ids: &'a [String],
    finding_ids: &'a [String],
    case_ids: &'a [String],
    work_cycle_ids: &'a [String],
    implementer_worker_run_ids: &'a [String],
}

fn datadog_proof_summary_markdown(input: &DatadogProofSummaryInput<'_>) -> String {
    let surfaces = input
        .investigation
        .evidence_scope
        .iter()
        .map(|scope| {
            format!(
                "- {}: {}",
                scope.surface,
                truncate_middle(&scope.result_summary, 300)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let findings = input
        .investigation
        .findings
        .iter()
        .map(|finding| {
            format!(
                "- {} [{} / {}] -> {}",
                finding.title, finding.severity, finding.risk_lane, finding.work_summary
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let findings = if findings.trim().is_empty() {
        "- No actionable findings opened.".to_string()
    } else {
        findings
    };

    format!(
        "# Datadog MCP Patrol Proof\n\nPatrolRun `{patrol_run_id}` was executed by WorkerRun `{worker_run_id}` using the local Codex agent and its authenticated Datadog MCP tools.\n\n```mermaid\n{}\n```\n\n## Result\n\n{}\n\n## Evidence Scope\n\n{}\n\n## Findings\n\n{}\n\n## Created Temper Entities\n\n- Signals: {}\n- ObservabilityFindings: {}\n- FactoryCases: {}\n- WorkCycles: {}\n- Low-risk implementer WorkerRuns queued: {}\n\n## Gate Posture\n\nThe patrol does not mutate code or production. Actionable findings become WorkCycles; high-risk or production-impacting work pauses before implementation.",
        datadog_state_diagram_mermaid(),
        input.investigation.summary.trim(),
        surfaces,
        findings,
        input.signal_ids.len(),
        input.finding_ids.len(),
        input.case_ids.len(),
        input.work_cycle_ids.len(),
        input.implementer_worker_run_ids.len(),
        patrol_run_id = input.patrol_run_id,
        worker_run_id = input.worker_run_id,
    )
}

fn datadog_state_diagram_mermaid() -> &'static str {
    "flowchart LR\n  Run[\"PatrolRun datadog_observability\"] --> Worker[\"mac-mini-codex-prod WorkerRun\"]\n  Worker --> Codex[\"Codex agent\"]\n  Codex --> MCP[\"Datadog MCP investigation\"]\n  MCP --> Scope[\"monitors logs traces metrics incidents dashboards\"]\n  Scope --> Signals[\"Signals\"]\n  Scope --> Findings[\"ObservabilityFindings\"]\n  Findings --> Cases[\"FactoryCases\"]\n  Cases --> Work[\"Risk-gated WorkCycles\"]\n  Worker --> Proof[\"Visual ProofPacket\"]\n  Proof --> Complete[\"PatrolRun Complete\"]"
}

fn datadog_visual_summary_url(
    evidence_surface_count: usize,
    finding_count: usize,
    work_cycle_count: usize,
) -> String {
    let svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"960\" height=\"540\" viewBox=\"0 0 960 540\" role=\"img\" aria-labelledby=\"title desc\"><title id=\"title\">Datadog MCP Patrol proof</title><desc id=\"desc\">Factual proof summary generated from a Codex Datadog MCP investigation.</desc><rect width=\"960\" height=\"540\" fill=\"#f7f5ef\"/><rect x=\"40\" y=\"36\" width=\"880\" height=\"468\" rx=\"8\" fill=\"#ffffff\" stroke=\"#d5d2c6\"/><text x=\"70\" y=\"92\" font-family=\"ui-sans-serif, system-ui\" font-size=\"32\" font-weight=\"700\" fill=\"#202124\">Datadog MCP Patrol Proof</text><text x=\"70\" y=\"130\" font-family=\"ui-sans-serif, system-ui\" font-size=\"16\" fill=\"#64615a\">Codex investigated Datadog via MCP, then the worker wrote structured state to Temper.</text><rect x=\"70\" y=\"172\" width=\"230\" height=\"126\" rx=\"8\" fill=\"#fff4e5\" stroke=\"#f4c27a\"/><text x=\"96\" y=\"218\" font-family=\"ui-sans-serif, system-ui\" font-size=\"15\" fill=\"#64615a\">Evidence surfaces</text><text x=\"96\" y=\"266\" font-family=\"ui-sans-serif, system-ui\" font-size=\"42\" font-weight=\"700\" fill=\"#a15c00\">{evidence_surface_count}</text><rect x=\"330\" y=\"172\" width=\"230\" height=\"126\" rx=\"8\" fill=\"#eef5ff\" stroke=\"#b8c7dc\"/><text x=\"356\" y=\"218\" font-family=\"ui-sans-serif, system-ui\" font-size=\"15\" fill=\"#64615a\">Findings opened</text><text x=\"356\" y=\"266\" font-family=\"ui-sans-serif, system-ui\" font-size=\"42\" font-weight=\"700\" fill=\"#174ea6\">{finding_count}</text><rect x=\"590\" y=\"172\" width=\"230\" height=\"126\" rx=\"8\" fill=\"#edf4ee\" stroke=\"#b7d3bc\"/><text x=\"616\" y=\"218\" font-family=\"ui-sans-serif, system-ui\" font-size=\"15\" fill=\"#64615a\">WorkCycles gated</text><text x=\"616\" y=\"266\" font-family=\"ui-sans-serif, system-ui\" font-size=\"42\" font-weight=\"700\" fill=\"#137333\">{work_cycle_count}</text><text x=\"70\" y=\"360\" font-family=\"ui-sans-serif, system-ui\" font-size=\"18\" font-weight=\"700\" fill=\"#202124\">Flow</text><text x=\"70\" y=\"394\" font-family=\"ui-sans-serif, system-ui\" font-size=\"16\" fill=\"#202124\">PatrolRun -> Codex -> Datadog MCP -> Signals -> Findings -> Cases -> WorkCycles -> ProofPacket.</text><text x=\"70\" y=\"450\" font-family=\"ui-sans-serif, system-ui\" font-size=\"14\" fill=\"#64615a\">Production-impacting fixes remain blocked until the risk lane allows them to proceed.</text></svg>"
    );
    format!("data:image/svg+xml,{}", percent_encode_data_url(&svg))
}
