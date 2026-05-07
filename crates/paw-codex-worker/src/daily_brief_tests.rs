#[cfg(test)]
mod daily_brief_tests {
    use super::*;

    #[test]
    fn daily_brief_task_is_detected_from_worker_prompt() {
        let task =
            "You are the local Codex DailyBrief agent.\n\nDailyBrief: db-123\nWorkCycle: wc-456";

        assert_eq!(extract_daily_brief_id(task).as_deref(), Some("db-123"));
    }

    #[test]
    fn daily_brief_parser_requires_agent_render_packet() {
        let output = r##"
DAILY_BRIEF_RESULT_JSON_BEGIN
{
  "summary_markdown": "# Patrol daily brief\n\n```mermaid\nflowchart LR\n  A[Proof] --> B[Risk]\n```\n\nOpen risks are linked.",
  "visual_summary_url": "data:image/svg+xml,%3Csvg%3E%3C/svg%3E",
  "proof_packet_ids": ["pp-1"],
  "open_risks": [{"type":"QualityFinding","id":"qf-1","title":"Mixed concerns"}],
  "done_items": [{"type":"WorkCycle","id":"wc-1","summary":"Finished one loop"}],
  "residual_risks": ["Datadog evidence was sampled."]
}
DAILY_BRIEF_RESULT_JSON_END
"##;

        let parsed = parse_daily_brief_agent_output(output).expect("parse daily brief output");

        assert!(parsed.summary_markdown.contains("Patrol daily brief"));
        assert_eq!(parsed.proof_packet_ids, "[\"pp-1\"]");
        assert!(parsed.open_risks.contains("QualityFinding"));
        assert!(parsed.done_items.contains("WorkCycle"));
        assert!(parsed.residual_risks.contains("sampled"));
    }
}
