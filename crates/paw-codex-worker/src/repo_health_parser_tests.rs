#[test]
fn repo_sweep_task_is_detected_from_worker_prompt() {
    let task = "RepoGraphSnapshot: en-123\nWorkCycle: wc-456\nRequired loop:";

    assert_eq!(
        extract_repo_sweep_snapshot_id(task).as_deref(),
        Some("en-123")
    );
}

#[test]
fn repo_health_patrol_parser_requires_agent_evidence_surfaces() {
    let output = r##"
REPO_HEALTH_PATROL_RESULT_JSON_BEGIN
{
  "summary_markdown": "# Agent-led repo health",
  "evidence_scope": [
    {"surface":"codebase_graph","query_or_command":"rg --files","result_summary":"graph inspected"},
    {"surface":"wasm_modules","query_or_command":"rg os-apps","result_summary":"wasm inspected"},
    {"surface":"specs_policies","query_or_command":"rg cedar ioa","result_summary":"specs inspected"},
    {"surface":"dependencies","query_or_command":"cargo metadata","result_summary":"dependencies inspected"},
    {"surface":"tests_proofs","query_or_command":"cargo test --no-run","result_summary":"tests inspected"},
    {"surface":"security_readability","query_or_command":"rg TODO HACK","result_summary":"readability inspected"}
  ],
  "quality_findings": [
    {
      "title": "Mixed-concern WASM module",
      "severity": "warn",
      "evidence": "os-apps/paw-agent/wasm/monty_repl/src/lib.rs mixes REPL, parsing, and orchestration.",
      "affected_paths": ["./os-apps/paw-agent/wasm/monty_repl/src/lib.rs"]
    }
  ],
  "security_findings": [
    {
      "title": "Broad Cedar policy needs review",
      "severity": "critical",
      "risk_lane": "l3",
      "evidence": "policy permits a broad shape.",
      "affected_paths": ["os-apps/demo/policies/demo.cedar"]
    }
  ],
  "summary": {
    "scanned_files": 120,
    "scanned_lines": 44000,
    "giant_modules": 1,
    "todo_hack_hits": 4,
    "duplicate_logic_candidates": 2,
    "broad_cedar_policies": 1,
    "dependency_risk_hits": 0,
    "rust_orchestration_hits": 1,
    "polling_loop_hits": 1,
    "missing_test_coverage_hits": 3
  },
  "residual_risks": ["human should approve L3"],
  "recommended_next_actions": ["split Monty REPL"]
}
REPO_HEALTH_PATROL_RESULT_JSON_END
"##;

    let parsed = parse_repo_health_agent_output(output).expect("parse agent output");

    assert_eq!(parsed.graph.quality_findings.len(), 1);
    assert_eq!(parsed.graph.security_findings.len(), 1);
    assert_eq!(parsed.graph.quality_findings[0].severity, "low");
    assert!(
        parsed.graph.quality_findings[0]
            .fingerprint
            .starts_with("quality:")
    );
    assert_eq!(parsed.graph.security_findings[0].severity, "high");
    assert_eq!(parsed.graph.security_findings[0].risk_lane, "L3");
    assert_eq!(
        parsed.graph.quality_findings[0].affected_paths,
        vec!["os-apps/paw-agent/wasm/monty_repl/src/lib.rs"]
    );
}
