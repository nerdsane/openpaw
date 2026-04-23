use temper_wasm_sdk::prelude::*;

/// Fan-in: on each Perspective completion, checks if all perspectives are done.
/// When all complete, spawns one synthesizer agent with all perspective outputs
/// to produce the Brief. Deterministic counting + agent spawn.
#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        ctx.log("info", "check_and_synthesize: starting");

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

        let perspective_count: usize = fields
            .get("perspective_count")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let perspectives_complete: usize = fields
            .get("perspectives_complete")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        // Not all done yet — noop
        if perspectives_complete < perspective_count {
            ctx.log("info", &format!(
                "check_and_synthesize: {perspectives_complete}/{perspective_count} complete, waiting"
            ));
            set_success_result(
                "",
                &json!({
                    "status": "waiting",
                    "complete": perspectives_complete,
                    "total": perspective_count,
                }),
            );
            return Ok(());
        }

        let synthesis_model = fields
            .get("synthesis_model")
            .and_then(|v| v.as_str())
            .filter(|value| !value.trim().is_empty())
            .ok_or("Deliberation.synthesis_model is required")?;
        let synthesis_provider = fields
            .get("synthesis_provider")
            .and_then(|v| v.as_str())
            .filter(|value| !value.trim().is_empty())
            .ok_or("Deliberation.synthesis_provider is required")?;

        ctx.log(
            "info",
            "check_and_synthesize: all perspectives complete, spawning synthesizer",
        );

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

        // Transition to Synthesizing
        let synth_resp = ctx.http_call(
            "POST",
            &format!(
                "{api_url}/tdata/Deliberations('{entity_id}')/{odata_namespace}.BeginSynthesis"
            ),
            &headers,
            "{}",
        )?;
        if !(200..300).contains(&synth_resp.status) {
            return Err(format!(
                "Failed to BeginSynthesis: HTTP {}: {}",
                synth_resp.status,
                &synth_resp.body[..synth_resp.body.len().min(300)]
            ));
        }

        // Query all completed perspectives for this deliberation
        let filter = format!(
            "DeliberationId%20eq%20'{}'%20and%20State%20eq%20'Complete'",
            urlenc(&entity_id)
        );
        let list_resp = ctx.http_call(
            "GET",
            &format!("{api_url}/tdata/Perspectives?$filter={filter}"),
            &headers,
            "",
        )?;

        let mut perspective_outputs: Vec<serde_json::Value> = Vec::new();
        if (200..300).contains(&list_resp.status) {
            if let Ok(data) = serde_json::from_str::<serde_json::Value>(&list_resp.body) {
                if let Some(arr) = data.get("value").and_then(|v| v.as_array()) {
                    for p in arr {
                        let p_fields = p.get("fields").cloned().unwrap_or(json!({}));
                        perspective_outputs.push(json!({
                            "role": p_fields.get("role").and_then(|v| v.as_str()).unwrap_or(""),
                            "lens": p_fields.get("lens").and_then(|v| v.as_str()).unwrap_or(""),
                            "output_file_id": p_fields.get("output_file_id").and_then(|v| v.as_str()).unwrap_or(""),
                        }));
                    }
                }
            }
        }

        // Create synthesizer session
        let create_resp = ctx.http_call(
            "POST",
            &format!("{api_url}/tdata/Sessions"),
            &headers,
            &json!({"fields": {}}).to_string(),
        )?;
        if !(200..300).contains(&create_resp.status) {
            return Err(format!(
                "Failed to create synthesizer Session: HTTP {}: {}",
                create_resp.status,
                &create_resp.body[..create_resp.body.len().min(300)]
            ));
        }
        let created: serde_json::Value = serde_json::from_str(&create_resp.body)
            .map_err(|e| format!("Failed to parse Session response: {e}"))?;
        let session_id = created
            .get("entity_id")
            .and_then(|v| v.as_str())
            .ok_or("Created Session has no entity_id")?
            .to_string();

        let perspectives_json =
            serde_json::to_string(&perspective_outputs).unwrap_or_else(|_| "[]".to_string());

        let user_message = format!(
            r#"You are the synthesizer agent in a consilium deliberation.

## Deliberation Intent
{intent}

## Perspective Outputs
{perspectives_json}

## Instructions

Read ALL perspective outputs from their file IDs. Each perspective was generated
independently — they never saw each other. Your job is to synthesize them into
a coherent Brief that:

1. Surfaces emergent insights that arise from combining the perspectives
2. Identifies tensions and disagreements between perspectives
3. Highlights areas of convergence across perspectives
4. Notes blind spots — what no perspective addressed
5. Produces a structured synthesis, not a summary of each perspective

Use your `synthesize-brief` skill. When done, create a Brief entity:

```
brief_content = your_synthesis_text
result = temper.write('/briefs/deliberation-{entity_id}.md', brief_content)
brief = temper.create('Briefs', {{}})
temper.action('Briefs', brief['entity_id'], 'Draft', {{
    'deliberation_id': '{entity_id}',
    'content_file_id': result['file_id'],
    'summary': one_paragraph_summary,
    'perspective_count': '{perspective_count}'
}})
temper.action('Briefs', brief['entity_id'], 'Publish', {{}})
temper.action('Deliberations', '{entity_id}', 'SynthesisComplete', {{
    'brief_id': brief['entity_id']
}})
temper.done("synthesis complete")
```
"#
        );

        let session_config = json!({
            "user_message": user_message,
            "model": synthesis_model,
            "provider": synthesis_provider,
            "temperature": "0.7",
            "tools_enabled": "temper_get,temper_list,temper_create,temper_action,temper_write,temper_read",
            "max_turns": "50",
        });
        let config_resp = ctx.http_call(
            "POST",
            &format!("{api_url}/tdata/Sessions('{session_id}')/TemperPaw.Configure"),
            &headers,
            &session_config.to_string(),
        )?;
        if !(200..300).contains(&config_resp.status) {
            return Err(format!(
                "Failed to configure synthesizer Session: HTTP {}: {}",
                config_resp.status,
                &config_resp.body[..config_resp.body.len().min(300)]
            ));
        }

        ctx.log("info", &format!(
            "check_and_synthesize: spawned synthesizer session '{session_id}' with {} perspective outputs",
            perspective_outputs.len()
        ));

        set_success_result(
            "",
            &json!({
                "status": "synthesizing",
                "session_id": session_id,
                "perspective_count": perspective_outputs.len(),
            }),
        );
        Ok(())
    })();

    if let Err(e) = result {
        set_error_result(&e);
    }
    0
}

fn urlenc(s: &str) -> String {
    s.replace('%', "%25")
        .replace(' ', "%20")
        .replace('&', "%26")
        .replace('=', "%3D")
        .replace('?', "%3F")
        .replace('#', "%23")
        .replace('\'', "%27")
}
