#[test]
fn datadog_patrol_worker_is_mcp_agent_driven_not_rust_datadog_collector() {
    let source = include_str!("datadog_patrol.rs");

    for forbidden in [
        "/api/v1/monitor/search",
        "DD-API-KEY",
        "DD-APPLICATION-KEY",
        "query_datadog_monitor_search",
        "active_datadog_monitors",
        "\"Signals\"",
        "\"ObservabilityFindings\"",
        "\"FactoryCases\"",
    ] {
        assert!(
            !source.contains(forbidden),
            "Datadog Patrol worker should run the MCP agent and report evidence, not collect or fan out Patrol state directly in Rust: {forbidden}"
        );
    }

    for required in [
        "DATADOG_PATROL_RESULT_JSON_BEGIN",
        "DATADOG_PATROL_RESULT_JSON_END",
        "Datadog MCP",
        "max_tokens <= 12000",
        "summarize the evidence",
        "monitors",
        "logs",
        "traces",
        "metrics",
        "incidents",
        "dashboards",
    ] {
        assert!(
            source.contains(required),
            "Datadog Patrol should make the MCP investigation contract explicit: {required}"
        );
    }
}

#[test]
fn datadog_patrol_result_parser_extracts_agent_mcp_findings() {
    let output = r#"
Codex investigated Datadog using MCP.
DATADOG_PATROL_RESULT_JSON_BEGIN
{
  "summary": "One active Discord-facing error pattern needs follow-up.",
  "evidence_scope": [
    {"surface":"monitors","query":"searched OpenPaw and Temper monitor states"},
    {"surface":"logs","query":"searched recent production errors"},
    {"surface":"traces","query":"checked APM traces for Discord request failures"},
    {"surface":"metrics","query":"checked error-rate and latency metrics"},
    {"surface":"incidents","query":"checked open incidents"},
    {"surface":"dashboards","query":"reviewed OpenPaw runtime dashboards"}
  ],
  "findings": [
    {
      "title": "Discord DM replies are surfacing raw traces",
      "severity": "high",
      "risk_lane": "L2",
      "source_url": "https://app.datadoghq.com/logs?query=discord",
      "datadog_monitor_id": "",
      "fingerprint": "datadog:mcp:discord-trace-leak",
      "affected_services": ["openpaw-production"],
      "evidence_json": {"surface":"logs","sample_count":3},
      "work_summary": "Stop raw trace leakage in Discord DM replies",
      "work_detail": "Reproduce from Datadog log evidence, add regression coverage, and verify Discord-facing output is sanitized.",
      "requires_human_approval": true
    }
  ],
  "residual_risks": ["Datadog MCP queries are sampled evidence."],
  "recommended_next_queries": ["Inspect latest Discord transport traces after the fix."]
}
DATADOG_PATROL_RESULT_JSON_END
"#;

    let investigation =
        parse_datadog_patrol_investigation_output(output).expect("parse MCP patrol output");

    assert_eq!(
        investigation.summary,
        "One active Discord-facing error pattern needs follow-up."
    );
    assert_eq!(investigation.evidence_scope.len(), 6);
    assert_eq!(investigation.findings.len(), 1);
    assert_eq!(
        investigation.findings[0].fingerprint,
        "datadog:mcp:discord-trace-leak"
    );
    assert!(investigation.findings[0].requires_human_approval);
}

#[test]
fn datadog_patrol_classifier_ignores_followup_and_rework_prompts() {
    let patrol_task = "You are the local Codex Datadog Patrol agent for TemperPaw paw-patrol.\n\nPatrolRun: en-patrol\nPatrolKind: datadog_observability";
    let implementer_task = "You are the local Codex implementer for a Paw Patrol Datadog MCP observability finding.\n\nPatrolRun: en-patrol\nPatrol kind: datadog_observability\nFinding: OpenPaw monitor coverage is degraded by No Data states";
    let rework_task = "You are the local Codex implementer for reviewer-requested rework.\n\nFactoryCase: \nWorkCycle: wc-patrol\nSummary: Risk Patrol\n\nOriginal task:\nYou are the local Codex Datadog Patrol agent for TemperPaw paw-patrol.\n\nPatrolRun: en-patrol\nPatrolKind: datadog_observability";

    assert_eq!(
        extract_datadog_patrol_run_id(patrol_task).as_deref(),
        Some("en-patrol")
    );
    assert_eq!(
        extract_datadog_patrol_run_id(implementer_task),
        None,
        "Datadog follow-up implementation must not use the patrol writeback collector"
    );
    assert_eq!(
        extract_datadog_patrol_run_id(rework_task),
        None,
        "reviewer-requested rework embeds the original patrol task but must run as normal Codex implementation"
    );
}

#[test]
fn datadog_patrol_risk_lanes_control_start_approval() {
    let mut low = DatadogPatrolFinding {
        title: "Minor dashboard freshness drift".to_string(),
        severity: "warn".to_string(),
        risk_lane: "L1".to_string(),
        source_url: String::new(),
        datadog_monitor_id: String::new(),
        fingerprint: String::new(),
        affected_services: Vec::new(),
        evidence_json: json!({ "surface": "dashboards" }),
        work_summary: String::new(),
        work_detail: String::new(),
        requires_human_approval: false,
    };
    low.normalize();

    let mut high = DatadogPatrolFinding {
        title: "Discord users receive raw traces".to_string(),
        severity: "error".to_string(),
        risk_lane: "L2".to_string(),
        source_url: String::new(),
        datadog_monitor_id: String::new(),
        fingerprint: String::new(),
        affected_services: Vec::new(),
        evidence_json: json!({ "surface": "logs" }),
        work_summary: String::new(),
        work_detail: String::new(),
        requires_human_approval: false,
    };
    high.normalize();

    assert!(!low.requires_start_approval());
    assert!(high.requires_start_approval());
    assert_eq!(
        datadog_followup_branch_name(&low, "en-019e00bc-1234"),
        "codex/paw-datadog-minor-dashboard-freshness-drift-en-019e0"
    );
}
