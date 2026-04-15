use temper_wasm_sdk::prelude::*;

/// On ForesightModel.Seed (or RefreshSignals/Reactivate): spawns an agent
/// session to build or refresh the model's knowledge graph JSON.
///
/// The model_type field determines what the seed agent does — the agent
/// receives model_type + signal_source_config and builds whatever JSON
/// structure is appropriate for that domain.
///
/// This replaces the old monolithic seed_model WASM that hardcoded
/// GitHub/Datadog fetching. Intelligence is now in the seed skill.
#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        ctx.log("info", "spawn_seed_agent: starting");

        let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));

        let entity_id = ctx
            .entity_state
            .get("entity_id")
            .and_then(|v| v.as_str())
            .unwrap_or(&ctx.entity_id)
            .to_string();

        let name = fields
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("unnamed")
            .to_string();

        let model_type = fields
            .get("model_type")
            .and_then(|v| v.as_str())
            .unwrap_or("software_product")
            .to_string();

        let signal_source_config = fields
            .get("signal_source_config")
            .and_then(|v| v.as_str())
            .unwrap_or("{}")
            .to_string();

        let seed_model = fields
            .get("seed_model")
            .and_then(|v| v.as_str())
            .unwrap_or("claude-sonnet-4-6")
            .to_string();

        let seed_provider = fields
            .get("seed_provider")
            .and_then(|v| v.as_str())
            .unwrap_or("anthropic")
            .to_string();

        let seed_soul_id = fields
            .get("seed_soul_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let current_status = ctx
            .entity_state
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let is_refresh = current_status == "Active";

        // --- Config ---
        let api_url = ctx
            .config
            .get("temper_api_url")
            .filter(|s| !s.is_empty() && !s.contains("{secret:"))
            .cloned()
            .unwrap_or_else(|| "http://127.0.0.1:3000".to_string());

        let tenant = &ctx.tenant;
        let headers = vec![
            ("Content-Type".to_string(), "application/json".to_string()),
            ("X-Tenant-Id".to_string(), tenant.to_string()),
            ("x-temper-principal-kind".to_string(), "agent".to_string()),
            ("x-temper-principal-id".to_string(), "system".to_string()),
            ("x-temper-agent-type".to_string(), "system".to_string()),
        ];

        // --- Build user_message ---
        let callback_action = if is_refresh { "RefreshComplete" } else { "SeedComplete" };

        let user_message = format!(
            r#"You are seeding a ForesightModel ({model_type}) named '{name}'.
Model ID: {entity_id}
Operation: {operation}

## Signal Source Configuration

{signal_source_config}

## Instructions

Build a comprehensive knowledge graph JSON for this domain. The JSON structure
is not predefined — create whatever structure best represents this domain's
current state for foresight projection.

For model_type '{model_type}':
- START by researching the domain thoroughly using temper.web_search() —
  search for the domain name, key topics, recent developments, trends,
  competitive landscape, and any terms from the signal source config
- Use temper.web_fetch() to read specific URLs: pages from search results,
  URLs listed in signal_source_config, API endpoints, RSS feeds, etc.
- Use temper.list() / temper.get() for Temper entities if relevant
- Do NOT rely solely on signal_source_config — it provides starting points,
  but you should actively discover additional sources through web research
- Build a rich JSON that captures: current state, trends, key entities,
  relationships, signals, and metadata
- The more grounded your knowledge graph is in real, current data, the
  better the foresight probes will be able to project from it

When done:
1. Write the JSON to paw-fs (temper.write with filename 'foresight_model_{entity_id}.json')
2. Dispatch the callback:
```
temper.action('ForesightModels', '{entity_id}', '{callback_action}', {{
  'model_snapshot_file_id': '<file_id>',
  'signal_count': '<count>',
  'last_signal_refresh': '<timestamp>'
}})
temper.done("seed complete")
```

If you cannot complete:
```
temper.action('ForesightModels', '{entity_id}', 'SeedFailed', {{
  'error_message': '<reason>'
}})
temper.done("seed failed")
```
"#,
            operation = if is_refresh { "refresh" } else { "initial seed" },
        );

        // --- Create Session ---
        let create_resp = ctx.http_call(
            "POST",
            &format!("{api_url}/tdata/Sessions"),
            &headers,
            &json!({"fields": {}}).to_string(),
        )?;
        if !(200..300).contains(&create_resp.status) {
            return Err(format!(
                "Failed to create Session: HTTP {}: {}",
                create_resp.status,
                &create_resp.body[..create_resp.body.len().min(500)]
            ));
        }
        let created: serde_json::Value = serde_json::from_str(&create_resp.body)
            .map_err(|e| format!("Failed to parse Session response: {e}"))?;
        let session_id = created
            .get("entity_id")
            .and_then(|v| v.as_str())
            .ok_or("Created Session has no entity_id")?
            .to_string();

        // --- Configure Session ---
        let mut config_body = json!({
            "user_message": user_message,
            "model": seed_model,
            "provider": seed_provider,
            "tools_enabled": "temper_get,temper_list,temper_action,temper_create,temper_write,temper_read,temper_web_search,temper_web_fetch",
            "max_turns": "50",
        });
        if !seed_soul_id.is_empty() {
            config_body["soul_id"] = json!(seed_soul_id);
        }

        let configure_resp = ctx.http_call(
            "POST",
            &format!("{api_url}/tdata/Sessions('{session_id}')/OpenPaw.Configure"),
            &headers,
            &config_body.to_string(),
        )?;
        if !(200..300).contains(&configure_resp.status) {
            return Err(format!(
                "Failed to Configure Session: HTTP {}: {}",
                configure_resp.status,
                &configure_resp.body[..configure_resp.body.len().min(500)]
            ));
        }

        ctx.log("info", &format!(
            "spawn_seed_agent: session '{session_id}' for model_type '{model_type}'"
        ));
        set_success_result(
            "",
            &json!({
                "status": "ok",
                "session_id": session_id,
                "model_type": model_type,
            }),
        );
        Ok(())
    })();

    if let Err(e) = result {
        set_error_result(&e);
    }
    0
}
