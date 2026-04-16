//! Handle Probe Done — WASM module for Projection.ProbeStepDone.
//!
//! Triggered when a Probe self-reports completion of the current step.
//! Checks if ALL probes have reported (stateless query on Observations).
//! When all probes are done, spawns a Convergence Analyst with the expanded
//! role of producing a projected state update.
//!
//! Build: `cargo build --target wasm32-unknown-unknown --release`

use temper_wasm_sdk::prelude::*;

#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        ctx.log("info", "handle_probe_done: starting");

        let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));
        let entity_id = ctx
            .entity_state
            .get("entity_id")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        // Read Projection state
        let current_step = fields
            .get("current_step")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;
        let foresight_model_id = fields
            .get("foresight_model_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let probe_agent_ids_raw = fields
            .get("probe_agent_ids")
            .cloned()
            .unwrap_or(json!([]));
        let probe_agent_ids: Vec<String> = if let Some(arr) = probe_agent_ids_raw.as_array() {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        } else if let Some(s) = probe_agent_ids_raw.as_str() {
            serde_json::from_str(s).unwrap_or_default()
        } else {
            vec![]
        };
        let projected_state_file_id = fields
            .get("projected_state_file_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let step_schedule_raw = fields
            .get("step_schedule")
            .cloned()
            .unwrap_or(json!([1, 3, 7]));
        let step_schedule: Vec<u64> = if let Some(arr) = step_schedule_raw.as_array() {
            arr.iter().filter_map(|v| v.as_u64()).collect()
        } else if let Some(s) = step_schedule_raw.as_str() {
            serde_json::from_str(s).unwrap_or_else(|_| vec![1, 3, 7])
        } else {
            vec![1, 3, 7]
        };

        let num_probes = probe_agent_ids.len();
        if num_probes == 0 {
            return Err("handle_probe_done: no probe_agent_ids configured".to_string());
        }

        let temper_api_url = ctx
            .config
            .get("temper_api_url")
            .filter(|s| !s.is_empty() && !s.contains("{secret:"))
            .cloned()
            .unwrap_or_else(|| "http://127.0.0.1:3000".to_string());
        let tenant = &ctx.tenant;
        let headers = vec![
            ("content-type".to_string(), "application/json".to_string()),
            ("x-tenant-id".to_string(), tenant.to_string()),
            ("x-temper-principal-kind".to_string(), "agent".to_string()),
            ("x-temper-principal-id".to_string(), ctx.entity_id.clone()),
            ("x-temper-agent-type".to_string(), "system".to_string()),
        ];

        // Stateless check: count distinct probe_agent_ids with Observations for this step
        let obs_url = format!(
            "{temper_api_url}/tdata/Observations?$filter=projection_id eq '{entity_id}' and step_at eq '{current_step}'"
        );
        let obs_resp = ctx.http_call("GET", &obs_url, &headers, "")?;
        if obs_resp.status < 200 || obs_resp.status >= 300 {
            return Err(format!(
                "handle_probe_done: failed to query Observations (HTTP {})",
                obs_resp.status
            ));
        }
        let obs_body: Value = serde_json::from_str(&obs_resp.body).unwrap_or(json!({}));
        let observations = obs_body
            .get("value")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        // Count distinct probe_agent_ids from Observations
        let mut reported_probes: std::collections::BTreeSet<String> =
            std::collections::BTreeSet::new();
        for obs in &observations {
            if let Some(pid) = obs
                .get("fields")
                .and_then(|f| f.get("probe_agent_id"))
                .and_then(|v| v.as_str())
            {
                if !pid.is_empty() && probe_agent_ids.contains(&pid.to_string()) {
                    reported_probes.insert(pid.to_string());
                }
            }
        }

        // Also include the probe that just called ProbeStepDone (it may have
        // completed without creating Observations — still counts as reported)
        if let Some(caller_id) = ctx.trigger_params.get("probe_agent_id").and_then(|v| v.as_str()) {
            if !caller_id.is_empty() && probe_agent_ids.contains(&caller_id.to_string()) {
                reported_probes.insert(caller_id.to_string());
            }
        }

        // Check sessions: any probe whose session is terminal (Completed/Failed)
        // also counts as reported, even if it created no Observations
        for agent_id in &probe_agent_ids {
            if reported_probes.contains(agent_id) {
                continue;
            }
            let session_url = format!(
                "{temper_api_url}/tdata/Sessions?$filter=agent_id eq '{agent_id}'&$orderby=created_at desc&$top=1"
            );
            if let Ok(resp) = ctx.http_call("GET", &session_url, &headers, "") {
                if resp.status >= 200 && resp.status < 300 {
                    let body: Value = serde_json::from_str(&resp.body).unwrap_or(json!({}));
                    if let Some(sessions) = body.get("value").and_then(|v| v.as_array()) {
                        if let Some(s) = sessions.first() {
                            let status = s.get("status").and_then(|v| v.as_str()).unwrap_or("");
                            if matches!(status, "Completed" | "Failed" | "Cancelled") {
                                reported_probes.insert(agent_id.clone());
                            }
                        }
                    }
                }
            }
        }

        ctx.log(
            "info",
            &format!(
                "handle_probe_done: {}/{} probes reported for step {}",
                reported_probes.len(),
                num_probes,
                current_step
            ),
        );

        if reported_probes.len() < num_probes {
            // Not all probes reported yet — do nothing, wait for more ProbeStepDone events
            ctx.log("info", "handle_probe_done: waiting for more probes");
            set_success_result("", &json!({}));
            return Ok(());
        }

        // All probes reported — spawn Convergence Analyst with expanded role
        ctx.log(
            "info",
            &format!(
                "handle_probe_done: all {} probes reported for step {}, spawning Convergence Analyst",
                num_probes, current_step
            ),
        );

        // Read current projected state (or base knowledge graph for step 0)
        let state_file_id = if !projected_state_file_id.is_empty() {
            projected_state_file_id.to_string()
        } else {
            // Step 0: use the ForesightModel's model_snapshot_file_id
            let pm_url =
                format!("{temper_api_url}/tdata/ForesightModels('{foresight_model_id}')");
            let pm_resp = ctx.http_call("GET", &pm_url, &headers, "")?;
            if pm_resp.status < 200 || pm_resp.status >= 300 {
                return Err(format!(
                    "handle_probe_done: failed to fetch ForesightModel (HTTP {})",
                    pm_resp.status
                ));
            }
            let pm: Value = serde_json::from_str(&pm_resp.body).unwrap_or(json!({}));
            pm.get("fields")
                .and_then(|f| f.get("model_snapshot_file_id"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string()
        };

        let current_state_content = if !state_file_id.is_empty() {
            let file_url =
                format!("{temper_api_url}/tdata/Files('{state_file_id}')/$value");
            match ctx.http_call("GET", &file_url, &headers, "") {
                Ok(resp) if resp.status >= 200 && resp.status < 300 => resp.body,
                _ => {
                    ctx.log("warn", "handle_probe_done: could not read projected state file");
                    String::new()
                }
            }
        } else {
            String::new()
        };

        // Serialize observations for the analyst
        let obs_json = serde_json::to_string_pretty(&observations).unwrap_or_default();

        // Get days_offset for current step
        let days_offset = if current_step < step_schedule.len() {
            step_schedule[current_step]
        } else {
            step_schedule.last().copied().unwrap_or(7)
        };

        // Spawn Convergence Analyst
        spawn_convergence_analyst(
            &ctx,
            &temper_api_url,
            &headers,
            entity_id,
            current_step,
            days_offset,
            &obs_json,
            &current_state_content,
        )?;

        // Return no chained action — wait for ConvergenceComplete
        set_success_result("", &json!({}));
        ctx.log("info", "handle_probe_done: done, waiting for ConvergenceComplete");
        Ok(())
    })();

    if let Err(e) = result {
        set_error_result(&e);
    }
    0
}

fn spawn_convergence_analyst(
    ctx: &Context,
    temper_api_url: &str,
    headers: &[(String, String)],
    projection_id: &str,
    current_step: usize,
    days_offset: u64,
    observations_json: &str,
    current_state_content: &str,
) -> Result<(), String> {
    // Create Agent entity
    let agent_url = format!("{temper_api_url}/tdata/Agents");
    let agent_body = json!({
        "Name": format!("Convergence-Analyst-step-{}", current_step),
        "Role": "convergence-analyst"
    });
    let agent_resp = ctx.http_call("POST", &agent_url, headers, &agent_body.to_string())?;
    if agent_resp.status < 200 || agent_resp.status >= 300 {
        return Err(format!(
            "failed to create Convergence Analyst Agent (HTTP {})",
            agent_resp.status
        ));
    }
    let agent_parsed: Value = serde_json::from_str(&agent_resp.body).unwrap_or(json!({}));
    let agent_id = agent_parsed
        .get("entity_id")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    // Create Session entity
    let session_url = format!("{temper_api_url}/tdata/Sessions");
    let session_resp =
        ctx.http_call("POST", &session_url, headers, &json!({"agent_id": agent_id}).to_string())?;
    if session_resp.status < 200 || session_resp.status >= 300 {
        return Err(format!(
            "failed to create Convergence Analyst Session (HTTP {})",
            session_resp.status
        ));
    }
    let session_parsed: Value = serde_json::from_str(&session_resp.body).unwrap_or(json!({}));
    let session_id = session_parsed
        .get("entity_id")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    // Truncate inputs if needed
    let obs_for_prompt = if observations_json.len() > 30000 {
        format!("{}... (truncated)", &observations_json[..30000])
    } else {
        observations_json.to_string()
    };
    let state_for_prompt = if current_state_content.len() > 20000 {
        format!("{}... (truncated)", &current_state_content[..20000])
    } else {
        current_state_content.to_string()
    };

    let user_message = format!(
        "CRITICAL: When you finish ALL your work, you MUST call BOTH of these in order:\n\
         1. temper.action(\"Projections\", \"{projection_id}\", \"ConvergenceComplete\", \
            {{\"projected_state_file_id\": \"\"}})\n\
         2. temper.done(\"complete\")\n\
         If you do NOT call ConvergenceComplete, the projection will stall forever.\n\n\
         You are a Convergence Analyst for Projection {projection_id}, step {current_step}.\n\
         Your Agent ID: {agent_id}\n\
         Simulated day offset: {days_offset} days\n\n\
         == CURRENT PROJECTED STATE ==\n\
         {state_for_prompt}\n\n\
         == ALL OBSERVATIONS FROM STEP {current_step} ==\n\
         {obs_for_prompt}\n\n\
         == YOUR TASKS ==\n\n\
         For each pair of Observations from DIFFERENT Probes:\n\
         - If they independently say the same thing → temper.action(\"Observations\", \"<id>\", \"Confirm\", \
           {{\"confirmer_agent_id\": \"{agent_id}\", \"confirmation_note\": \"Converges with <other_id>: <reason>\"}})\n\
         - If they contradict → temper.create(\"Observations\", {{\"content\": \"CONTRADICTION: ...\", \
           \"importance\": \"high\", \"probe_agent_id\": \"{agent_id}\", \
           \"projection_id\": \"{projection_id}\", \"step_at\": \"{current_step}\"}})\n\n\
         Be conservative: only Confirm when genuinely saying the same thing.\n\n\
         When done analyzing, call ConvergenceComplete as instructed above, then temper.done(\"complete\")."
    );

    // Configure Session — read model/provider from Projection's probe_config
    let configure_url = format!(
        "{temper_api_url}/tdata/Sessions('{session_id}')/OpenPaw.Configure"
    );
    let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));
    let probe_config_raw = fields.get("probe_config").cloned().unwrap_or(json!([]));
    let probe_config: Vec<Value> = if let Some(arr) = probe_config_raw.as_array() {
        arr.clone()
    } else if let Some(s) = probe_config_raw.as_str() {
        serde_json::from_str(s).unwrap_or_default()
    } else {
        vec![]
    };
    // Analyst inherits provider/model from probe_config[0]. Missing values are
    // a configuration error rather than a silent fallback (openpaw#65).
    let first_probe = probe_config.first().cloned().unwrap_or(json!({}));
    let analyst_model = first_probe
        .get("model")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "spawn_convergence_analyst: probe_config[0] missing 'model' — orchestrator must populate probe_config (openpaw#65)".to_string())?;
    let analyst_provider = first_probe
        .get("provider")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "spawn_convergence_analyst: probe_config[0] missing 'provider' — orchestrator must populate probe_config (openpaw#65)".to_string())?;
    let configure_body = json!({
        "model": analyst_model,
        "provider": analyst_provider,
        "agent_name": "convergence-analyst",
        "tools_enabled": "temper_get,temper_list,temper_action,temper_create,temper_write",
        "max_turns": "40",
        "user_message": user_message,
        "sandbox_url": "none",
        "temper_api_url": temper_api_url
    });
    let configure_resp =
        ctx.http_call("POST", &configure_url, headers, &configure_body.to_string())?;
    if configure_resp.status < 200 || configure_resp.status >= 300 {
        ctx.log(
            "warn",
            &format!(
                "handle_probe_done: Configure failed (HTTP {}): {}",
                configure_resp.status,
                &configure_resp.body[..configure_resp.body.len().min(200)]
            ),
        );
    } else {
        ctx.log(
            "info",
            &format!(
                "handle_probe_done: spawned Convergence Analyst {agent_id} session {session_id} for step {current_step}"
            ),
        );
    }

    Ok(())
}
