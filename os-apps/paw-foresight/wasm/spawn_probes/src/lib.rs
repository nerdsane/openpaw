//! Spawn Probes — WASM module for the Projection.SpawnProbes integration.
//!
//! Creates Probe Agent+Session entities for a Projection. Each probe is an
//! independent agent configured with the Probe soul and pointed at the
//! ProductModel knowledge graph.
//!
//! Build: `cargo build --target wasm32-unknown-unknown --release`

use temper_wasm_sdk::prelude::*;

/// Entry point.
#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        ctx.log("info", "spawn_probes: starting");

        let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));

        // 1. Read Projection entity state
        let probe_config_raw = fields
            .get("probe_config")
            .cloned()
            .unwrap_or(json!([]));
        let probe_config: Vec<Value> = if let Some(arr) = probe_config_raw.as_array() {
            arr.clone()
        } else if let Some(s) = probe_config_raw.as_str() {
            serde_json::from_str(s).unwrap_or_default()
        } else {
            vec![]
        };

        let product_model_id = fields
            .get("product_model_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if probe_config.is_empty() {
            return Err("spawn_probes: probe_config is required and must not be empty".to_string());
        }
        if product_model_id.is_empty() {
            return Err("spawn_probes: product_model_id is required".to_string());
        }

        let entity_id = ctx
            .entity_state
            .get("entity_id")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        // 2. Read config
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

        // 2b. Read ProductModel entity for summary context
        let pm_url = format!("{temper_api_url}/tdata/ProductModels('{product_model_id}')");
        let pm_resp = ctx.http_call("GET", &pm_url, &headers, "")?;
        let pm_name = if pm_resp.status >= 200 && pm_resp.status < 300 {
            let pm: Value = serde_json::from_str(&pm_resp.body).unwrap_or(json!({}));
            pm.get("fields")
                .and_then(|f| f.get("name"))
                .and_then(|v| v.as_str())
                .or_else(|| pm.get("name").and_then(|v| v.as_str()))
                .unwrap_or("unknown")
                .to_string()
        } else {
            ctx.log(
                "warn",
                &format!(
                    "spawn_probes: failed to fetch ProductModel (HTTP {})",
                    pm_resp.status
                ),
            );
            "unknown".to_string()
        };

        // 2c. Read the knowledge graph file content so we can include it in the prompt
        //     (Probes can't read TemperFS files via read_entity in the sandbox)
        let knowledge_graph = if pm_resp.status >= 200 && pm_resp.status < 300 {
            let pm: Value = serde_json::from_str(&pm_resp.body).unwrap_or(json!({}));
            let file_id = pm
                .get("fields")
                .and_then(|f| f.get("model_snapshot_file_id"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if !file_id.is_empty() {
                let file_url = format!("{temper_api_url}/tdata/Files('{file_id}')/$value");
                match ctx.http_call("GET", &file_url, &headers, "") {
                    Ok(resp) if resp.status >= 200 && resp.status < 300 => {
                        ctx.log("info", &format!("spawn_probes: loaded knowledge graph ({} bytes)", resp.body.len()));
                        resp.body
                    }
                    _ => {
                        ctx.log("warn", "spawn_probes: could not read knowledge graph file");
                        String::new()
                    }
                }
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        ctx.log(
            "info",
            &format!(
                "spawn_probes: spawning {} probes for ProductModel {} ({})",
                probe_config.len(),
                product_model_id,
                pm_name
            ),
        );

        // 3. For each probe in probe_config, create Agent + Session + Configure
        let mut probe_agent_ids: Vec<String> = Vec::new();

        for probe in &probe_config {
            let probe_name = probe
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("Probe");
            let probe_model = probe
                .get("model")
                .and_then(|v| v.as_str())
                .unwrap_or("claude-sonnet-4-6");

            // 3a. Create Agent
            let agent_url = format!("{temper_api_url}/tdata/Agents");
            let agent_body = json!({
                "Name": probe_name,
                "Role": "probe",
                "SoulId": "Probe"
            });
            let agent_resp =
                ctx.http_call("POST", &agent_url, &headers, &agent_body.to_string())?;
            if agent_resp.status < 200 || agent_resp.status >= 300 {
                ctx.log(
                    "warn",
                    &format!(
                        "spawn_probes: failed to create Agent '{}' (HTTP {}): {}",
                        probe_name,
                        agent_resp.status,
                        &agent_resp.body[..agent_resp.body.len().min(300)]
                    ),
                );
                continue;
            }
            let agent_parsed: Value = serde_json::from_str(&agent_resp.body)
                .map_err(|e| format!("spawn_probes: failed to parse Agent response: {e}"))?;
            let agent_id = agent_parsed
                .get("entity_id")
                .and_then(|v| v.as_str())
                .ok_or("spawn_probes: Agent creation did not return entity_id")?;

            ctx.log(
                "info",
                &format!("spawn_probes: created Agent {agent_id} ({probe_name})"),
            );

            // 3b. Create Session
            let session_url = format!("{temper_api_url}/tdata/Sessions");
            let session_body = json!({});
            let session_resp =
                ctx.http_call("POST", &session_url, &headers, &session_body.to_string())?;
            if session_resp.status < 200 || session_resp.status >= 300 {
                ctx.log(
                    "warn",
                    &format!(
                        "spawn_probes: failed to create Session for Agent {} (HTTP {})",
                        agent_id, session_resp.status
                    ),
                );
                continue;
            }
            let session_parsed: Value = serde_json::from_str(&session_resp.body)
                .map_err(|e| format!("spawn_probes: failed to parse Session response: {e}"))?;
            let session_id = session_parsed
                .get("entity_id")
                .and_then(|v| v.as_str())
                .ok_or("spawn_probes: Session creation did not return entity_id")?;

            ctx.log(
                "info",
                &format!("spawn_probes: created Session {session_id} for Agent {agent_id}"),
            );

            // 3c. Configure Session
            let configure_url = format!(
                "{temper_api_url}/tdata/Sessions('{session_id}')/OpenPaw.Configure"
            );
            // Truncate knowledge graph if too large for a user message
            let kg_for_prompt = if knowledge_graph.len() > 40000 {
                format!("{}... (truncated)", &knowledge_graph[..40000])
            } else {
                knowledge_graph.clone()
            };

            let user_message = format!(
                "You are {probe_name}, a Foresight Probe. Analyze this product and project its future.\n\n\
                 Projection ID: {entity_id}\n\
                 ProductModel ID: {product_model_id}\n\
                 Your Agent ID: {agent_id}\n\
                 Step: 0 (initial)\n\n\
                 Here is the ProductModel knowledge graph (real signals from the codebase):\n\n\
                 {kg_for_prompt}\n\n\
                 IMPORTANT: Work INDEPENDENTLY. Do NOT read other Probes' Observations.\n\
                 Analyze the knowledge graph above. Create 3-5 Observations and 1-2 Directions.\n\n\
                 CRITICAL: Use EXACTLY these field names. The API silently drops unknown fields.\n\n\
                 temper.create(\"Observations\", {{\n\
                   \"content\": \"What you observed and why it matters\",\n\
                   \"importance\": \"high\",\n\
                   \"signal_refs\": '[\"commit:abc\", \"pr:42\"]',\n\
                   \"counterfactual\": \"What happens if this is ignored\",\n\
                   \"probe_agent_id\": \"{agent_id}\",\n\
                   \"projection_id\": \"{entity_id}\",\n\
                   \"step_at\": \"0\"\n\
                 }})\n\n\
                 temper.create(\"Directions\", {{\n\
                   \"title\": \"Short title for the direction\",\n\
                   \"reasoning\": \"Full reasoning about why this direction matters\",\n\
                   \"grounding\": '[\"signal refs from the knowledge graph\"]',\n\
                   \"observation_ids\": '[\"obs_id\"]',\n\
                   \"counterfactual_summary\": \"What happens if NOT taken\",\n\
                   \"proposer_agent_id\": \"{agent_id}\",\n\
                   \"projection_id\": \"{entity_id}\"\n\
                 }})\n\n\
                 DO NOT use 'body', 'description', or 'confidence' as field names.\n\
                 When done, call temper.done(\"complete\")."
            );
            let configure_body = json!({
                "model": probe_model,
                "soul_id": "Probe",
                "tools_enabled": "temper_get,temper_list,temper_action,temper_create,read_entity",
                "max_turns": "50",
                "user_message": user_message
            });
            let configure_resp = ctx.http_call(
                "POST",
                &configure_url,
                &headers,
                &configure_body.to_string(),
            )?;
            if configure_resp.status < 200 || configure_resp.status >= 300 {
                ctx.log(
                    "warn",
                    &format!(
                        "spawn_probes: Configure failed for Session {} (HTTP {}): {}",
                        session_id,
                        configure_resp.status,
                        &configure_resp.body[..configure_resp.body.len().min(300)]
                    ),
                );
                continue;
            }
            ctx.log(
                "info",
                &format!("spawn_probes: configured Session {session_id}"),
            );

            probe_agent_ids.push(agent_id.to_string());
        }

        if probe_agent_ids.is_empty() {
            return Err("spawn_probes: no probes were successfully created".to_string());
        }

        // 4-5. Return ProbesReady with probe_agent_ids
        let ids_json = serde_json::to_string(&probe_agent_ids)
            .unwrap_or_else(|_| "[]".to_string());

        set_success_result(
            "ProbesReady",
            &json!({
                "probe_agent_ids": ids_json
            }),
        );

        ctx.log(
            "info",
            &format!(
                "spawn_probes: done, {} probes spawned for Projection {}",
                probe_agent_ids.len(),
                entity_id
            ),
        );
        Ok(())
    })();

    if let Err(e) = result {
        set_error_result(&e);
    }
    0
}
