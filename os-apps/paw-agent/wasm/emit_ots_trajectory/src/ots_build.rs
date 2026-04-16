//! Pure functions that assemble OTS trajectory JSON from Session state.
//!
//! Kept separate from `lib.rs` so the mapping logic can be unit-tested without
//! a live Temper context. The resulting JSON shape matches the `OTSTrajectory`
//! serde schema in `temper/crates/temper-ots/src/models/trajectory.rs` as of
//! 2026-04-16 and is versioned at "0.1.0".

use serde_json::{Value, json};

/// Map the Session's terminal state + has_result flag to an OTS `OutcomeType`.
///
/// The OTS enum serializes as snake_case; returned strings match the on-wire
/// representation exactly.
pub fn derive_outcome(status: &str, has_result: bool) -> &'static str {
    match (status, has_result) {
        ("Completed", true) => "success",
        ("Completed", false) => "partial_success",
        ("Failed", _) => "failure",
        ("Cancelled", _) => "partial_success",
        _ => "failure",
    }
}

/// Classify a tool-call error message into an OTS `consequence.error_type`.
///
/// Categories match the conventions described in ADR-0035 decision section 4.
pub fn classify_error(error_text: &str) -> &'static str {
    let lowered = error_text.to_ascii_lowercase();
    if lowered.is_empty() {
        "unknown_error"
    } else if lowered.contains("timeout") || lowered.contains("timed out") {
        "tool_timeout"
    } else if lowered.contains("cedar") || lowered.contains("denied") {
        "cedar_denied"
    } else {
        "tool_error"
    }
}

/// Truncate a string to at most `max_chars` characters (not bytes). Keeps UTF-8 safe.
pub fn truncate_chars(s: &str, max_chars: usize) -> String {
    s.chars().take(max_chars).collect()
}

/// Convert a single tool-span JSON object (as emitted by `monty_repl::emit_tool_call_telemetry`)
/// into an `OTSDecision` JSON value.
///
/// Optional fields (state, alternatives, evaluation, credit_assignment, embedding) are
/// not populated — see ADR-0035 decision section 5. The `duration_ms` is preserved as
/// a non-standard `_duration_ms` field for evaluation-agent consumption.
pub fn span_to_decision(span: &Value) -> Value {
    let tool_call_id = span
        .get("tool_call_id")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let tool_name = span
        .get("tool_name")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let is_error = span
        .get("is_error")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let result = span.get("result").and_then(|v| v.as_str()).unwrap_or("");
    let duration_ms = span.get("duration_ms").and_then(|v| v.as_u64()).unwrap_or(0);

    let mut choice = json!({ "action": tool_name });
    match span.get("arguments") {
        Some(Value::String(s)) => {
            if let Ok(parsed) = serde_json::from_str::<Value>(s) {
                choice["arguments"] = parsed;
            } else {
                choice["arguments"] = Value::String(s.clone());
            }
        }
        Some(Value::Null) | None => {}
        Some(other) => {
            choice["arguments"] = other.clone();
        }
    }

    let truncated_result = truncate_chars(result, 500);
    let mut consequence = json!({
        "success": !is_error,
        "result_summary": truncated_result,
    });
    if is_error {
        consequence["error_type"] = json!(classify_error(result));
    }

    json!({
        "decision_id": tool_call_id,
        "decision_type": "tool_selection",
        "choice": choice,
        "consequence": consequence,
        "_duration_ms": duration_ms,
    })
}

/// Parse a tool-span JSONL document into a vector of `OTSDecision` JSON values.
///
/// Invalid lines are skipped silently — tool-span persistence is best-effort and
/// the emitter must not fail on a corrupted span.
pub fn parse_tool_spans_to_decisions(tool_spans_jsonl: &str) -> Vec<Value> {
    tool_spans_jsonl
        .lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .map(|span| span_to_decision(&span))
        .collect()
}

/// Assemble a complete `OTSTrajectory` JSON document.
///
/// Decisions are collapsed into a single synthetic `OTSTurn` for MVP. Precise
/// turn-boundary reconstruction from the session tree is deferred to a follow-up
/// track — see ADR-0035 risks section 1.
pub fn build_trajectory(
    trajectory_id: &str,
    session_id: &str,
    agent_id: &str,
    status: &str,
    fields: &Value,
    tool_spans_jsonl: &str,
) -> Value {
    let has_result = fields
        .get("has_result")
        .and_then(|v| v.as_bool())
        .or_else(|| {
            fields
                .get("has_result")
                .and_then(|v| v.as_str())
                .map(|s| s == "true")
        })
        .unwrap_or(false);
    let outcome = derive_outcome(status, has_result);

    let task_description = truncate_chars(
        fields
            .get("user_message")
            .and_then(|v| v.as_str())
            .unwrap_or(""),
        500,
    );
    let model = fields
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let provider = fields
        .get("provider")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let session_mode = fields
        .get("session_mode")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let mut tags: Vec<String> = Vec::new();
    if !model.is_empty() {
        tags.push(model);
    }
    if !provider.is_empty() {
        tags.push(provider);
    }
    if !session_mode.is_empty() {
        tags.push(session_mode);
    }

    let decisions = parse_tool_spans_to_decisions(tool_spans_jsonl);

    let turn = json!({
        "turn_id": format!("{session_id}-turn"),
        "decisions": decisions,
    });

    let metadata = json!({
        "task_description": task_description,
        "domain": "openpaw-agent",
        "agent_id": agent_id,
        "framework": "openpaw",
        "environment": "production",
        "outcome": outcome,
        "tags": tags,
    });

    let mut trajectory = json!({
        "trajectory_id": trajectory_id,
        "version": "0.1.0",
        "metadata": metadata,
        "turns": [turn],
    });

    let system_prompt = fields
        .get("system_prompt")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if !system_prompt.is_empty() {
        trajectory["system_message"] = json!({
            "role": "system",
            "content": truncate_chars(system_prompt, 2000),
            "timestamp": "",
        });
    }

    trajectory
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derive_outcome_matrix() {
        assert_eq!(derive_outcome("Completed", true), "success");
        assert_eq!(derive_outcome("Completed", false), "partial_success");
        assert_eq!(derive_outcome("Failed", true), "failure");
        assert_eq!(derive_outcome("Failed", false), "failure");
        assert_eq!(derive_outcome("Cancelled", true), "partial_success");
        assert_eq!(derive_outcome("Cancelled", false), "partial_success");
        assert_eq!(derive_outcome("Thinking", true), "failure"); // defensive
    }

    #[test]
    fn classify_error_categories() {
        assert_eq!(classify_error(""), "unknown_error");
        assert_eq!(classify_error("request timed out after 30s"), "tool_timeout");
        assert_eq!(classify_error("Cedar denied action"), "cedar_denied");
        assert_eq!(
            classify_error("permission denied by policy"),
            "cedar_denied"
        );
        assert_eq!(classify_error("subprocess failed"), "tool_error");
    }

    #[test]
    fn truncate_chars_respects_utf8() {
        let s = "héllo wörld";
        assert_eq!(truncate_chars(s, 5), "héllo");
        assert_eq!(truncate_chars(s, 100), s);
    }

    #[test]
    fn span_to_decision_success_shape() {
        let span = json!({
            "tool_name": "temper.read",
            "tool_call_id": "tc-123",
            "arguments": "{\"path\":\"/tmp/foo\"}",
            "result": "ok",
            "duration_ms": 42,
            "is_error": false,
        });
        let dec = span_to_decision(&span);
        assert_eq!(dec["decision_id"], "tc-123");
        assert_eq!(dec["decision_type"], "tool_selection");
        assert_eq!(dec["choice"]["action"], "temper.read");
        assert_eq!(dec["choice"]["arguments"]["path"], "/tmp/foo");
        assert_eq!(dec["consequence"]["success"], true);
        assert_eq!(dec["consequence"]["result_summary"], "ok");
        assert!(dec["consequence"].get("error_type").is_none());
        assert_eq!(dec["_duration_ms"], 42);
    }

    #[test]
    fn span_to_decision_error_classifies() {
        let span = json!({
            "tool_name": "temper.bash",
            "tool_call_id": "tc-err",
            "arguments": "{}",
            "result": "Cedar denied bash execution",
            "duration_ms": 5,
            "is_error": true,
        });
        let dec = span_to_decision(&span);
        assert_eq!(dec["consequence"]["success"], false);
        assert_eq!(dec["consequence"]["error_type"], "cedar_denied");
    }

    #[test]
    fn span_to_decision_keeps_non_json_arguments_as_string() {
        let span = json!({
            "tool_name": "raw",
            "tool_call_id": "tc-raw",
            "arguments": "not-valid-json",
            "result": "",
            "duration_ms": 0,
            "is_error": false,
        });
        let dec = span_to_decision(&span);
        assert_eq!(dec["choice"]["arguments"], "not-valid-json");
    }

    #[test]
    fn span_to_decision_truncates_long_result() {
        let long_result = "x".repeat(1000);
        let span = json!({
            "tool_name": "temper.bash",
            "tool_call_id": "tc-long",
            "arguments": "{}",
            "result": long_result,
            "duration_ms": 1,
            "is_error": false,
        });
        let dec = span_to_decision(&span);
        let summary = dec["consequence"]["result_summary"].as_str().unwrap();
        assert_eq!(summary.len(), 500);
    }

    #[test]
    fn parse_tool_spans_skips_invalid_lines() {
        let jsonl = "{\"tool_call_id\":\"a\",\"tool_name\":\"x\",\"result\":\"\",\"duration_ms\":0,\"is_error\":false}\nINVALID\n{\"tool_call_id\":\"b\",\"tool_name\":\"y\",\"result\":\"\",\"duration_ms\":0,\"is_error\":false}\n";
        let decisions = parse_tool_spans_to_decisions(jsonl);
        assert_eq!(decisions.len(), 2);
        assert_eq!(decisions[0]["decision_id"], "a");
        assert_eq!(decisions[1]["decision_id"], "b");
    }

    #[test]
    fn build_trajectory_minimum_shape() {
        let fields = json!({
            "user_message": "please find the bug",
            "model": "claude-sonnet-4-6",
            "provider": "anthropic",
            "has_result": true,
            "session_mode": "execute",
        });
        let jsonl = "{\"tool_call_id\":\"a\",\"tool_name\":\"read\",\"arguments\":\"{}\",\"result\":\"ok\",\"duration_ms\":10,\"is_error\":false}\n";
        let t = build_trajectory(
            "trj-42",
            "ss-77",
            "aj-99",
            "Completed",
            &fields,
            jsonl,
        );
        assert_eq!(t["trajectory_id"], "trj-42");
        assert_eq!(t["version"], "0.1.0");
        assert_eq!(t["metadata"]["outcome"], "success");
        assert_eq!(t["metadata"]["agent_id"], "aj-99");
        assert_eq!(t["metadata"]["domain"], "openpaw-agent");
        assert_eq!(t["metadata"]["framework"], "openpaw");
        assert_eq!(t["metadata"]["task_description"], "please find the bug");
        let tags = t["metadata"]["tags"].as_array().unwrap();
        assert!(tags.iter().any(|v| v == "claude-sonnet-4-6"));
        assert!(tags.iter().any(|v| v == "anthropic"));
        assert!(tags.iter().any(|v| v == "execute"));
        let turns = t["turns"].as_array().unwrap();
        assert_eq!(turns.len(), 1);
        assert_eq!(turns[0]["turn_id"], "ss-77-turn");
        assert_eq!(turns[0]["decisions"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn build_trajectory_no_spans_emits_empty_decisions() {
        let fields = json!({ "user_message": "", "has_result": false });
        let t = build_trajectory("trj-0", "ss-0", "aj-0", "Failed", &fields, "");
        assert_eq!(t["metadata"]["outcome"], "failure");
        assert_eq!(
            t["turns"][0]["decisions"].as_array().unwrap().len(),
            0
        );
    }

    #[test]
    fn build_trajectory_cancelled_is_partial_success() {
        let fields = json!({ "user_message": "", "has_result": false });
        let t = build_trajectory("trj-c", "ss-c", "aj-c", "Cancelled", &fields, "");
        assert_eq!(t["metadata"]["outcome"], "partial_success");
    }

    #[test]
    fn build_trajectory_includes_system_message_when_present() {
        let fields = json!({ "user_message": "", "system_prompt": "you are a helpful agent" });
        let t = build_trajectory("trj-s", "ss-s", "aj-s", "Completed", &fields, "");
        assert_eq!(t["system_message"]["role"], "system");
        assert_eq!(t["system_message"]["content"], "you are a helpful agent");
    }

    #[test]
    fn build_trajectory_skips_system_message_when_empty() {
        let fields = json!({ "user_message": "", "system_prompt": "" });
        let t = build_trajectory("trj-s2", "ss-s2", "aj-s2", "Completed", &fields, "");
        assert!(t.get("system_message").is_none());
    }
}
