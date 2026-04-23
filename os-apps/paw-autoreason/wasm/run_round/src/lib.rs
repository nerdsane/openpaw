use temper_wasm_sdk::prelude::*;

/// On Tournament.NextRound: spawns a referee agent session that orchestrates
/// one tournament round (critique -> author B -> synthesizer AB -> judge spawning).
///
/// The referee is one smart agent that handles the full round via sub-agents.
/// WASM just spawns the referee — all intelligence is in the skill.
#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        ctx.log("info", "run_round: starting");

        let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));
        let counters = ctx
            .entity_state
            .get("counters")
            .cloned()
            .unwrap_or(json!({}));

        let entity_id = ctx
            .entity_state
            .get("entity_id")
            .and_then(|v| v.as_str())
            .unwrap_or(&ctx.entity_id)
            .to_string();

        let current_version_a_id = fields
            .get("current_version_a_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let domain_context = fields
            .get("domain_context")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let author_temperature = fields
            .get("author_temperature")
            .and_then(|v| v.as_str())
            .unwrap_or("0.8")
            .to_string();
        let author_model = fields
            .get("author_model")
            .and_then(|v| v.as_str())
            .filter(|value| !value.trim().is_empty())
            .ok_or("Tournament.author_model is required")?
            .to_string();
        let author_provider = fields
            .get("author_provider")
            .and_then(|v| v.as_str())
            .filter(|value| !value.trim().is_empty())
            .ok_or("Tournament.author_provider is required")?
            .to_string();

        let judge_temperature = fields
            .get("judge_temperature")
            .and_then(|v| v.as_str())
            .unwrap_or("0.3")
            .to_string();

        let judge_count = fields
            .get("judge_count")
            .and_then(|v| v.as_str())
            .unwrap_or("5")
            .to_string();

        let judge_config_json = fields
            .get("judge_config_json")
            .and_then(|v| v.as_str())
            .unwrap_or("[]")
            .to_string();

        let round_count = counters
            .get("round_count")
            .and_then(|v| v.as_i64())
            .unwrap_or(1);

        let api_url = ctx
            .config
            .get("temper_api_url")
            .filter(|s| !s.is_empty() && !s.contains("{secret:"))
            .cloned()
            .unwrap_or_else(|| "http://127.0.0.1:3000".to_string());

        let odata_namespace = ctx
            .config
            .get("odata_namespace")
            .filter(|s| !s.is_empty())
            .cloned()
            .unwrap_or_else(|| "Autoreason".to_string());

        let tenant = &ctx.tenant;
        let headers = vec![
            ("Content-Type".to_string(), "application/json".to_string()),
            ("X-Tenant-Id".to_string(), tenant.to_string()),
            ("x-temper-principal-kind".to_string(), "agent".to_string()),
            ("x-temper-principal-id".to_string(), "system".to_string()),
            ("x-temper-agent-type".to_string(), "system".to_string()),
        ];

        // Spawn referee session
        let create_resp = ctx.http_call(
            "POST",
            &format!("{api_url}/tdata/Sessions"),
            &headers,
            &json!({"fields": {}}).to_string(),
        )?;
        if !(200..300).contains(&create_resp.status) {
            return Err(format!(
                "Failed to create referee Session: HTTP {}",
                create_resp.status
            ));
        }
        let created: serde_json::Value = serde_json::from_str(&create_resp.body)
            .map_err(|e| format!("Failed to parse Session response: {e}"))?;
        let session_id = created
            .get("entity_id")
            .and_then(|v| v.as_str())
            .ok_or("Session has no entity_id")?
            .to_string();

        let user_message = format!(
            r#"You are the referee agent for tournament round {round_count}.

Tournament ID: {entity_id}
Current Version A ID: {current_version_a_id}
Round: {round_count}
OData Namespace: {odata_namespace}
Judge Count: {judge_count}
Author Temperature: {author_temperature}
Judge Temperature: {judge_temperature}
Judge Config: {judge_config_json}

## Domain Context
{domain_context}

## Instructions

Use your `referee` skill to orchestrate this round:
1. Spawn a critic agent to analyze Version A's weaknesses
2. Spawn an author agent to revise from A + critique -> Version B
3. Spawn a synthesizer agent to merge A + B -> Version AB (without seeing critique)
4. Create randomized label mappings for each judge (different per judge)
5. Spawn judge agents with randomized versions
6. When all judges submit, dispatch AllJudged on the Tournament

Follow the referee skill instructions precisely. Context firewalls are the protocol.
"#
        );

        let session_config = json!({
            "user_message": user_message,
            "model": author_model,
            "provider": author_provider,
            "temperature": author_temperature,
            "tools_enabled": "temper_get,temper_list,temper_create,temper_action,temper_write,temper_read",
            "max_turns": "100",
        });
        let config_resp = ctx.http_call(
            "POST",
            &format!("{api_url}/tdata/Sessions('{session_id}')/TemperPaw.Configure"),
            &headers,
            &session_config.to_string(),
        )?;
        if !(200..300).contains(&config_resp.status) {
            return Err(format!(
                "Failed to configure referee Session: HTTP {}",
                config_resp.status
            ));
        }

        ctx.log(
            "info",
            &format!("run_round: spawned referee session '{session_id}' for round {round_count}"),
        );

        set_success_result(
            "",
            &json!({
                "status": "ok",
                "session_id": session_id,
                "round": round_count,
            }),
        );
        Ok(())
    })();

    if let Err(e) = result {
        set_error_result(&e);
    }
    0
}
