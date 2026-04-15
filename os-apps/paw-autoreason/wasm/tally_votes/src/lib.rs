use temper_wasm_sdk::prelude::*;

/// Pure math: decodes randomized labels, computes Borda count
/// (1st=2pts, 2nd=1, 3rd=0), conservative tiebreak (incumbent wins ties),
/// checks convergence (k consecutive A wins -> Converge).
/// Advances to next round or terminates. No LLM needed.
#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        ctx.log("info", "tally_votes: starting");

        let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));
        let counters = ctx.entity_state.get("counters").cloned().unwrap_or(json!({}));

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

        let consecutive_incumbent_wins: usize = fields
            .get("consecutive_incumbent_wins")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        let convergence_k: usize = fields
            .get("convergence_k")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse().ok())
            .unwrap_or(2);

        let max_rounds: usize = fields
            .get("max_rounds")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse().ok())
            .unwrap_or(10);

        let round_count = counters
            .get("round_count")
            .and_then(|v| v.as_i64())
            .unwrap_or(1) as usize;

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

        // Query all submitted judgments for this tournament's current round
        let filter = format!(
            "TournamentId%20eq%20'{}'%20and%20State%20eq%20'Submitted'",
            urlenc(&entity_id)
        );
        let list_resp = ctx.http_call(
            "GET",
            &format!("{api_url}/tdata/Judgments?$filter={filter}&$orderby=CreatedAt%20desc"),
            &headers,
            "",
        )?;

        let judgments: Vec<serde_json::Value> = if (200..300).contains(&list_resp.status) {
            serde_json::from_str::<serde_json::Value>(&list_resp.body)
                .ok()
                .and_then(|v| v.get("value").and_then(|v| v.as_array()).cloned())
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        if judgments.is_empty() {
            return Err("tally_votes: no submitted judgments found".to_string());
        }

        // --- Borda count: 1st=2pts, 2nd=1pt, 3rd=0pts ---
        let mut scores: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        scores.insert("A".to_string(), 0);
        scores.insert("B".to_string(), 0);
        scores.insert("AB".to_string(), 0);

        let mut label_mappings: Vec<serde_json::Value> = Vec::new();

        for judgment in &judgments {
            let j_fields = judgment.get("fields").cloned().unwrap_or(json!({}));
            let ranking_raw = j_fields.get("ranking_json").and_then(|v| v.as_str()).unwrap_or("[]");
            let labels_raw = j_fields.get("randomized_labels_json").and_then(|v| v.as_str()).unwrap_or("{}");

            let ranking: Vec<String> = serde_json::from_str(ranking_raw).unwrap_or_default();
            let label_map: std::collections::HashMap<String, String> =
                serde_json::from_str(labels_raw).unwrap_or_default();

            label_mappings.push(json!(label_map));

            // Decode randomized labels to real labels and assign points
            let points = [2usize, 1, 0]; // 1st, 2nd, 3rd
            for (i, randomized_label) in ranking.iter().enumerate() {
                if let Some(real_label) = label_map.get(randomized_label) {
                    if let (Some(score), Some(&pts)) = (scores.get_mut(real_label), points.get(i)) {
                        *score += pts;
                    }
                }
            }
        }

        // --- Determine winner (conservative tiebreak: A wins ties) ---
        let score_a = *scores.get("A").unwrap_or(&0);
        let score_b = *scores.get("B").unwrap_or(&0);
        let score_ab = *scores.get("AB").unwrap_or(&0);

        let winner = if score_a >= score_b && score_a >= score_ab {
            "A"
        } else if score_ab >= score_a && score_ab >= score_b {
            "AB"
        } else {
            "B"
        };

        let borda_scores = json!({
            "A": score_a,
            "B": score_b,
            "AB": score_ab,
        });

        ctx.log("info", &format!(
            "tally_votes: A={score_a}, B={score_b}, AB={score_ab} -> winner={winner}"
        ));

        // --- Update consecutive incumbent wins ---
        let new_consecutive = if winner == "A" {
            consecutive_incumbent_wins + 1
        } else {
            0
        };

        // --- Determine next state ---
        // Find the winning version's file_id
        // For A: use current version A's file
        // For B or AB: query the Round for the version file IDs
        let current_round_filter = format!(
            "TournamentId%20eq%20'{}'%20and%20State%20eq%20'Active'",
            urlenc(&entity_id)
        );
        let round_resp = ctx.http_call(
            "GET",
            &format!("{api_url}/tdata/Rounds?$filter={current_round_filter}&$orderby=CreatedAt%20desc&$top=1"),
            &headers,
            "",
        )?;

        let round_data = serde_json::from_str::<serde_json::Value>(&round_resp.body)
            .ok()
            .and_then(|v| v.get("value").and_then(|v| v.as_array()).cloned())
            .and_then(|arr| arr.into_iter().next());

        let round_id = round_data.as_ref()
            .and_then(|r| r.get("entity_id").or_else(|| r.get("Id")))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let round_fields = round_data.as_ref()
            .and_then(|r| r.get("fields"))
            .cloned()
            .unwrap_or(json!({}));

        let winning_file_id = match winner {
            "A" => round_fields.get("version_a_file_id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            "B" => round_fields.get("version_b_file_id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            "AB" => round_fields.get("version_ab_file_id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            _ => String::new(),
        };

        // Record result on the round
        if !round_id.is_empty() {
            let _ = ctx.http_call(
                "POST",
                &format!("{api_url}/tdata/Rounds('{round_id}')/{odata_namespace}.RecordResult"),
                &headers,
                &json!({
                    "winner": winner,
                    "borda_scores_json": borda_scores.to_string(),
                    "label_mapping_json": serde_json::to_string(&label_mappings).unwrap_or_else(|_| "[]".to_string()),
                }).to_string(),
            );
        }

        // Check convergence or max rounds
        if new_consecutive >= convergence_k {
            // Converged!
            let resp = ctx.http_call(
                "POST",
                &format!("{api_url}/tdata/Tournaments('{entity_id}')/{odata_namespace}.Converge"),
                &headers,
                &json!({"final_version_file_id": winning_file_id}).to_string(),
            )?;
            if !(200..300).contains(&resp.status) {
                return Err(format!("Failed to Converge: HTTP {}", resp.status));
            }
            ctx.log("info", &format!("tally_votes: CONVERGED after {round_count} rounds"));
        } else if round_count >= max_rounds {
            // Max rounds reached
            let resp = ctx.http_call(
                "POST",
                &format!("{api_url}/tdata/Tournaments('{entity_id}')/{odata_namespace}.ReachMaxRounds"),
                &headers,
                &json!({"final_version_file_id": winning_file_id}).to_string(),
            )?;
            if !(200..300).contains(&resp.status) {
                return Err(format!("Failed to ReachMaxRounds: HTTP {}", resp.status));
            }
            ctx.log("info", &format!("tally_votes: MAX ROUNDS after {round_count} rounds"));
        } else {
            // Next round — winner becomes the new version A
            let resp = ctx.http_call(
                "POST",
                &format!("{api_url}/tdata/Tournaments('{entity_id}')/{odata_namespace}.NextRound"),
                &headers,
                &json!({
                    "current_version_a_id": current_version_a_id,
                    "consecutive_incumbent_wins": new_consecutive.to_string(),
                }).to_string(),
            )?;
            if !(200..300).contains(&resp.status) {
                return Err(format!("Failed to NextRound: HTTP {}", resp.status));
            }
            ctx.log("info", &format!("tally_votes: round {round_count} -> winner={winner}, next round"));
        }

        set_success_result(
            "",
            &json!({
                "status": "ok",
                "winner": winner,
                "borda_scores": borda_scores,
                "consecutive_incumbent_wins": new_consecutive,
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

fn urlenc(s: &str) -> String {
    s.replace('%', "%25")
        .replace(' ', "%20")
        .replace('&', "%26")
        .replace('=', "%3D")
        .replace('?', "%3F")
        .replace('#', "%23")
        .replace('\'', "%27")
}
