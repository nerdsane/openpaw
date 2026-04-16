//! Spawn Probes — WASM module for the Projection.SpawnProbes integration.
//!
//! Creates Probe Agent+Session entities for a Projection. Each probe is an
//! independent agent configured with the Probe soul and pointed at the
//! ForesightModel knowledge graph.
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

        let foresight_model_id = fields
            .get("foresight_model_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if probe_config.is_empty() {
            return Err("spawn_probes: probe_config is required and must not be empty".to_string());
        }
        if foresight_model_id.is_empty() {
            return Err("spawn_probes: foresight_model_id is required".to_string());
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

        // 2b. Read ForesightModel entity for context
        let fm_url = format!("{temper_api_url}/tdata/ForesightModels('{foresight_model_id}')");
        let fm_resp = ctx.http_call("GET", &fm_url, &headers, "")?;
        let (fm_name, model_type, signal_source_config) = if fm_resp.status >= 200 && fm_resp.status < 300 {
            let fm: Value = serde_json::from_str(&fm_resp.body).unwrap_or(json!({}));
            let f = fm.get("fields").cloned().unwrap_or(json!({}));
            let name = f.get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            let mt = f.get("model_type")
                .and_then(|v| v.as_str())
                .unwrap_or("software_product")
                .to_string();
            let ssc = f.get("signal_source_config")
                .and_then(|v| v.as_str())
                .unwrap_or("{}")
                .to_string();
            (name, mt, ssc)
        } else {
            ctx.log(
                "warn",
                &format!(
                    "spawn_probes: failed to fetch ForesightModel (HTTP {})",
                    fm_resp.status
                ),
            );
            ("unknown".to_string(), "software_product".to_string(), "{}".to_string())
        };

        // 2c. Read the knowledge graph file content so we can include it in the prompt
        let knowledge_graph = if fm_resp.status >= 200 && fm_resp.status < 300 {
            let fm: Value = serde_json::from_str(&fm_resp.body).unwrap_or(json!({}));
            let file_id = fm
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
                "spawn_probes: spawning {} probes for ForesightModel {} ({}, type={})",
                probe_config.len(),
                foresight_model_id,
                fm_name,
                model_type
            ),
        );

        // 3. For each probe in probe_config, create Agent + Session + Configure
        let mut probe_agent_ids: Vec<String> = Vec::new();

        for probe in &probe_config {
            let probe_name = probe
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("Probe");
            let (probe_model, probe_provider) = validate_probe_entry(probe, probe_name)?;

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

            // 3b. Create Session (with agent_id so advance_step can find it)
            let session_url = format!("{temper_api_url}/tdata/Sessions");
            let session_body = json!({"agent_id": agent_id});
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
                "IMPORTANT: You MUST use the execute tool for ALL actions. Do NOT write analysis as text. \
                 ALL observations and directions must be created via temper.create() calls inside execute.\n\n\
                 You are {probe_name}, a Foresight Probe in a temporal simulation.\n\
                 Your job is to PROJECT WHERE THIS DOMAIN COULD GO, not just report what exists.\n\n\
                 Projection ID: {entity_id}\n\
                 ForesightModel ID: {foresight_model_id}\n\
                 Model Type: {model_type}\n\
                 Domain: {fm_name}\n\
                 Your Agent ID: {agent_id}\n\
                 Step: 0 (initial)\n\n\
                 == SIGNAL SOURCE CONFIGURATION ==\n\
                 {signal_source_config}\n\n\
                 == KNOWLEDGE GRAPH ==\n\
                 {kg_for_prompt}\n\n\
                 YOUR TASK:\n\
                 1. UNDERSTAND THE DOMAIN FIRST. Read the knowledge graph thoroughly. \
                    Use temper.web_fetch() to gather additional signals from URLs in the signal source config.\n\
                 2. Think about what this domain IS and where it's heading — trends, shifts, \
                    emerging patterns, risks.\n\
                 3. Project forward: where COULD this domain evolve?\n\
                 4. Create Observations about the domain's TRAJECTORY — focus on direction, \
                    emerging patterns, and potential shifts.\n\
                 5. Propose exactly ONE Direction — your single strongest thesis for where this \
                    domain is heading. Include step_at field.\n\n\
                 Work INDEPENDENTLY. Do NOT read other Probes' Observations.\n\n\
                 CRITICAL FIELD NAMES (API silently drops unknown fields):\n\n\
                 temper.create(\"Observations\", {{\n\
                   \"content\": \"What you see in the trajectory\",\n\
                   \"importance\": \"high\",\n\
                   \"signal_refs\": '[\"signal refs\"]',\n\
                   \"counterfactual\": \"What happens if this signal is ignored\",\n\
                   \"probe_agent_id\": \"{agent_id}\",\n\
                   \"projection_id\": \"{entity_id}\",\n\
                   \"step_at\": \"0\"\n\
                 }})\n\n\
                 temper.create(\"Directions\", {{\n\
                   \"title\": \"Direction title\",\n\
                   \"reasoning\": \"Full brief: why, what it enables, what it costs\",\n\
                   \"grounding\": '[\"signal refs\"]',\n\
                   \"observation_ids\": '[\"obs_id\"]',\n\
                   \"counterfactual_summary\": \"What if this direction is NOT taken\",\n\
                   \"proposer_agent_id\": \"{agent_id}\",\n\
                   \"projection_id\": \"{entity_id}\",\n\
                   \"step_at\": \"0\"\n\
                 }})\n\n\
                 When done, FIRST call:\n\
                 temper.action(\"Projections\", \"{entity_id}\", \"ProbeStepDone\",\n\
                   {{\"probe_agent_id\": \"{agent_id}\", \"direction_id\": \"<your_direction_id>\"}})\n\
                 THEN call temper.done(\"complete\")."
            );
            let configure_body = json!({
                "model": probe_model,
                "provider": probe_provider,
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

/// Validate that a `probe_config` entry specifies both `model` and `provider`.
///
/// Returns `(model, provider)` on success. Missing or empty values produce an
/// explicit error referencing openpaw#65 — the orchestrator session is the
/// source of truth for provider/model, and we refuse to silently fall back
/// to a hardcoded default that would 401 on codex-only tenants.
fn validate_probe_entry(
    probe: &Value,
    probe_name: &str,
) -> Result<(String, String), String> {
    let model = probe
        .get("model")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!(
            "spawn_probes: probe_config entry '{probe_name}' missing 'model' — \
             the orchestrator must populate probe_config with its own model/provider \
             (see openpaw#65)"
        ))?
        .to_string();
    let provider = probe
        .get("provider")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!(
            "spawn_probes: probe_config entry '{probe_name}' missing 'provider' — \
             the orchestrator must populate probe_config with its own model/provider \
             (see openpaw#65)"
        ))?
        .to_string();
    Ok((model, provider))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_probe_entry_accepts_complete_config() {
        let probe = json!({
            "name": "Probe-A",
            "model": "gpt-5",
            "provider": "openai_codex"
        });
        let result = validate_probe_entry(&probe, "Probe-A");
        assert_eq!(result, Ok(("gpt-5".into(), "openai_codex".into())));
    }

    #[test]
    fn validate_probe_entry_rejects_missing_provider() {
        let probe = json!({
            "name": "Probe-A",
            "model": "gpt-5"
        });
        let err = validate_probe_entry(&probe, "Probe-A").unwrap_err();
        assert!(err.contains("missing 'provider'"), "got: {err}");
        assert!(err.contains("openpaw#65"), "error must reference the issue");
    }

    #[test]
    fn validate_probe_entry_rejects_missing_model() {
        let probe = json!({
            "name": "Probe-A",
            "provider": "anthropic"
        });
        let err = validate_probe_entry(&probe, "Probe-A").unwrap_err();
        assert!(err.contains("missing 'model'"), "got: {err}");
        assert!(err.contains("openpaw#65"));
    }

    #[test]
    fn validate_probe_entry_rejects_empty_string_values() {
        // Empty string is as bad as missing — the .filter(|s| !s.is_empty())
        // guard rejects both, avoiding a silent configure-with-empty that
        // would produce a downstream 401 with no signal.
        let probe = json!({"name": "x", "model": "", "provider": "anthropic"});
        assert!(validate_probe_entry(&probe, "x").is_err());
        let probe = json!({"name": "x", "model": "gpt-5", "provider": ""});
        assert!(validate_probe_entry(&probe, "x").is_err());
    }

    #[test]
    fn validate_probe_entry_error_names_the_probe() {
        // Error messages must include the probe name so operators can locate
        // the offending entry when probe_config has multiple probes.
        let probe = json!({"name": "Convergence-Analyst"});
        let err = validate_probe_entry(&probe, "Convergence-Analyst").unwrap_err();
        assert!(err.contains("'Convergence-Analyst'"), "got: {err}");
    }
}
