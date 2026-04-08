use temper_wasm_sdk::prelude::*;

#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        ctx.log("info", "evaluate_mastery: starting");

        // --- Read Encounter entity fields ---
        let fields = ctx
            .entity_state
            .get("fields")
            .cloned()
            .unwrap_or(json!({}));

        let concept_ids = fields
            .get("concept_ids")
            .cloned()
            .unwrap_or(json!([]));
        let concept_ids_arr = concept_ids
            .as_array()
            .ok_or("concept_ids must be a JSON array")?;

        let student_response = fields
            .get("student_response")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let content = fields
            .get("content")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let encounter_context = fields
            .get("context")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let encounter_type = fields
            .get("encounter_type")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        let learner_profile_id = fields
            .get("learner_profile_id")
            .and_then(|v| v.as_str())
            .ok_or("missing learner_profile_id on Encounter")?
            .to_string();

        // --- Config ---
        let temper_api_url = ctx
            .config
            .get("temper_api_url")
            .filter(|s| !s.is_empty() && !s.contains("{secret:"))
            .cloned()
            .unwrap_or_else(|| "http://127.0.0.1:3000".to_string());

        let anthropic_api_key = ctx
            .config
            .get("anthropic_api_key")
            .filter(|s| !s.is_empty() && !s.contains("{secret:"))
            .cloned()
            .ok_or("anthropic_api_key not configured")?;

        let tenant = &ctx.tenant;
        let entity_id = ctx
            .entity_state
            .get("entity_id")
            .and_then(|v| v.as_str())
            .unwrap_or(&ctx.entity_id);

        let odata_headers = vec![
            ("content-type".to_string(), "application/json".to_string()),
            ("x-tenant-id".to_string(), tenant.to_string()),
            (
                "x-temper-principal-kind".to_string(),
                "agent".to_string(),
            ),
            (
                "x-temper-principal-id".to_string(),
                ctx.entity_id.clone(),
            ),
            ("x-temper-agent-type".to_string(), "system".to_string()),
        ];

        // --- Fetch LearnerProfile ---
        ctx.log("info", &format!(
            "evaluate_mastery: fetching LearnerProfile {learner_profile_id}"
        ));

        let profile_url = format!(
            "{temper_api_url}/tdata/LearnerProfiles('{learner_profile_id}')"
        );
        let profile_resp =
            ctx.http_call("GET", &profile_url, &odata_headers, "")?;
        if profile_resp.status < 200 || profile_resp.status >= 300 {
            return Err(format!("HTTP {}: {}", profile_resp.status, &profile_resp.body[..profile_resp.body.len().min(300)]));
        }

        let profile: serde_json::Value = serde_json::from_str(&profile_resp.body)
            .map_err(|e| format!("failed to parse LearnerProfile response: {e}"))?;

        let profile_fields = profile
            .get("fields")
            .cloned()
            .unwrap_or(json!({}));
        let domain = profile_fields
            .get("domain")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let learning_style = profile_fields
            .get("learning_style")
            .and_then(|v| v.as_str())
            .unwrap_or("balanced");

        // --- Query existing Mastery entities for each concept ---
        let mut current_masteries: Vec<serde_json::Value> = Vec::new();

        for concept_id_val in concept_ids_arr {
            let concept_id = concept_id_val
                .as_str()
                .ok_or("each concept_id must be a string")?;

            let mastery_url = format!(
                "{temper_api_url}/tdata/Masteries?$filter=learner_profile_id eq '{}' and concept_id eq '{}'",
                learner_profile_id, concept_id
            );
            ctx.log("info", &format!(
                "evaluate_mastery: querying mastery for concept {concept_id}"
            ));

            let mastery_resp =
                ctx.http_call("GET", &mastery_url, &odata_headers, "")?;
            if mastery_resp.status < 200 || mastery_resp.status >= 300 {
                return Err(format!("HTTP {}: {}", mastery_resp.status, &mastery_resp.body[..mastery_resp.body.len().min(300)]));
            }

            let mastery_data: serde_json::Value =
                serde_json::from_str(&mastery_resp.body).map_err(|e| {
                    format!("failed to parse Mastery query response: {e}")
                })?;

            let items = mastery_data
                .get("value")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();

            if let Some(mastery) = items.into_iter().next() {
                current_masteries.push(json!({
                    "concept_id": concept_id,
                    "mastery": mastery.get("fields").cloned().unwrap_or(json!({})),
                    "state": mastery.get("state").cloned().unwrap_or(json!("unknown")),
                }));
            } else {
                current_masteries.push(json!({
                    "concept_id": concept_id,
                    "mastery": null,
                    "state": "none",
                }));
            }
        }

        // --- Call Anthropic API for evaluation ---
        ctx.log("info", "evaluate_mastery: calling Anthropic API");

        let system_prompt = "You are an expert language assessment evaluator. \
            Assess this student's response across 4 dimensions: \
            recognition (0.0-1.0), production (0.0-1.0), contextual_use (0.0-1.0), \
            transfer (0.0-1.0). Also suggest mastery state transitions \
            (advance/regress/maintain for each concept). \
            Respond with ONLY valid JSON, no markdown fences. Use this schema:\n\
            {\n  \"evaluations\": [\n    {\n      \"concept_id\": \"...\",\n      \
            \"dimensions\": { \"recognition\": 0.0, \"production\": 0.0, \
            \"contextual_use\": 0.0, \"transfer\": 0.0 },\n      \
            \"confidence\": 0.0,\n      \"transition\": \"advance|regress|maintain\",\n      \
            \"reasoning\": \"...\"\n    }\n  ]\n}";

        let user_message = format!(
            "Domain: {domain}\nLearning style: {learning_style}\n\
             Encounter type: {encounter_type}\n\n\
             Content:\n{content}\n\n\
             Context:\n{encounter_context}\n\n\
             Student response:\n{student_response}\n\n\
             Current mastery state:\n{current_masteries}",
            current_masteries =
                serde_json::to_string_pretty(&current_masteries)
                    .unwrap_or_else(|_| "[]".to_string())
        );

        let anthropic_body = json!({
            "model": "claude-sonnet-4-20250514",
            "max_tokens": 2048,
            "system": system_prompt,
            "messages": [
                { "role": "user", "content": user_message }
            ]
        });

        let anthropic_headers = vec![
            ("content-type".to_string(), "application/json".to_string()),
            ("x-api-key".to_string(), anthropic_api_key),
            (
                "anthropic-version".to_string(),
                "2023-06-01".to_string(),
            ),
        ];

        let llm_resp = ctx.http_call(
            "POST",
            "https://api.anthropic.com/v1/messages",
            &anthropic_headers,
            &anthropic_body.to_string(),
        )?;
        if llm_resp.status < 200 || llm_resp.status >= 300 {
            return Err(format!("HTTP {}: {}", llm_resp.status, &llm_resp.body[..llm_resp.body.len().min(300)]));
        }

        let llm_data: serde_json::Value = serde_json::from_str(&llm_resp.body)
            .map_err(|e| format!("failed to parse Anthropic response: {e}"))?;

        // Extract text content from the Anthropic response
        let raw_text = llm_data
            .get("content")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .and_then(|block| block.get("text"))
            .and_then(|t| t.as_str())
            .ok_or("no text content in Anthropic response")?;

        let evaluation: serde_json::Value =
            serde_json::from_str(raw_text).map_err(|e| {
                format!("failed to parse LLM evaluation JSON: {e}")
            })?;

        // --- Build mastery_updates from evaluation ---
        let evaluations = evaluation
            .get("evaluations")
            .and_then(|v| v.as_array())
            .ok_or("LLM response missing 'evaluations' array")?;

        let mastery_updates: Vec<serde_json::Value> = evaluations
            .iter()
            .map(|eval| {
                let transition = eval
                    .get("transition")
                    .and_then(|v| v.as_str())
                    .unwrap_or("maintain");
                json!({
                    "concept_id": eval.get("concept_id").cloned().unwrap_or(json!(null)),
                    "dimensions": eval.get("dimensions").cloned().unwrap_or(json!({})),
                    "confidence": eval.get("confidence").cloned().unwrap_or(json!(0.0)),
                    "should_advance": transition == "advance",
                    "should_regress": transition == "regress",
                    "reasoning": eval.get("reasoning").cloned().unwrap_or(json!("")),
                })
            })
            .collect();

        // --- Dispatch RecordEvaluation on the Encounter ---
        ctx.log("info", "evaluate_mastery: dispatching RecordEvaluation");

        let action_url = format!(
            "{temper_api_url}/tdata/EntitySets('{entity_id}')/Temper.RecordEvaluation"
        );
        let action_body = json!({
            "evaluation": evaluation,
            "mastery_updates": mastery_updates,
        });

        let action_resp = ctx.http_call(
            "POST",
            &action_url,
            &odata_headers,
            &action_body.to_string(),
        )?;
        if action_resp.status < 200 || action_resp.status >= 300 {
            return Err(format!("HTTP {}: {}", action_resp.status, &action_resp.body[..action_resp.body.len().min(300)]));
        }

        ctx.log("info", &format!(
            "evaluate_mastery: completed — evaluated {} concepts",
            mastery_updates.len()
        ));

        set_success_result(
            "",
            &json!({
                "status": "ok",
                "concepts_evaluated": mastery_updates.len(),
            }),
        );
        Ok(())
    })();

    if let Err(e) = result {
        set_error_result(&e);
    }
    0
}
