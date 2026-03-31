//! Alert Opener — WASM module for the AlertCycle.Open (spawn_sre) integration.
//!
//! Spawns an SRE agent to triage and remediate a monitor alert. Queries the
//! Monitor and ProjectHarness entities for context, creates a new Agent, and
//! dispatches Configure + Provision actions on it.
//!
//! Build: `cargo build --target wasm32-unknown-unknown --release`

use temper_wasm_sdk::prelude::*;

/// Entry point.
#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        ctx.log("info", "alert_opener: starting");

        let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));

        // 1. Read trigger params
        let alert_payload = ctx
            .trigger_params
            .get("alert_payload")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let monitor_id = ctx
            .trigger_params
            .get("monitor_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if monitor_id.is_empty() {
            return Err("alert_opener: monitor_id is required".to_string());
        }

        let entity_id = ctx
            .entity_state
            .get("entity_id")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        // 2. Read integration config
        let temper_api_url = ctx
            .config
            .get("temper_api_url")
            .filter(|s| !s.is_empty() && !s.contains("{secret:"))
            .cloned()
            .unwrap_or_else(|| "http://127.0.0.1:3000".to_string());
        let default_agent_model = ctx
            .config
            .get("default_agent_model")
            .cloned()
            .unwrap_or_else(|| "claude-sonnet-4-20250514".to_string());
        let default_sre_workdir = ctx
            .config
            .get("default_sre_workdir")
            .cloned()
            .unwrap_or_else(|| "/tmp/openpaw-sre-webhook".to_string());
        let default_developer_workdir = ctx
            .config
            .get("default_developer_workdir")
            .cloned()
            .unwrap_or_else(|| "/tmp/openpaw-self-heal".to_string());

        let tenant = &ctx.tenant;
        let headers = vec![
            ("content-type".to_string(), "application/json".to_string()),
            ("x-tenant-id".to_string(), tenant.to_string()),
            ("x-temper-principal-kind".to_string(), "agent".to_string()),
            ("x-temper-principal-id".to_string(), ctx.entity_id.clone()),
            ("x-temper-agent-type".to_string(), "system".to_string()),
        ];

        // 3. Query Monitor entity
        let monitor_url = format!("{temper_api_url}/tdata/Monitors('{monitor_id}')");
        let monitor_resp = ctx.http_call("GET", &monitor_url, &headers, "")?;
        let monitor: Value = if monitor_resp.status >= 200 && monitor_resp.status < 300 {
            serde_json::from_str(&monitor_resp.body).unwrap_or(json!({}))
        } else {
            ctx.log(
                "warn",
                &format!(
                    "alert_opener: failed to fetch Monitor {monitor_id} (HTTP {})",
                    monitor_resp.status
                ),
            );
            json!({})
        };

        let monitor_name = monitor
            .get("fields")
            .and_then(|f| f.get("name"))
            .and_then(|v| v.as_str())
            .or_else(|| monitor.get("name").and_then(|v| v.as_str()))
            .unwrap_or("unknown-monitor");
        let dd_monitor_id = monitor
            .get("fields")
            .and_then(|f| f.get("dd_monitor_id"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // 4. Query ProjectHarness entities looking for one matching the alert repo
        let alert_json: Value =
            serde_json::from_str(alert_payload).unwrap_or_else(|_| json!({}));
        let alert_repo = alert_json
            .get("repo_url")
            .or_else(|| alert_json.get("repository"))
            .or_else(|| alert_json.get("repo"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let mut harness_info = String::new();
        let mut project_harness_id = String::new();

        if !alert_repo.is_empty() {
            let harness_url = format!(
                "{temper_api_url}/tdata/ProjectHarnesses?$filter=repo_url eq '{}'&$top=1",
                alert_repo.replace('\'', "''")
            );
            let harness_resp = ctx.http_call("GET", &harness_url, &headers, "");
            if let Ok(resp) = harness_resp {
                if resp.status >= 200 && resp.status < 300 {
                    let harness_body: Value =
                        serde_json::from_str(&resp.body).unwrap_or(json!({}));
                    if let Some(items) = harness_body.get("value").and_then(|v| v.as_array()) {
                        if let Some(harness) = items.first() {
                            project_harness_id = harness
                                .get("entity_id")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            let harness_name = harness
                                .get("fields")
                                .and_then(|f| f.get("name"))
                                .and_then(|v| v.as_str())
                                .or_else(|| harness.get("name").and_then(|v| v.as_str()))
                                .unwrap_or("unknown");
                            harness_info = format!(
                                "- ProjectHarness: {project_harness_id} (name={harness_name}, repo={alert_repo})"
                            );
                        }
                    }
                }
            }
        }

        // 5. Build SRE user_message
        let severity = alert_json
            .get("severity")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let summary = alert_json
            .get("summary")
            .or_else(|| alert_json.get("message"))
            .or_else(|| alert_json.get("title"))
            .and_then(|v| v.as_str())
            .unwrap_or("n/a");
        let failure = alert_json
            .get("failure")
            .or_else(|| alert_json.get("error"))
            .and_then(|v| v.as_str())
            .unwrap_or("n/a");

        let priority_hint = match severity {
            "critical" | "sev0" | "sev1" => "1",
            "high" | "error" => "2",
            "medium" | "warn" | "warning" => "3",
            _ => "2",
        };

        let harness_section = if harness_info.is_empty() {
            "- ProjectHarness: not found (no matching repo in alert payload)".to_string()
        } else {
            harness_info
        };

        let sre_message = format!(
            "You are handling a real webhook-driven self-heal remediation.\n\n\
Workflow entity IDs:\n\
{harness_section}\n\
- Monitor: {monitor_id} (name={monitor_name}, dd_monitor_id={dd_monitor_id})\n\
- AlertCycle: {entity_id}\n\
- Repository: {alert_repo}\n\n\
Alert context:\n\
- Severity: {severity}\n\
- Summary: {summary}\n\
- Failure signal: {failure}\n\
- Raw alert payload JSON: {alert_payload}\n\n\
Treat this as a real issue unless the payload clearly proves it is monitor noise.\n\n\
Required workflow:\n\
1. Read the ProjectHarness, Monitor, and AlertCycle first.\n\
2. Record a concrete diagnosis in your own reasoning based on the failing symptom.\n\
3. For a confirmed real issue, create or reuse exactly one PM Issue before spawning a Developer:\n\
   - look for an existing non-final Issue only if it already covers this exact Monitor ID\n\
   - do not reuse an Issue that references a different Monitor ID even if the diagnosis text looks similar\n\
   - if none exists, create one new Issue entity\n\
   - set a description that includes the Monitor ID, AlertCycle ID, and later the WorkCycle ID once it exists\n\
   - set priority level {priority_hint}\n\
   - move the Issue into Triage so it is visible as active work\n\
4. Create or reuse exactly one WorkCycle tied to the ProjectHarness for the remediation.\n\
5. Spawn exactly one Developer child agent with:\n\
   - `soul_id = Developer`\n\
   - tools including `read,write,edit,bash,temper_get,temper_list,temper_action,read_entity`\n\
   - `workdir = {default_developer_workdir}`\n\
   - `background = false`\n\
   - do not invent a sandbox URL; let platform provisioning use the configured default sandbox\n\
   - do not spawn a replacement developer unless the first child reaches a terminal failed state\n\
6. Give the Developer a precise remediation task:\n\
   - clone the repository\n\
   - reproduce the issue from the alert payload\n\
   - apply the smallest safe fix\n\
   - run at least one concrete validation command\n\
   - commit, push, and open a PR if code changed\n\
7. After the child finishes, read back the updated WorkCycle and PM Issue.\n\
8. Close the loop:\n\
   - success: `WorkCycle.BeginTesting`, `WorkCycle.PassTests`, `WorkCycle.Approve`, `AlertCycle.HealComplete`\n\
   - monitor noise: `Monitor.Tune` then `AlertCycle.TuneComplete`\n\
   - failed remediation: `WorkCycle.Fail` then `AlertCycle.Escalate`\n\
9. If you created or updated a PM Issue, make sure its description reflects the final WorkCycle ID, AlertCycle ID, diagnosis, and PR URL.\n\n\
Your final response must include these exact keys on separate lines:\n\
ALERT_CYCLE_STATUS=<status>\n\
WORK_CYCLE_STATUS=<status or empty>\n\
PR_URL=<url or empty>\n\
DEVELOPER_AGENT_ID=<id or empty>\n\
ISSUE_ID=<id or empty>"
        );

        // 6. Create Agent entity
        let agent_url = format!("{temper_api_url}/tdata/Agents");
        let agent_body = json!({});
        let agent_resp = ctx.http_call("POST", &agent_url, &headers, &agent_body.to_string())?;
        if agent_resp.status < 200 || agent_resp.status >= 300 {
            return Err(format!(
                "alert_opener: failed to create Agent (HTTP {}): {}",
                agent_resp.status,
                &agent_resp.body[..agent_resp.body.len().min(300)]
            ));
        }
        let agent_parsed: Value = serde_json::from_str(&agent_resp.body)
            .map_err(|e| format!("alert_opener: failed to parse Agent response: {e}"))?;
        let agent_id = agent_parsed
            .get("entity_id")
            .and_then(|v| v.as_str())
            .ok_or("alert_opener: Agent creation did not return entity_id")?;

        ctx.log("info", &format!("alert_opener: created Agent {agent_id}"));

        // 7. Dispatch Agents.OpenPaw.Configure
        let configure_url = format!(
            "{temper_api_url}/tdata/Agents('{agent_id}')/OpenPaw.Configure"
        );
        let configure_body = json!({
            "model": default_agent_model,
            "provider": "anthropic",
            "tools_enabled": "temper_get,temper_list,temper_action,temper_create,spawn_agent,read_entity",
            "workdir": default_sre_workdir,
            "soul_id": "SRE",
            "temper_api_url": temper_api_url,
            "user_message": sre_message,
            "project_harness_id": project_harness_id,
        });
        let configure_resp =
            ctx.http_call("POST", &configure_url, &headers, &configure_body.to_string())?;
        if configure_resp.status < 200 || configure_resp.status >= 300 {
            return Err(format!(
                "alert_opener: Configure failed (HTTP {}): {}",
                configure_resp.status,
                &configure_resp.body[..configure_resp.body.len().min(300)]
            ));
        }
        ctx.log("info", &format!("alert_opener: configured Agent {agent_id}"));

        // 8. Dispatch Agents.OpenPaw.Provision
        let provision_url = format!(
            "{temper_api_url}/tdata/Agents('{agent_id}')/OpenPaw.Provision"
        );
        let provision_body = json!({});
        let provision_resp =
            ctx.http_call("POST", &provision_url, &headers, &provision_body.to_string())?;
        if provision_resp.status < 200 || provision_resp.status >= 300 {
            ctx.log(
                "warn",
                &format!(
                    "alert_opener: Provision dispatch returned HTTP {} (non-fatal, agent may self-provision)",
                    provision_resp.status
                ),
            );
        }
        ctx.log("info", &format!("alert_opener: provisioned Agent {agent_id}"));

        // 9. PATCH AlertCycle to set sre_agent_id
        let patch_url = format!(
            "{temper_api_url}/tdata/AlertCycles('{entity_id}')"
        );
        let patch_body = json!({ "sre_agent_id": agent_id });
        let patch_resp = ctx.http_call("PATCH", &patch_url, &headers, &patch_body.to_string())?;
        if patch_resp.status < 200 || patch_resp.status >= 300 {
            ctx.log(
                "warn",
                &format!(
                    "alert_opener: PATCH sre_agent_id failed (HTTP {})",
                    patch_resp.status
                ),
            );
        }

        // 10. Success — SRE agent will self-report via the AlertCycle state machine
        set_success_result("", &json!({ "sre_agent_id": agent_id }));

        ctx.log("info", &format!("alert_opener: done, SRE agent={agent_id}"));
        Ok(())
    })();

    if let Err(e) = result {
        set_error_result(&e);
    }
    0
}
