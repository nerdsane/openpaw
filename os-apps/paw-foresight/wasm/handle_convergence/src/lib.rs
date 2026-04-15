//! Handle Convergence — WASM module for Projection.ConvergenceComplete.
//!
//! Triggered when the Convergence Analyst finishes. Spawns a Model Projector
//! agent to produce the updated projected state for the next step.
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

        let current_step = fields
            .get("current_step")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;
        let max_steps = fields
            .get("max_steps")
            .and_then(|v| v.as_u64().or_else(|| v.as_str().and_then(|s| s.parse().ok())))
            .unwrap_or(5) as usize;
        let foresight_model_id = fields
            .get("foresight_model_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let projected_state_file_id = fields
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

        // If max_steps reached, complete now (no need for model projector)
        let next_step = current_step + 1;
        if next_step >= max_steps {
            ctx.log(
                "info",
                &format!("handle_convergence: reached max_steps ({next_step}/{max_steps}), completing"),
            );
            set_success_result("Complete", &json!({}));
            return Ok(());
        }

        ctx.log(
            "info",
            &format!(
                "handle_convergence: step {current_step} convergence done, spawning Model Projector"
            ),
        );

        // Read current state content (for the projector to evolve)
        let state_file_id = if !projected_state_file_id.is_empty() {
            projected_state_file_id.to_string()
        } else {
            // Use ForesightModel's knowledge graph
            let pm_url = format!("{temper_api_url}/tdata/ForesightModels('{foresight_model_id}')");
            let pm_resp = ctx.http_call("GET", &pm_url, &headers, "")?;
            if pm_resp.status >= 200 && pm_resp.status < 300 {
                let pm: Value = serde_json::from_str(&pm_resp.body).unwrap_or(json!({}));
                pm.get("fields")
                    .and_then(|f| f.get("model_snapshot_file_id"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string()
            } else {
                String::new()
            }
        };

        let current_state = if !state_file_id.is_empty() {
            let file_url = format!("{temper_api_url}/tdata/Files('{state_file_id}')/$value");
            match ctx.http_call("GET", &file_url, &headers, "") {
                Ok(resp) if resp.status >= 200 && resp.status < 300 => resp.body,
                _ => String::new(),
            }
        } else {
            String::new()
        };

        // Read confirmed observations from this step
        let obs_url = format!(
            "{temper_api_url}/tdata/Observations?$filter=projection_id eq '{entity_id}'"
        );
        let obs_resp = ctx.http_call("GET", &obs_url, &headers, "")?;
        let obs_json = if obs_resp.status >= 200 && obs_resp.status < 300 {
            let body: Value = serde_json::from_str(&obs_resp.body).unwrap_or(json!({}));
            serde_json::to_string_pretty(body.get("value").unwrap_or(&json!([]))).unwrap_or_default()
        } else {
            "[]".to_string()
        };

        // Read directions from this step
        let dir_url = format!(
            "{temper_api_url}/tdata/Directions?$filter=projection_id eq '{entity_id}'"
        );
        let dir_resp = ctx.http_call("GET", &dir_url, &headers, "")?;
        let dir_json = if dir_resp.status >= 200 && dir_resp.status < 300 {
            let body: Value = serde_json::from_str(&dir_resp.body).unwrap_or(json!({}));
            serde_json::to_string_pretty(body.get("value").unwrap_or(&json!([]))).unwrap_or_default()
        } else {
            "[]".to_string()
        };

        // Truncate for prompt
        let state_for_prompt = if current_state.len() > 20000 {
            format!("{}... (truncated)", &current_state[..20000])
        } else {
            current_state
        };
        let obs_for_prompt = if obs_json.len() > 15000 {
            format!("{}... (truncated)", &obs_json[..15000])
        } else {
            obs_json
        };
        let dir_for_prompt = if dir_json.len() > 5000 {
            format!("{}... (truncated)", &dir_json[..5000])
        } else {
            dir_json
        };

        // Spawn Model Projector agent
        let agent_url = format!("{temper_api_url}/tdata/Agents");
        let agent_body = json!({
            "Name": format!("Model-Projector-step-{}", current_step),
            "Role": "model-projector"
        });
        let agent_resp = ctx.http_call("POST", &agent_url, &headers, &agent_body.to_string())?;
        if agent_resp.status < 200 || agent_resp.status >= 300 {
            return Err(format!("failed to create Model Projector (HTTP {})", agent_resp.status));
        }
        let agent_parsed: Value = serde_json::from_str(&agent_resp.body).unwrap_or(json!({}));
        let agent_id = agent_parsed.get("entity_id").and_then(|v| v.as_str()).unwrap_or("unknown");

        let session_url = format!("{temper_api_url}/tdata/Sessions");
        let session_resp = ctx.http_call("POST", &session_url, &headers, &json!({"agent_id": agent_id}).to_string())?;
        if session_resp.status < 200 || session_resp.status >= 300 {
            return Err(format!("failed to create Model Projector Session (HTTP {})", session_resp.status));
        }
        let session_parsed: Value = serde_json::from_str(&session_resp.body).unwrap_or(json!({}));
        let session_id = session_parsed.get("entity_id").and_then(|v| v.as_str()).unwrap_or("unknown");

        let step_schedule_raw = fields.get("step_schedule").cloned().unwrap_or(json!([1, 3, 7]));
        let step_schedule: Vec<u64> = if let Some(arr) = step_schedule_raw.as_array() {
            arr.iter().filter_map(|v| v.as_u64()).collect()
        } else if let Some(s) = step_schedule_raw.as_str() {
            serde_json::from_str(s).unwrap_or_else(|_| vec![1, 3, 7])
        } else {
            vec![1, 3, 7]
        };
        let days_offset = if current_step < step_schedule.len() {
            step_schedule[current_step]
        } else {
            step_schedule.last().copied().unwrap_or(7)
        };

        let user_message = format!(
            "CRITICAL: When you finish, you MUST call BOTH of these in order:\n\
             1. temper.action(\"Projections\", \"{entity_id}\", \"ProjectionUpdated\", \
                {{\"projected_state_file_id\": \"<file_id_you_created>\"}})\n\
             2. temper.done(\"complete\")\n\
             If you do NOT call ProjectionUpdated, the projection stalls forever.\n\n\
             You are a Model Projector for Projection {entity_id}, step {current_step}.\n\
             Your Agent ID: {agent_id}\n\
             Simulated day offset: {days_offset} days\n\n\
             == CURRENT STATE OF THE WORLD ==\n\
             {state_for_prompt}\n\n\
             == OBSERVATIONS FROM THIS STEP (confirmed + created) ==\n\
             {obs_for_prompt}\n\n\
             == DIRECTIONS FROM THIS STEP ==\n\
             {dir_for_prompt}\n\n\
             == YOUR TASK ==\n\
             Produce an UPDATED projected state that represents how the simulated world \
             has evolved after {days_offset} simulated days. The probes observed and proposed \
             directions — your job is to synthesize what WOULD HAVE HAPPENED.\n\n\
             Read the observations and directions. Based on what the probes independently \
             projected, produce a new knowledge graph that reflects:\n\
             - What changed in this domain (new developments, structural shifts)\n\
             - What signals would exist now (new evidence, new trends, new content)\n\
             - How the domain's trajectory shifted\n\n\
             Structure as JSON:\n\
             {{\n\
               \"base_model\": <reference to original knowledge graph>,\n\
               \"step_history\": [<prior steps if any>, {{\n\
                 \"step\": {current_step}, \"day_offset\": {days_offset},\n\
                 \"convergent_findings\": \"<what probes agreed on>\",\n\
                 \"projected_changes\": \"<what changed in the simulated world>\"\n\
               }}],\n\
               \"current_projected_state\": <the world as it looks now — same structure \
                 as the base model but with projected additions>\n\
             }}\n\n\
             Upload this JSON:\n\
             1. Create a file: result = temper.create(\"Files\", {{\"Name\": \"projected_state_step_{current_step}.json\", \"MimeType\": \"application/json\"}})\n\
             2. Write content: temper.file_upload(\"projected_state_step_{current_step}.json\", <json_string>)\n\
             3. The file_id is result[\"entity_id\"]\n\n\
             Then call ProjectionUpdated with the file_id as instructed above."
        );

        // Read model/provider from probe_config
        let probe_config_raw = fields.get("probe_config").cloned().unwrap_or(json!([]));
        let probe_config: Vec<Value> = if let Some(arr) = probe_config_raw.as_array() {
            arr.clone()
        } else if let Some(s) = probe_config_raw.as_str() {
            serde_json::from_str(s).unwrap_or_default()
        } else {
            vec![]
        };
        let first_probe = probe_config.first().cloned().unwrap_or(json!({}));
        let model = first_probe.get("model").and_then(|v| v.as_str()).unwrap_or("claude-sonnet-4-6");
        let provider = first_probe.get("provider").and_then(|v| v.as_str()).unwrap_or("anthropic");

        let configure_url = format!(
            "{temper_api_url}/tdata/Sessions('{session_id}')/OpenPaw.Configure"
        );
        let configure_body = json!({
            "model": model,
            "provider": provider,
            "agent_name": "model-projector",
            "tools_enabled": "temper_get,temper_list,temper_action,temper_create,temper_write",
            "max_turns": "20",
            "user_message": user_message,
            "sandbox_url": "none",
            "temper_api_url": temper_api_url
        });
        let configure_resp = ctx.http_call("POST", &configure_url, &headers, &configure_body.to_string())?;
        if configure_resp.status < 200 || configure_resp.status >= 300 {
            ctx.log("warn", &format!(
                "handle_convergence: Configure failed (HTTP {})", configure_resp.status
            ));
        } else {
            ctx.log("info", &format!(
                "handle_convergence: spawned Model Projector {agent_id} session {session_id}"
            ));
        }

        // Return no chained action — wait for ProjectionUpdated
        set_success_result("", &json!({}));
        ctx.log("info", "handle_convergence: done, waiting for ProjectionUpdated");
        Ok(())
    })();

    if let Err(e) = result {
        set_error_result(&e);
    }
    0
}
