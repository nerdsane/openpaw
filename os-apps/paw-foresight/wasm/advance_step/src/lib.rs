//! Advance Step — WASM module for the Projection.AdvanceStep integration.
//!
//! Minimal timing coordinator. Does NOT provide pre-digested analysis to
//! probes. Steers active probes with the current time horizon and re-spawns
//! any probes whose sessions have reached a terminal state.
//!
//! Build: `cargo build --target wasm32-unknown-unknown --release`

use temper_wasm_sdk::prelude::*;

/// Entry point.
#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        ctx.log("info", "advance_step: starting");

        let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));

        // 1. Read Projection state
        let current_step = fields
            .get("current_step")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;
        let max_steps = fields
            .get("max_steps")
            .and_then(|v| v.as_u64())
            .unwrap_or(5) as usize;
        let product_model_id = fields
            .get("product_model_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let step_schedule_raw = fields
            .get("step_schedule")
            .cloned()
            .unwrap_or(json!([1, 3, 7, 14, 30]));
        let probe_agent_ids_raw = fields
            .get("probe_agent_ids")
            .cloned()
            .unwrap_or(json!([]));

        // Parse step_schedule
        let step_schedule: Vec<u64> = if let Some(arr) = step_schedule_raw.as_array() {
            arr.iter()
                .filter_map(|v| v.as_u64())
                .collect()
        } else if let Some(s) = step_schedule_raw.as_str() {
            serde_json::from_str(s).unwrap_or_else(|_| vec![1, 3, 7, 14, 30])
        } else {
            vec![1, 3, 7, 14, 30]
        };

        // Parse probe_agent_ids
        let probe_agent_ids: Vec<String> = if let Some(arr) = probe_agent_ids_raw.as_array() {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        } else if let Some(s) = probe_agent_ids_raw.as_str() {
            serde_json::from_str(s).unwrap_or_default()
        } else {
            vec![]
        };

        let entity_id = ctx
            .entity_state
            .get("entity_id")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        // 2. Get days_offset for current step
        let days_offset = if current_step < step_schedule.len() {
            step_schedule[current_step]
        } else {
            // Default: extrapolate linearly
            step_schedule.last().copied().unwrap_or(30)
        };

        ctx.log(
            "info",
            &format!(
                "advance_step: step {current_step}/{max_steps}, horizon={days_offset}d, probes={}",
                probe_agent_ids.len()
            ),
        );

        // Read config
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

        // 3. For each Probe agent, steer or re-spawn
        for agent_id in &probe_agent_ids {
            // 3a. Query latest Session for this agent
            let session_query_url = format!(
                "{temper_api_url}/tdata/Sessions?$filter=agent_id eq '{agent_id}'&$orderby=created_at desc&$top=1"
            );
            let session_resp = ctx.http_call("GET", &session_query_url, &headers, "")?;

            if session_resp.status < 200 || session_resp.status >= 300 {
                ctx.log(
                    "warn",
                    &format!(
                        "advance_step: failed to query Sessions for Agent {} (HTTP {})",
                        agent_id, session_resp.status
                    ),
                );
                continue;
            }

            let session_body: Value =
                serde_json::from_str(&session_resp.body).unwrap_or(json!({}));
            let sessions = session_body
                .get("value")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();

            if sessions.is_empty() {
                ctx.log(
                    "warn",
                    &format!("advance_step: no Sessions found for Agent {agent_id}, re-spawning"),
                );
                respawn_probe(&ctx, &temper_api_url, &headers, agent_id, product_model_id)?;
                continue;
            }

            let latest_session = &sessions[0];
            let session_id = latest_session
                .get("entity_id")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let session_state = latest_session
                .get("state")
                .and_then(|v| v.as_str())
                .or_else(|| {
                    latest_session
                        .get("fields")
                        .and_then(|f| f.get("state"))
                        .and_then(|v| v.as_str())
                })
                .unwrap_or("unknown");

            // 3b. Check if terminal
            let is_terminal = matches!(
                session_state,
                "Completed" | "Failed" | "Cancelled" | "completed" | "failed" | "cancelled"
            );

            if is_terminal {
                ctx.log(
                    "info",
                    &format!(
                        "advance_step: Session {session_id} is {session_state}, re-spawning probe {agent_id}"
                    ),
                );
                respawn_probe(&ctx, &temper_api_url, &headers, agent_id, product_model_id)?;
            } else {
                // 3c. Steer active session
                let steer_url = format!(
                    "{temper_api_url}/tdata/Sessions('{session_id}')/OpenPaw.Steer"
                );
                let steer_message = format!(
                    "[Foresight] Step {current_step} of {max_steps}. \
                     Time horizon: project what happens over the next {days_offset} days. \
                     ProductModel ID: {product_model_id}. \
                     Use temper_get to read the ProductModel. \
                     Use temper_list to read other Probes' Observations. \
                     Project what happens. Record Observations. \
                     Propose Directions when you see them."
                );
                let steer_body = json!({ "message": steer_message });
                let steer_resp = ctx.http_call(
                    "POST",
                    &steer_url,
                    &headers,
                    &steer_body.to_string(),
                )?;
                if steer_resp.status < 200 || steer_resp.status >= 300 {
                    ctx.log(
                        "warn",
                        &format!(
                            "advance_step: Steer failed for Session {} (HTTP {})",
                            session_id, steer_resp.status
                        ),
                    );
                } else {
                    ctx.log(
                        "info",
                        &format!("advance_step: steered Session {session_id} for Agent {agent_id}"),
                    );
                }
            }
        }

        // 4. Spawn Convergence Analyst for previous step's Observations
        //    The analyst is an LLM-powered agent that reads all Observations and
        //    identifies semantic convergence (not just signal_refs string matching).
        //    It runs asynchronously — does not block step advancement.
        if current_step > 0 {
            let prev_step = current_step - 1;
            let obs_url = format!(
                "{temper_api_url}/tdata/Observations?$filter=projection_id eq '{entity_id}'"
            );
            if let Ok(obs_resp) = ctx.http_call("GET", &obs_url, &headers, "") {
                if obs_resp.status >= 200 && obs_resp.status < 300 {
                    let obs_body: Value = serde_json::from_str(&obs_resp.body).unwrap_or(json!({}));
                    let observations = obs_body.get("value").and_then(|v| v.as_array()).cloned().unwrap_or_default();

                    // Guard: need 2+ different Probes
                    let mut probe_ids: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
                    for obs in &observations {
                        if let Some(pid) = obs.get("fields").and_then(|f| f.get("probe_agent_id")).and_then(|v| v.as_str()) {
                            if !pid.is_empty() { probe_ids.insert(pid.to_string()); }
                        }
                    }

                    if probe_ids.len() >= 2 && !observations.is_empty() {
                        let obs_json = serde_json::to_string_pretty(&observations).unwrap_or_default();
                        if let Err(e) = spawn_convergence_analyst(&ctx, &temper_api_url, &headers, entity_id, prev_step, &obs_json) {
                            ctx.log("warn", &format!("advance_step: failed to spawn convergence analyst: {e}"));
                        }
                    } else {
                        ctx.log("info", &format!("advance_step: skipping convergence (need 2+ probes, have {})", probe_ids.len()));
                    }
                }
            }
        }

        // 5. Check completion
        if current_step >= max_steps {
            ctx.log(
                "info",
                &format!("advance_step: Projection {entity_id} reached max_steps, completing"),
            );
            set_success_result("Complete", &json!({
                "final_step": current_step,
                "days_offset": days_offset
            }));
        } else {
            set_success_result("StepComplete", &json!({
                "current_step": current_step,
                "days_offset": days_offset,
                "probes_steered": probe_agent_ids.len()
            }));
        }

        ctx.log("info", "advance_step: done");
        Ok(())
    })();

    if let Err(e) = result {
        set_error_result(&e);
    }
    0
}

/// Spawn a Convergence Analyst agent to analyze Observations for semantic convergence.
/// Follows the same Agent+Session+Configure pattern as spawn_probes.
fn spawn_convergence_analyst(
    ctx: &Context,
    temper_api_url: &str,
    headers: &[(String, String)],
    projection_id: &str,
    prev_step: usize,
    observations_json: &str,
) -> Result<(), String> {
    // 1. Create Agent entity
    let agent_url = format!("{temper_api_url}/tdata/Agents");
    let agent_body = json!({
        "Name": format!("Convergence-Analyst-step-{}", prev_step),
        "Role": "convergence-analyst"
    });
    let agent_resp = ctx.http_call("POST", &agent_url, headers, &agent_body.to_string())?;
    if agent_resp.status < 200 || agent_resp.status >= 300 {
        return Err(format!("failed to create Agent (HTTP {})", agent_resp.status));
    }
    let agent_parsed: Value = serde_json::from_str(&agent_resp.body).unwrap_or(json!({}));
    let agent_id = agent_parsed.get("entity_id").and_then(|v| v.as_str()).unwrap_or("unknown");

    // 2. Create Session entity
    let session_url = format!("{temper_api_url}/tdata/Sessions");
    let session_resp = ctx.http_call("POST", &session_url, headers, &json!({}).to_string())?;
    if session_resp.status < 200 || session_resp.status >= 300 {
        return Err(format!("failed to create Session (HTTP {})", session_resp.status));
    }
    let session_parsed: Value = serde_json::from_str(&session_resp.body).unwrap_or(json!({}));
    let session_id = session_parsed.get("entity_id").and_then(|v| v.as_str()).unwrap_or("unknown");

    // 3. Configure — observations inline, no soul
    let configure_url = format!(
        "{temper_api_url}/tdata/Sessions('{session_id}')/OpenPaw.Configure"
    );

    let obs_for_prompt = if observations_json.len() > 40000 {
        format!("{}... (truncated)", &observations_json[..40000])
    } else {
        observations_json.to_string()
    };

    let user_message = format!(
        "You are a Convergence Analyst. Analyze these Observations for semantic convergence and contradictions.\n\n\
         Projection ID: {projection_id}\n\
         Step analyzed: {prev_step}\n\
         Your Agent ID: {agent_id}\n\n\
         Here are ALL Observations from this step:\n\n\
         {obs_for_prompt}\n\n\
         For each pair from DIFFERENT Probes:\n\
         - If they independently say the same thing → temper.action(\"Observations\", \"<id>\", \"Confirm\", \
           {{\"confirmer_agent_id\": \"{agent_id}\", \"confirmation_note\": \"Converges with <other_id>: <reason>\"}})\n\
         - If they contradict → temper.create(\"Observations\", {{\"content\": \"CONTRADICTION: ...\", \
           \"importance\": \"high\", \"signal_refs\": \"<merged>\", \"probe_agent_id\": \"{agent_id}\", \
           \"projection_id\": \"{projection_id}\", \"step_at\": \"{prev_step}\"}})\n\n\
         Be conservative: only Confirm when genuinely saying the same thing.\n\
         When done, call temper.done(\"complete\") with a summary."
    );

    let configure_body = json!({
        "model": "claude-sonnet-4-6",
        "agent_name": "convergence-analyst",
        "tools_enabled": "temper_get,temper_list,temper_action,temper_create",
        "max_turns": "30",
        "user_message": user_message
    });
    let configure_resp = ctx.http_call("POST", &configure_url, headers, &configure_body.to_string())?;
    if configure_resp.status < 200 || configure_resp.status >= 300 {
        ctx.log("warn", &format!(
            "spawn_convergence_analyst: Configure failed (HTTP {}): {}",
            configure_resp.status, &configure_resp.body[..configure_resp.body.len().min(200)]
        ));
    } else {
        ctx.log("info", &format!(
            "spawn_convergence_analyst: spawned analyst {agent_id} session {session_id} for step {prev_step}"
        ));
    }

    Ok(())
}

/// Re-spawn a probe by creating a new Session and configuring it.
fn respawn_probe(
    ctx: &Context,
    temper_api_url: &str,
    headers: &[(String, String)],
    agent_id: &str,
    product_model_id: &str,
) -> Result<(), String> {
    // Create new Session
    let session_url = format!("{temper_api_url}/tdata/Sessions");
    let session_body = json!({});
    let session_resp = ctx.http_call("POST", &session_url, headers, &session_body.to_string())?;
    if session_resp.status < 200 || session_resp.status >= 300 {
        ctx.log(
            "warn",
            &format!(
                "advance_step: failed to create Session for re-spawn of Agent {} (HTTP {})",
                agent_id, session_resp.status
            ),
        );
        return Ok(());
    }

    let session_parsed: Value =
        serde_json::from_str(&session_resp.body).unwrap_or(json!({}));
    let session_id = session_parsed
        .get("entity_id")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    // Configure new Session
    let configure_url = format!(
        "{temper_api_url}/tdata/Sessions('{session_id}')/OpenPaw.Configure"
    );
    let user_message = format!(
        "You are a Foresight Probe. ProductModel ID: {product_model_id}. \
         Use temper_get to read the ProductModel and begin projecting. \
         Read other Probes' Observations with temper_list."
    );
    let configure_body = json!({
        "model": "claude-sonnet-4-6",
        "soul_id": "Probe",
        "tools_enabled": "temper_get,temper_list,temper_action,temper_create",
        "max_turns": "50",
        "user_message": user_message,
        "temper_api_url": temper_api_url
    });
    let configure_resp = ctx.http_call(
        "POST",
        &configure_url,
        headers,
        &configure_body.to_string(),
    )?;
    if configure_resp.status < 200 || configure_resp.status >= 300 {
        ctx.log(
            "warn",
            &format!(
                "advance_step: Configure failed for re-spawned Session {} (HTTP {})",
                session_id, configure_resp.status
            ),
        );
    } else {
        ctx.log(
            "info",
            &format!(
                "advance_step: re-spawned probe {agent_id} with new Session {session_id}"
            ),
        );
    }

    Ok(())
}
