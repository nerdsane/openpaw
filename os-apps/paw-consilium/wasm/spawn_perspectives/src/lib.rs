use temper_wasm_sdk::prelude::*;

/// On Deliberation.Convene: creates N Perspective entities and N agent sessions
/// from config_json. Each agent gets the `deliberate` skill with its role, lens,
/// and the knowledge material. Deterministic fan-out.
///
/// Config_json format: [{ "role": "...", "model": "...", "provider": "...",
///   "temperature": "...", "lens": "..." }, ...]
#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        ctx.log("info", "spawn_perspectives: starting");

        let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));

        let entity_id = ctx
            .entity_state
            .get("entity_id")
            .and_then(|v| v.as_str())
            .unwrap_or(&ctx.entity_id)
            .to_string();

        let intent = fields
            .get("intent")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let knowledge_file_id = fields
            .get("knowledge_file_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let config_raw = fields
            .get("config_json")
            .and_then(|v| v.as_str())
            .unwrap_or("[]")
            .to_string();

        let configs: Vec<serde_json::Value> =
            serde_json::from_str(&config_raw).map_err(|e| format!("Invalid config_json: {e}"))?;

        if configs.is_empty() {
            return Err("spawn_perspectives: config_json is empty".to_string());
        }

        // --- Config ---
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
            .unwrap_or_else(|| "Consilium".to_string());

        let tenant = &ctx.tenant;
        let headers = vec![
            ("Content-Type".to_string(), "application/json".to_string()),
            ("X-Tenant-Id".to_string(), tenant.to_string()),
            ("x-temper-principal-kind".to_string(), "agent".to_string()),
            ("x-temper-principal-id".to_string(), "system".to_string()),
            ("x-temper-agent-type".to_string(), "system".to_string()),
        ];

        let mut perspective_ids: Vec<String> = Vec::new();

        for config in &configs {
            let role = config
                .get("role")
                .and_then(|v| v.as_str())
                .unwrap_or("analyst");
            let model = config
                .get("model")
                .and_then(|v| v.as_str())
                .filter(|value| !value.trim().is_empty())
                .ok_or("Perspective config requires model")?;
            let provider = config
                .get("provider")
                .and_then(|v| v.as_str())
                .filter(|value| !value.trim().is_empty())
                .ok_or("Perspective config requires provider")?;
            let temperature = config
                .get("temperature")
                .and_then(|v| v.as_str())
                .unwrap_or("1.0");
            let lens = config.get("lens").and_then(|v| v.as_str()).unwrap_or("");

            // Create Perspective entity
            let create_resp = ctx.http_call(
                "POST",
                &format!("{api_url}/tdata/Perspectives"),
                &headers,
                &json!({"fields": {}}).to_string(),
            )?;
            if !(200..300).contains(&create_resp.status) {
                return Err(format!(
                    "Failed to create Perspective: HTTP {}: {}",
                    create_resp.status,
                    &create_resp.body[..create_resp.body.len().min(300)]
                ));
            }
            let created: serde_json::Value = serde_json::from_str(&create_resp.body)
                .map_err(|e| format!("Failed to parse Perspective response: {e}"))?;
            let perspective_id = created
                .get("entity_id")
                .and_then(|v| v.as_str())
                .ok_or("Created Perspective has no entity_id")?
                .to_string();

            // Configure Perspective
            let config_body = json!({
                "deliberation_id": entity_id,
                "role": role,
                "model": model,
                "provider": provider,
                "temperature": temperature,
                "lens": lens,
            });
            let config_resp = ctx.http_call(
                "POST",
                &format!(
                    "{api_url}/tdata/Perspectives('{perspective_id}')/{odata_namespace}.Configure"
                ),
                &headers,
                &config_body.to_string(),
            )?;
            if !(200..300).contains(&config_resp.status) {
                return Err(format!(
                    "Failed to configure Perspective: HTTP {}: {}",
                    config_resp.status,
                    &config_resp.body[..config_resp.body.len().min(300)]
                ));
            }

            // Create Session for this perspective agent
            let create_session_resp = ctx.http_call(
                "POST",
                &format!("{api_url}/tdata/Sessions"),
                &headers,
                &json!({"fields": {}}).to_string(),
            )?;
            if !(200..300).contains(&create_session_resp.status) {
                return Err(format!(
                    "Failed to create Session: HTTP {}: {}",
                    create_session_resp.status,
                    &create_session_resp.body[..create_session_resp.body.len().min(300)]
                ));
            }
            let session_created: serde_json::Value =
                serde_json::from_str(&create_session_resp.body)
                    .map_err(|e| format!("Failed to parse Session response: {e}"))?;
            let session_id = session_created
                .get("entity_id")
                .and_then(|v| v.as_str())
                .ok_or("Created Session has no entity_id")?
                .to_string();

            // Build perspective-specific user_message
            let user_message = format!(
                r#"You are a perspective agent in a consilium deliberation.

## Your Role: {role}
## Your Lens: {lens}

## Deliberation Intent
{intent}

## Knowledge Material
Read the knowledge material from file ID: {knowledge_file_id}

## Instructions

Examine the knowledge material from your specific role and lens. You are context-isolated —
you cannot and must not try to read other perspectives. Your unique viewpoint is valuable
precisely because it is independent.

Use your `deliberate` skill to:
1. Read the knowledge material
2. Analyze it through your specific lens
3. Surface insights, tensions, questions, and angles that your role uniquely reveals
4. Write your perspective output to a paw-fs file

When done:
```
result = temper.write('/perspectives/{perspective_id}.md', your_perspective_text)
temper.action('Perspectives', '{perspective_id}', 'Complete', {{'output_file_id': result['file_id']}})
temper.done("perspective complete")
```

If you fail:
```
temper.action('Perspectives', '{perspective_id}', 'Fail', {{'error_message': reason}})
temper.done("perspective failed")
```
"#
            );

            // Configure Session
            let session_config = json!({
                "user_message": user_message,
                "model": model,
                "provider": provider,
                "temperature": temperature,
                "tools_enabled": "temper_get,temper_list,temper_create,temper_action,temper_write,temper_read",
                "max_turns": "50",
            });
            let session_config_resp = ctx.http_call(
                "POST",
                &format!("{api_url}/tdata/Sessions('{session_id}')/TemperPaw.Configure"),
                &headers,
                &session_config.to_string(),
            )?;
            if !(200..300).contains(&session_config_resp.status) {
                return Err(format!(
                    "Failed to configure Session: HTTP {}: {}",
                    session_config_resp.status,
                    &session_config_resp.body[..session_config_resp.body.len().min(300)]
                ));
            }

            // Mark Perspective as generating
            let begin_resp = ctx.http_call(
                "POST",
                &format!(
                    "{api_url}/tdata/Perspectives('{perspective_id}')/{odata_namespace}.Begin"
                ),
                &headers,
                &json!({"session_id": session_id}).to_string(),
            )?;
            if !(200..300).contains(&begin_resp.status) {
                ctx.log(
                    "warn",
                    &format!(
                        "Failed to Begin Perspective '{perspective_id}': HTTP {}",
                        begin_resp.status
                    ),
                );
            }

            perspective_ids.push(perspective_id);
        }

        // Dispatch AllSpawned on the Deliberation
        let all_spawned_resp = ctx.http_call(
            "POST",
            &format!("{api_url}/tdata/Deliberations('{entity_id}')/{odata_namespace}.AllSpawned"),
            &headers,
            &json!({"perspective_count": perspective_ids.len().to_string()}).to_string(),
        )?;
        if !(200..300).contains(&all_spawned_resp.status) {
            return Err(format!(
                "Failed to dispatch AllSpawned: HTTP {}: {}",
                all_spawned_resp.status,
                &all_spawned_resp.body[..all_spawned_resp.body.len().min(300)]
            ));
        }

        ctx.log(
            "info",
            &format!(
                "spawn_perspectives: spawned {} perspectives",
                perspective_ids.len()
            ),
        );

        set_success_result(
            "",
            &json!({
                "status": "ok",
                "perspective_count": perspective_ids.len(),
                "perspective_ids": perspective_ids,
            }),
        );
        Ok(())
    })();

    if let Err(e) = result {
        set_error_result(&e);
    }
    0
}
