//! Heal Reporter — WASM module for terminal AlertCycle actions (report_outcome).
//!
//! Triggered by AlertResolved, TuneComplete, Escalate, and AlertPersists.
//! Builds a summary of the AlertCycle outcome and dispatches a
//! Channels.Paw.Channel.SendReply to report the result in the originating thread.
//!
//! Build: `cargo build --target wasm32-unknown-unknown --release`

use temper_wasm_sdk::prelude::*;

/// Entry point.
#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        ctx.log("info", "heal_reporter: starting");

        let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));

        // Read report target from entity state
        let report_channel_entity_id = fields
            .get("report_channel_entity_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let report_thread_id = fields
            .get("report_thread_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if report_channel_entity_id.is_empty() {
            ctx.log(
                "info",
                "heal_reporter: no report_channel_entity_id, skipping report",
            );
            set_success_result("", &json!({ "status": "skipped" }));
            return Ok(());
        }

        let entity_id = ctx
            .entity_state
            .get("entity_id")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let entity_status = ctx
            .entity_state
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown");

        // Read AlertCycle context fields
        let diagnosis = fields
            .get("diagnosis")
            .and_then(|v| v.as_str())
            .unwrap_or("No diagnosis recorded yet.");
        let pr_url = fields
            .get("pr_url")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let sre_agent_id = fields
            .get("sre_agent_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let developer_agent_id = fields
            .get("developer_agent_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let monitor_id = fields
            .get("monitor_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let merge_sha = fields
            .get("merge_sha")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let deployment_url = fields
            .get("deployment_url")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let tuning_notes = fields
            .get("tuning_notes")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let temper_api_url = ctx
            .config
            .get("temper_api_url")
            .filter(|s| !s.is_empty() && !s.contains("{secret:"))
            .cloned()
            .unwrap_or_else(|| "http://127.0.0.1:3000".to_string());
        let tenant = &ctx.tenant;
        let api_headers = vec![
            ("content-type".to_string(), "application/json".to_string()),
            ("x-tenant-id".to_string(), tenant.to_string()),
            ("x-temper-principal-kind".to_string(), "agent".to_string()),
            ("x-temper-principal-id".to_string(), ctx.entity_id.clone()),
            ("x-temper-agent-type".to_string(), "system".to_string()),
        ];

        // Try to fetch related WorkCycle and Issue for richer context
        let mut work_cycle_line = "WorkCycle: n/a".to_string();
        let mut issue_line = "Issue: n/a".to_string();

        // Query WorkCycles related to this AlertCycle's Harness
        // (best-effort; non-fatal if it fails)
        if !sre_agent_id.is_empty() {
            let wc_url = format!(
                "{temper_api_url}/tdata/WorkCycles?$orderby=sequence_nr desc&$top=1"
            );
            if let Ok(wc_resp) = ctx.http_call("GET", &wc_url, &api_headers, "") {
                if wc_resp.status >= 200 && wc_resp.status < 300 {
                    let wc_body: Value =
                        serde_json::from_str(&wc_resp.body).unwrap_or(json!({}));
                    if let Some(items) = wc_body.get("value").and_then(|v| v.as_array()) {
                        if let Some(wc) = items.first() {
                            let wc_id = wc
                                .get("entity_id")
                                .and_then(|v| v.as_str())
                                .unwrap_or("unknown");
                            let wc_status = wc
                                .get("status")
                                .and_then(|v| v.as_str())
                                .unwrap_or("Unknown");
                            work_cycle_line = format!("WorkCycle: {wc_id} ({wc_status})");
                        }
                    }
                }
            }

            // Query Issues that mention this AlertCycle
            let issues_url = format!(
                "{temper_api_url}/tdata/Issues?$orderby=sequence_nr desc&$top=25"
            );
            if let Ok(issues_resp) = ctx.http_call("GET", &issues_url, &api_headers, "") {
                if issues_resp.status >= 200 && issues_resp.status < 300 {
                    let issues_body: Value =
                        serde_json::from_str(&issues_resp.body).unwrap_or(json!({}));
                    if let Some(items) = issues_body.get("value").and_then(|v| v.as_array()) {
                        for issue in items {
                            let desc = issue
                                .get("fields")
                                .and_then(|f| f.get("description"))
                                .and_then(|v| v.as_str())
                                .or_else(|| {
                                    issue.get("description").and_then(|v| v.as_str())
                                })
                                .unwrap_or("");
                            if desc.contains(entity_id) {
                                let issue_id = issue
                                    .get("entity_id")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("unknown");
                                issue_line = format!("Issue: {issue_id}");
                                break;
                            }
                        }
                    }
                }
            }
        }

        // Build summary message
        let mut lines = vec![
            "Open Paw self-heal update".to_string(),
            format!("AlertCycle: {entity_id} ({entity_status})"),
        ];
        if !sre_agent_id.is_empty() {
            lines.push(format!("SRE: {sre_agent_id}"));
        }
        if !developer_agent_id.is_empty() {
            lines.push(format!("Developer: {developer_agent_id}"));
        }
        lines.push(work_cycle_line);
        lines.push(issue_line);
        if !monitor_id.is_empty() {
            lines.push(format!("Monitor: {monitor_id}"));
        }
        lines.push(format!("Diagnosis: {diagnosis}"));
        if !pr_url.is_empty() {
            lines.push(format!("PR: {pr_url}"));
        }
        if !merge_sha.is_empty() {
            lines.push(format!("Merge SHA: {merge_sha}"));
        }
        if !deployment_url.is_empty() {
            lines.push(format!("Deployment: {deployment_url}"));
        }
        if !tuning_notes.is_empty() {
            lines.push(format!("Tuning notes: {tuning_notes}"));
        }

        let content = lines.join("\n");

        // Dispatch Channels.Paw.Channel.SendReply
        let send_url = format!(
            "{temper_api_url}/tdata/Channels('{report_channel_entity_id}')/Paw.Channel.SendReply"
        );
        let send_body = json!({
            "thread_id": report_thread_id,
            "content": content,
            "agent_entity_id": sre_agent_id,
        });
        let send_resp = ctx.http_call("POST", &send_url, &api_headers, &send_body.to_string())?;
        if send_resp.status < 200 || send_resp.status >= 300 {
            ctx.log(
                "warn",
                &format!(
                    "heal_reporter: SendReply failed (HTTP {}): {}",
                    send_resp.status,
                    &send_resp.body[..send_resp.body.len().min(300)]
                ),
            );
        } else {
            ctx.log("info", "heal_reporter: report sent successfully");
        }

        // Terminal state — no callback action needed
        set_success_result("", &json!({ "status": "reported" }));

        Ok(())
    })();

    if let Err(e) = result {
        set_error_result(&e);
    }
    0
}
