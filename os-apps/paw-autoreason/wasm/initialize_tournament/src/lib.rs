use temper_wasm_sdk::prelude::*;

/// On Tournament.Begin: creates the first Version A from the artifact,
/// creates the first Round, and spawns the referee agent.
#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        ctx.log("info", "initialize_tournament: starting");

        let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));

        let entity_id = ctx
            .entity_state
            .get("entity_id")
            .and_then(|v| v.as_str())
            .unwrap_or(&ctx.entity_id)
            .to_string();

        let artifact_file_id = fields
            .get("artifact_file_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        if artifact_file_id.is_empty() {
            return Err("initialize_tournament: no artifact_file_id".to_string());
        }

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

        // Create Version A (the original artifact)
        let version_resp = ctx.http_call(
            "POST",
            &format!("{api_url}/tdata/Versions"),
            &headers,
            &json!({"fields": {}}).to_string(),
        )?;
        if !(200..300).contains(&version_resp.status) {
            return Err(format!("Failed to create Version: HTTP {}", version_resp.status));
        }
        let version_created: serde_json::Value = serde_json::from_str(&version_resp.body)
            .map_err(|e| format!("Failed to parse Version response: {e}"))?;
        let version_a_id = version_created
            .get("entity_id")
            .and_then(|v| v.as_str())
            .ok_or("Version has no entity_id")?
            .to_string();

        // Configure and finalize Version A
        ctx.http_call(
            "POST",
            &format!("{api_url}/tdata/Versions('{version_a_id}')/{odata_namespace}.Configure"),
            &headers,
            &json!({
                "tournament_id": entity_id,
                "label": "A",
                "content_file_id": artifact_file_id,
            }).to_string(),
        )?;
        ctx.http_call(
            "POST",
            &format!("{api_url}/tdata/Versions('{version_a_id}')/{odata_namespace}.Finalize"),
            &headers,
            &json!({"content_file_id": artifact_file_id}).to_string(),
        )?;

        // Create Round 1
        let round_resp = ctx.http_call(
            "POST",
            &format!("{api_url}/tdata/Rounds"),
            &headers,
            &json!({"fields": {}}).to_string(),
        )?;
        if !(200..300).contains(&round_resp.status) {
            return Err(format!("Failed to create Round: HTTP {}", round_resp.status));
        }
        let round_created: serde_json::Value = serde_json::from_str(&round_resp.body)
            .map_err(|e| format!("Failed to parse Round response: {e}"))?;
        let round_id = round_created
            .get("entity_id")
            .and_then(|v| v.as_str())
            .ok_or("Round has no entity_id")?
            .to_string();

        ctx.http_call(
            "POST",
            &format!("{api_url}/tdata/Rounds('{round_id}')/{odata_namespace}.Configure"),
            &headers,
            &json!({
                "tournament_id": entity_id,
                "round_number": "1",
                "version_a_file_id": artifact_file_id,
            }).to_string(),
        )?;

        // Update tournament with version A
        ctx.http_call(
            "POST",
            &format!("{api_url}/tdata/Tournaments('{entity_id}')/{odata_namespace}.RoundReady"),
            &headers,
            &json!({"current_version_a_id": version_a_id}).to_string(),
        )?;

        ctx.log("info", &format!(
            "initialize_tournament: version_a={version_a_id}, round={round_id}"
        ));

        set_success_result(
            "",
            &json!({
                "status": "ok",
                "version_a_id": version_a_id,
                "round_id": round_id,
            }),
        );
        Ok(())
    })();

    if let Err(e) = result {
        set_error_result(&e);
    }
    0
}
