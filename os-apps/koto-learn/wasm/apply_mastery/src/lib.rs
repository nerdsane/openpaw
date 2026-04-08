use temper_wasm_sdk::prelude::*;

#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        ctx.log("info", "apply_mastery: starting");

        // --- Read Encounter entity fields ---
        let fields = ctx
            .entity_state
            .get("fields")
            .cloned()
            .unwrap_or(json!({}));

        let mastery_updates_raw = fields
            .get("mastery_updates")
            .ok_or("missing mastery_updates on Encounter")?;

        // mastery_updates may be stored as a JSON string or as a direct array
        let mastery_updates: Vec<serde_json::Value> =
            if let Some(arr) = mastery_updates_raw.as_array() {
                arr.clone()
            } else if let Some(s) = mastery_updates_raw.as_str() {
                serde_json::from_str(s).map_err(|e| {
                    format!("failed to parse mastery_updates string: {e}")
                })?
            } else {
                return Err("mastery_updates is neither an array nor a JSON string".into());
            };

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

        let tenant = &ctx.tenant;

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

        ctx.log("info", &format!(
            "apply_mastery: processing {} mastery updates for learner {}",
            mastery_updates.len(),
            learner_profile_id
        ));

        let mut created_count: u32 = 0;
        let mut updated_count: u32 = 0;

        for update in &mastery_updates {
            let concept_id = update
                .get("concept_id")
                .and_then(|v| v.as_str())
                .ok_or("mastery update missing concept_id")?;

            let dimensions = update
                .get("dimensions")
                .cloned()
                .unwrap_or(json!({}));
            let confidence = update
                .get("confidence")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            let should_advance = update
                .get("should_advance")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let should_regress = update
                .get("should_regress")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            // --- Query existing Mastery for this concept + learner ---
            let query_url = format!(
                "{temper_api_url}/tdata/Masteries?$filter=learner_profile_id eq '{}' and concept_id eq '{}'",
                learner_profile_id, concept_id
            );
            let query_resp =
                ctx.http_call("GET", &query_url, &odata_headers, "")?;
            if query_resp.status < 200 || query_resp.status >= 300 {
                return Err(format!("HTTP {}: {}", query_resp.status, &query_resp.body[..query_resp.body.len().min(300)]));
            }
            let query_data: serde_json::Value =
                serde_json::from_str(&query_resp.body).map_err(|e| {
                    format!("failed to parse Mastery query for {concept_id}: {e}")
                })?;

            let existing = query_data
                .get("value")
                .and_then(|v| v.as_array())
                .and_then(|arr| arr.first().cloned());

            let mastery_entity_id = match existing {
                Some(ref entity) => {
                    // Mastery already exists — extract its ID
                    entity
                        .get("entity_id")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| {
                            format!("existing Mastery for {concept_id} has no entity_id")
                        })?
                        .to_string()
                }
                None => {
                    // No Mastery exists — create one via Discover action
                    ctx.log("info", &format!(
                        "apply_mastery: creating new Mastery for concept {concept_id}"
                    ));

                    let create_url =
                        format!("{temper_api_url}/tdata/Masteries");
                    let create_body = json!({
                        "fields": {
                            "learner_profile_id": learner_profile_id,
                            "concept_id": concept_id,
                            "recognition": 0.0,
                            "production": 0.0,
                            "contextual_use": 0.0,
                            "transfer": 0.0,
                            "confidence": 0.0,
                        }
                    });
                    let create_resp = ctx.http_call(
                        "POST",
                        &create_url,
                        &odata_headers,
                        &create_body.to_string(),
                    )?;
                    if create_resp.status < 200 || create_resp.status >= 300 {
                        return Err(format!("HTTP {}: {}", create_resp.status, &create_resp.body[..create_resp.body.len().min(300)]));
                    }
                    let created: serde_json::Value =
                        serde_json::from_str(&create_resp.body).map_err(|e| {
                            format!("failed to parse Mastery creation response: {e}")
                        })?;

                    let new_id = created
                        .get("entity_id")
                        .and_then(|v| v.as_str())
                        .ok_or("created Mastery has no entity_id")?
                        .to_string();

                    // Dispatch Discover action to move into discovered state
                    let discover_url = format!(
                        "{temper_api_url}/tdata/EntitySets('{new_id}')/Temper.Discover"
                    );
                    let discover_resp = ctx.http_call(
                        "POST",
                        &discover_url,
                        &odata_headers,
                        &json!({}).to_string(),
                    )?;
                    if discover_resp.status < 200 || discover_resp.status >= 300 {
                        return Err(format!("HTTP {}: {}", discover_resp.status, &discover_resp.body[..discover_resp.body.len().min(300)]));
                    }

                    created_count += 1;
                    new_id
                }
            };

            // --- Dispatch Assess action with updated dimensions ---
            ctx.log("info", &format!(
                "apply_mastery: assessing mastery {mastery_entity_id} for concept {concept_id}"
            ));

            let assess_url = format!(
                "{temper_api_url}/tdata/EntitySets('{mastery_entity_id}')/Temper.Assess"
            );
            let assess_body = json!({
                "recognition": dimensions.get("recognition").and_then(|v| v.as_f64()).unwrap_or(0.0),
                "production": dimensions.get("production").and_then(|v| v.as_f64()).unwrap_or(0.0),
                "contextual_use": dimensions.get("contextual_use").and_then(|v| v.as_f64()).unwrap_or(0.0),
                "transfer": dimensions.get("transfer").and_then(|v| v.as_f64()).unwrap_or(0.0),
                "confidence": confidence,
            });
            let assess_resp = ctx.http_call(
                "POST",
                &assess_url,
                &odata_headers,
                &assess_body.to_string(),
            )?;
            if assess_resp.status < 200 || assess_resp.status >= 300 {
                return Err(format!("HTTP {}: {}", assess_resp.status, &assess_resp.body[..assess_resp.body.len().min(300)]));
            }

            // --- Dispatch Advance or Regress if indicated ---
            if should_advance {
                ctx.log("info", &format!(
                    "apply_mastery: advancing mastery for concept {concept_id}"
                ));
                let advance_url = format!(
                    "{temper_api_url}/tdata/EntitySets('{mastery_entity_id}')/Temper.Advance"
                );
                let advance_resp = ctx.http_call(
                    "POST",
                    &advance_url,
                    &odata_headers,
                    &json!({}).to_string(),
                )?;
                if advance_resp.status < 200 || advance_resp.status >= 300 {
                    return Err(format!("HTTP {}: {}", advance_resp.status, &advance_resp.body[..advance_resp.body.len().min(300)]));
                }
            } else if should_regress {
                ctx.log("info", &format!(
                    "apply_mastery: regressing mastery for concept {concept_id}"
                ));
                let regress_url = format!(
                    "{temper_api_url}/tdata/EntitySets('{mastery_entity_id}')/Temper.Regress"
                );
                let regress_resp = ctx.http_call(
                    "POST",
                    &regress_url,
                    &odata_headers,
                    &json!({}).to_string(),
                )?;
                if regress_resp.status < 200 || regress_resp.status >= 300 {
                    return Err(format!("HTTP {}: {}", regress_resp.status, &regress_resp.body[..regress_resp.body.len().min(300)]));
                }
            }

            updated_count += 1;
        }

        // --- Dispatch RecordEncounter on the LearnerProfile ---
        ctx.log("info", &format!(
            "apply_mastery: dispatching RecordEncounter on LearnerProfile {learner_profile_id}"
        ));

        let record_url = format!(
            "{temper_api_url}/tdata/EntitySets('{learner_profile_id}')/Temper.RecordEncounter"
        );
        let record_resp = ctx.http_call(
            "POST",
            &record_url,
            &odata_headers,
            &json!({}).to_string(),
        )?;
        if record_resp.status < 200 || record_resp.status >= 300 {
            return Err(format!("HTTP {}: {}", record_resp.status, &record_resp.body[..record_resp.body.len().min(300)]));
        }

        ctx.log("info", &format!(
            "apply_mastery: completed — created {created_count}, updated {updated_count}"
        ));

        set_success_result(
            "",
            &json!({
                "status": "ok",
                "created": created_count,
                "updated": updated_count,
            }),
        );
        Ok(())
    })();

    if let Err(e) = result {
        set_error_result(&e);
    }
    0
}
