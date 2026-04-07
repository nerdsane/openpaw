//! Handle Convergence — WASM module for Projection.ConvergenceComplete.
//!
//! Triggered when the Convergence Analyst finishes and provides an updated
//! projected state. Respawns all Probes for the next step with:
//!   - The new projected state (simulated world evolution)
//!   - Each Probe's own prior Observations (episodic memory)
//!   - Each Probe's own prior Direction
//!
//! If max_steps reached, completes the Projection instead.
//!
//! Build: `cargo build --target wasm32-unknown-unknown --release`

use temper_wasm_sdk::prelude::*;

#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        ctx.log("info", "handle_convergence: starting");

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
        let max_steps = fields
            .get("max_steps")
            .and_then(|v| v.as_u64())
            .unwrap_or(5) as usize;
        let product_model_id = fields
            .get("product_model_id")
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

        // Read projected_state_file_id from trigger params (the NEW projected state)
        let new_projected_state_file_id = ctx
            .trigger_params
            .get("projected_state_file_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");

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

        // Next step = current_step + 1 (AdvanceStep will increment the counter)
        let next_step = current_step + 1;

        ctx.log(
            "info",
            &format!(
                "handle_convergence: step {current_step} complete, projected_state={}, next_step={next_step}, max_steps={max_steps}",
                if new_projected_state_file_id.is_empty() { "none" } else { new_projected_state_file_id }
            ),
        );

        // Check if we've reached max_steps
        if next_step >= max_steps {
            ctx.log(
                "info",
                &format!(
                    "handle_convergence: Projection reached max_steps ({next_step}/{max_steps}), completing"
                ),
            );
            set_success_result("Complete", &json!({}));
            return Ok(());
        }

        // Read the new projected state content for the probe prompts
        let projected_state_content = if !new_projected_state_file_id.is_empty() {
            let file_url = format!(
                "{temper_api_url}/tdata/Files('{new_projected_state_file_id}')/$value"
            );
            match ctx.http_call("GET", &file_url, &headers, "") {
                Ok(resp) if resp.status >= 200 && resp.status < 300 => {
                    ctx.log(
                        "info",
                        &format!(
                            "handle_convergence: loaded projected state ({} bytes)",
                            resp.body.len()
                        ),
                    );
                    resp.body
                }
                _ => {
                    ctx.log("warn", "handle_convergence: could not read projected state file");
                    String::new()
                }
            }
        } else {
            String::new()
        };

        // Get days_offset for the NEXT step
        let next_days_offset = if next_step < step_schedule.len() {
            step_schedule[next_step]
        } else {
            step_schedule.last().copied().unwrap_or(7)
        };

        // Respawn each Probe with episodic memory + projected state
        for agent_id in &probe_agent_ids {
            if let Err(e) = respawn_probe_with_memory(
                &ctx,
                &temper_api_url,
                &headers,
                agent_id,
                entity_id,
                product_model_id,
                next_step,
                next_days_offset,
                &projected_state_content,
            ) {
                ctx.log(
                    "warn",
                    &format!("handle_convergence: failed to respawn Probe {agent_id}: {e}"),
                );
            }
        }

        // Return AdvanceStep to increment the step counter
        set_success_result(
            "AdvanceStep",
            &json!({
                "projected_state_file_id": new_projected_state_file_id,
            }),
        );
        ctx.log("info", "handle_convergence: done, probes respawned, advancing step");
        Ok(())
    })();

    if let Err(e) = result {
        set_error_result(&e);
    }
    0
}

fn respawn_probe_with_memory(
    ctx: &Context,
    temper_api_url: &str,
    headers: &[(String, String)],
    agent_id: &str,
    projection_id: &str,
    product_model_id: &str,
    next_step: usize,
    days_offset: u64,
    projected_state_content: &str,
) -> Result<(), String> {
    // Read this Probe's own prior Observations
    let obs_url = format!(
        "{temper_api_url}/tdata/Observations?$filter=probe_agent_id eq '{agent_id}' and projection_id eq '{projection_id}'"
    );
    let obs_resp = ctx.http_call("GET", &obs_url, headers, "")?;
    let own_observations = if obs_resp.status >= 200 && obs_resp.status < 300 {
        let body: Value = serde_json::from_str(&obs_resp.body).unwrap_or(json!({}));
        let obs = body.get("value").and_then(|v| v.as_array()).cloned().unwrap_or_default();
        // Extract just the key fields for the prompt
        let summaries: Vec<Value> = obs
            .iter()
            .map(|o| {
                let f = o.get("fields").cloned().unwrap_or(json!({}));
                json!({
                    "id": o.get("entity_id").and_then(|v| v.as_str()).unwrap_or(""),
                    "step": f.get("step_at").and_then(|v| v.as_str()).unwrap_or("?"),
                    "content": f.get("Content").or(f.get("content")).and_then(|v| v.as_str()).unwrap_or(""),
                    "importance": f.get("Importance").or(f.get("importance")).and_then(|v| v.as_str()).unwrap_or(""),
                    "status": o.get("status").and_then(|v| v.as_str()).unwrap_or("")
                })
            })
            .collect();
        serde_json::to_string_pretty(&summaries).unwrap_or_default()
    } else {
        "[]".to_string()
    };

    // Read this Probe's own prior Direction(s)
    let dir_url = format!(
        "{temper_api_url}/tdata/Directions?$filter=proposer_agent_id eq '{agent_id}' and projection_id eq '{projection_id}'&$orderby=created_at desc&$top=1"
    );
    let dir_resp = ctx.http_call("GET", &dir_url, headers, "")?;
    let own_direction = if dir_resp.status >= 200 && dir_resp.status < 300 {
        let body: Value = serde_json::from_str(&dir_resp.body).unwrap_or(json!({}));
        let dirs = body.get("value").and_then(|v| v.as_array()).cloned().unwrap_or_default();
        if let Some(d) = dirs.first() {
            let f = d.get("fields").cloned().unwrap_or(json!({}));
            serde_json::to_string_pretty(&json!({
                "id": d.get("entity_id").and_then(|v| v.as_str()).unwrap_or(""),
                "title": f.get("Title").or(f.get("title")).and_then(|v| v.as_str()).unwrap_or(""),
                "reasoning": f.get("Reasoning").or(f.get("reasoning")).and_then(|v| v.as_str()).unwrap_or("")
            }))
            .unwrap_or_default()
        } else {
            "null".to_string()
        }
    } else {
        "null".to_string()
    };

    // Truncate projected state if needed
    let state_for_prompt = if projected_state_content.len() > 30000 {
        format!("{}... (truncated)", &projected_state_content[..30000])
    } else {
        projected_state_content.to_string()
    };

    // Truncate observations if needed
    let obs_for_prompt = if own_observations.len() > 10000 {
        format!("{}... (truncated)", &own_observations[..10000])
    } else {
        own_observations.clone()
    };

    // Create new Session
    let session_url = format!("{temper_api_url}/tdata/Sessions");
    let session_resp = ctx.http_call(
        "POST",
        &session_url,
        headers,
        &json!({"agent_id": agent_id}).to_string(),
    )?;
    if session_resp.status < 200 || session_resp.status >= 300 {
        return Err(format!(
            "failed to create Session for Probe {agent_id} (HTTP {})",
            session_resp.status
        ));
    }
    let session_parsed: Value = serde_json::from_str(&session_resp.body).unwrap_or(json!({}));
    let session_id = session_parsed
        .get("entity_id")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    // Build episodic prompt
    let user_message = format!(
        "IMPORTANT: You MUST use the execute tool for ALL actions. Do NOT write analysis as text.\n\n\
         You are a Foresight Probe at step {next_step} of a temporal simulation.\n\n\
         Projection ID: {projection_id}\n\
         ProductModel ID: {product_model_id}\n\
         Your Agent ID: {agent_id}\n\
         Simulated day offset: {days_offset} days from start\n\n\
         == PROJECTED STATE OF THE WORLD ==\n\
         {state_for_prompt}\n\n\
         == YOUR PRIOR OBSERVATIONS ==\n\
         {obs_for_prompt}\n\n\
         == YOUR PRIOR DIRECTION ==\n\
         {own_direction}\n\n\
         == YOUR TASK ==\n\
         The simulated world has evolved. The projected state reflects convergent\n\
         projections from all probes.\n\n\
         1. Review the updated projected state — what has changed?\n\
         2. Review your prior observations and direction\n\
         3. DIRECTION VERSIONING: Archive your old Direction, then create a revised one:\n\
            temper.action(\"Directions\", \"<old_direction_id>\", \"Archive\", \
              {{\"archive_reason\": \"Revised in step {next_step}\"}})\n\
            Then create new Direction with parent_direction_id pointing to old one.\n\
         4. Create new Observations for step {next_step}\n\
         5. Propose exactly ONE Direction — revise or double down\n\n\
         CRITICAL FIELD NAMES:\n\n\
         temper.create(\"Observations\", {{\n\
           \"content\": \"What you observe in the projected trajectory\",\n\
           \"importance\": \"high\",\n\
           \"signal_refs\": '[\"...\"]',\n\
           \"counterfactual\": \"What happens if this is ignored\",\n\
           \"probe_agent_id\": \"{agent_id}\",\n\
           \"projection_id\": \"{projection_id}\",\n\
           \"step_at\": \"{next_step}\"\n\
         }})\n\n\
         temper.create(\"Directions\", {{\n\
           \"title\": \"Direction title\",\n\
           \"reasoning\": \"Full product brief\",\n\
           \"grounding\": '[\"signal refs\"]',\n\
           \"observation_ids\": '[\"obs_id\"]',\n\
           \"counterfactual_summary\": \"What if not taken\",\n\
           \"proposer_agent_id\": \"{agent_id}\",\n\
           \"projection_id\": \"{projection_id}\",\n\
           \"parent_direction_id\": \"<old_direction_id>\",\n\
           \"step_at\": \"{next_step}\"\n\
         }})\n\n\
         When done, call:\n\
         temper.action(\"Projections\", \"{projection_id}\", \"ProbeStepDone\",\n\
           {{\"probe_agent_id\": \"{agent_id}\", \"direction_id\": \"<your_direction_id>\"}})\n\
         then temper.done(\"complete\")"
    );

    // Configure Session
    let configure_url = format!(
        "{temper_api_url}/tdata/Sessions('{session_id}')/OpenPaw.Configure"
    );
    let configure_body = json!({
        "model": "claude-sonnet-4-6",
        "soul_id": "Probe",
        "tools_enabled": "temper_get,temper_list,temper_action,temper_create,temper_read",
        "max_turns": "50",
        "user_message": user_message,
        "sandbox_url": "none",
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
                "handle_convergence: Configure failed for Session {} (HTTP {})",
                session_id, configure_resp.status
            ),
        );
    } else {
        ctx.log(
            "info",
            &format!(
                "handle_convergence: respawned Probe {agent_id} with session {session_id} for step {next_step}"
            ),
        );
    }

    Ok(())
}
