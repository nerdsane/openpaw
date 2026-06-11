//! grade_hindcast — score a retrodiction run against recorded reality
//! (ADR-002).
//!
//! On Hindcast.Grade: read the recorded actuals from paw-fs, match each
//! actual to one of the hindcast world's preregistered Forecasts (by
//! event_node_id when given, else by case-insensitive substring on the
//! question), resolve and Brier-score the matches, and dispatch
//! Hindcast.ScoreComplete with the mean Brier and an honest coverage note.
//!
//! Honest limitations, recorded in the coverage note on every run: the
//! anachronism check is not yet implemented, and residual model-prior
//! contamination applies whenever the vantage predates the model's training
//! cutoff.
//!
//! Build: `cargo build --target wasm32-unknown-unknown --release`

use temper_wasm_sdk::prelude::*;

// --- Pure grading logic -----------------------------------------------------

/// One recorded ground-truth outcome from the actuals file.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Actual {
    event_node_id: String,
    question_contains: String,
    outcome: String, // "yes" | "no"
}

/// One of the world's forecasts, reduced to what grading needs.
#[derive(Debug, Clone, PartialEq)]
struct ForecastRow {
    id: String,
    event_node_id: String,
    question: String,
    probability: String,
    status: String,
}

/// Parse the actuals file: a JSON array of objects with an optional
/// "event_node_id", an optional "question_contains", and a required
/// "outcome" of "yes" or "no". Entries without a valid outcome are dropped.
fn parse_actuals(body: &str) -> Result<Vec<Actual>, String> {
    let parsed: Value = serde_json::from_str(body)
        .map_err(|e| format!("actuals file is not valid JSON: {e}"))?;
    let arr = parsed
        .as_array()
        .ok_or("actuals file must be a JSON array")?;
    Ok(arr
        .iter()
        .filter_map(|entry| {
            let str_of = |k: &str| {
                entry
                    .get(k)
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string()
            };
            let outcome = str_of("outcome");
            if outcome != "yes" && outcome != "no" {
                return None;
            }
            Some(Actual {
                event_node_id: str_of("event_node_id"),
                question_contains: str_of("question_contains"),
                outcome,
            })
        })
        .collect())
}

/// Match an actual to a forecast: exact event_node_id when given, otherwise
/// case-insensitive substring on the registered question.
fn match_forecast<'a>(actual: &Actual, forecasts: &'a [ForecastRow]) -> Option<&'a ForecastRow> {
    if !actual.event_node_id.is_empty() {
        return forecasts
            .iter()
            .find(|f| f.event_node_id == actual.event_node_id);
    }
    if actual.question_contains.is_empty() {
        return None;
    }
    let needle = actual.question_contains.to_lowercase();
    forecasts
        .iter()
        .find(|f| f.question.to_lowercase().contains(&needle))
}

/// Brier score for a registered probability against a yes/no outcome
/// (yes = 1.0, no = 0.0).
fn brier(probability: f64, outcome_yes: bool) -> f64 {
    let outcome = if outcome_yes { 1.0 } else { 0.0 };
    (probability - outcome).powi(2)
}

fn mean(xs: &[f64]) -> Option<f64> {
    if xs.is_empty() {
        None
    } else {
        Some(xs.iter().sum::<f64>() / xs.len() as f64)
    }
}

/// The ScoreComplete params, honest about coverage. graded 0/n is a valid
/// score report, not an error — silence would hide the gap.
fn score_complete_params(briers: &[f64], total_forecasts: usize, actuals_file_id: &str) -> Value {
    let mean_brier = mean(briers)
        .map(|m| format!("{m:.4}"))
        .unwrap_or_default();
    json!({
        "mean_brier": mean_brier,
        "graded_count": briers.len().to_string(),
        "actuals_file_id": actuals_file_id,
        "coverage_note": format!(
            "graded {}/{} forecasts; anachronism check not yet implemented; residual \
             model-prior contamination applies (vantage vs training cutoff)",
            briers.len(),
            total_forecasts
        ),
    })
}

// --- Entry point -------------------------------------------------------------

/// Entry point.
#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        grade(&ctx)
    })();

    if let Err(e) = result {
        set_error_result(&e);
    }
    0
}

fn grade(ctx: &Context) -> Result<(), String> {
    let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));
    let get = |k: &str| -> String {
        fields
            .get(k)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    };

    let world_id = get("world_id");
    if world_id.is_empty() {
        return Err("Hindcast.world_id is required for grading".to_string());
    }
    let actuals_file_id = get("actuals_file_id");
    if actuals_file_id.is_empty() {
        return Err("Hindcast.actuals_file_id is required: nothing to grade against".to_string());
    }

    let api = ctx
        .config
        .get("temper_api_url")
        .filter(|s| !s.is_empty() && !s.contains("{secret:"))
        .cloned()
        .unwrap_or_else(|| "http://127.0.0.1:3000".to_string());
    let headers = vec![
        ("content-type".to_string(), "application/json".to_string()),
        ("x-tenant-id".to_string(), ctx.tenant.clone()),
        ("x-temper-principal-kind".to_string(), "agent".to_string()),
        ("x-temper-principal-id".to_string(), ctx.entity_id.clone()),
        ("x-temper-agent-type".to_string(), "system".to_string()),
    ];

    // 1. Read the recorded actuals.
    let actuals_url = format!("{api}/tdata/Files('{actuals_file_id}')/$value");
    let actuals_resp = ctx.http_call("GET", &actuals_url, &headers, "")?;
    if actuals_resp.status < 200 || actuals_resp.status >= 300 {
        return Err(format!(
            "failed to read actuals file {actuals_file_id} (HTTP {})",
            actuals_resp.status
        ));
    }
    let actuals = parse_actuals(&actuals_resp.body)?;

    // 2. The hindcast world's forecasts.
    let forecasts_url = format!("{api}/tdata/Forecasts?$filter=world_id eq '{world_id}'");
    let fresp = ctx.http_call("GET", &forecasts_url, &headers, "")?;
    if fresp.status < 200 || fresp.status >= 300 {
        return Err(format!("failed to list Forecasts (HTTP {})", fresp.status));
    }
    let fbody: Value = serde_json::from_str(&fresp.body).unwrap_or(json!({}));
    let forecasts: Vec<ForecastRow> = fbody
        .get("value")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|f| {
            let str_of = |k: &str| {
                f.get(k)
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string()
            };
            let id = f
                .get("Id")
                .or_else(|| f.get("entity_id"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if id.is_empty() {
                return None;
            }
            Some(ForecastRow {
                id,
                event_node_id: str_of("EventNodeId"),
                question: str_of("Question"),
                probability: str_of("Probability"),
                status: str_of("Status"),
            })
        })
        .collect();

    // 3. Match, resolve, score.
    let outcome_refs = json!([format!("file:{actuals_file_id}")]).to_string();
    let mut briers: Vec<f64> = Vec::new();
    let mut graded_ids: Vec<String> = Vec::new();
    for actual in &actuals {
        let Some(forecast) = match_forecast(actual, &forecasts) else {
            ctx.log(
                "info",
                &format!(
                    "grade_hindcast: actual (node={:?}, contains={:?}) matched no forecast",
                    actual.event_node_id, actual.question_contains
                ),
            );
            continue;
        };
        if graded_ids.contains(&forecast.id) {
            ctx.log(
                "info",
                &format!(
                    "grade_hindcast: forecast {} already graded this run; skipping duplicate actual",
                    forecast.id
                ),
            );
            continue;
        }
        if forecast.status != "Preregistered" {
            ctx.log(
                "info",
                &format!(
                    "grade_hindcast: forecast {} is {} (not Preregistered); skipping",
                    forecast.id, forecast.status
                ),
            );
            continue;
        }
        let Ok(probability) = forecast.probability.trim().parse::<f64>() else {
            ctx.log(
                "warn",
                &format!(
                    "grade_hindcast: forecast {} has unparseable probability {:?}; cannot grade",
                    forecast.id, forecast.probability
                ),
            );
            continue;
        };

        let resolve_url = format!("{api}/tdata/Forecasts('{}')/TemperPaw.Resolve", forecast.id);
        let resolve_body = json!({
            "outcome": actual.outcome,
            "outcome_source_refs": outcome_refs,
        });
        match ctx.http_call("POST", &resolve_url, &headers, &resolve_body.to_string()) {
            Ok(r) if (200..300).contains(&r.status) => {}
            Ok(r) => {
                ctx.log(
                    "warn",
                    &format!(
                        "grade_hindcast: Forecast.Resolve on {} failed (HTTP {})",
                        forecast.id, r.status
                    ),
                );
                continue;
            }
            Err(e) => {
                ctx.log(
                    "warn",
                    &format!("grade_hindcast: Forecast.Resolve on {} failed: {e}", forecast.id),
                );
                continue;
            }
        }

        let score = brier(probability, actual.outcome == "yes");
        let score_url = format!("{api}/tdata/Forecasts('{}')/TemperPaw.Score", forecast.id);
        match ctx.http_call(
            "POST",
            &score_url,
            &headers,
            &json!({ "brier": format!("{score:.4}") }).to_string(),
        ) {
            Ok(r) if (200..300).contains(&r.status) => {
                ctx.log(
                    "info",
                    &format!(
                        "grade_hindcast: forecast {} graded — outcome {}, brier {score:.4}",
                        forecast.id, actual.outcome
                    ),
                );
                briers.push(score);
                graded_ids.push(forecast.id.clone());
            }
            Ok(r) => ctx.log(
                "warn",
                &format!(
                    "grade_hindcast: Forecast.Score on {} failed (HTTP {})",
                    forecast.id, r.status
                ),
            ),
            Err(e) => ctx.log(
                "warn",
                &format!("grade_hindcast: Forecast.Score on {} failed: {e}", forecast.id),
            ),
        }
    }

    // 4. Report the score, honest about coverage.
    let params = score_complete_params(&briers, forecasts.len(), &actuals_file_id);
    ctx.log(
        "info",
        &format!(
            "grade_hindcast: hindcast {} — graded {}/{} forecasts from {} actual(s)",
            ctx.entity_id,
            briers.len(),
            forecasts.len(),
            actuals.len()
        ),
    );
    set_success_result("ScoreComplete", &params);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn forecast(id: &str, node: &str, question: &str, p: &str, status: &str) -> ForecastRow {
        ForecastRow {
            id: id.to_string(),
            event_node_id: node.to_string(),
            question: question.to_string(),
            probability: p.to_string(),
            status: status.to_string(),
        }
    }

    fn actual(node: &str, contains: &str, outcome: &str) -> Actual {
        Actual {
            event_node_id: node.to_string(),
            question_contains: contains.to_string(),
            outcome: outcome.to_string(),
        }
    }

    #[test]
    fn matching_prefers_the_event_node_id() {
        let forecasts = vec![
            forecast("f-1", "n-1", "Will rates fall?", "0.6", "Preregistered"),
            forecast("f-2", "n-2", "Will rates rise?", "0.4", "Preregistered"),
        ];
        let m = match_forecast(&actual("n-2", "", "yes"), &forecasts).unwrap();
        assert_eq!(m.id, "f-2");
        // An id match is exact even when a substring would hit another row.
        let m = match_forecast(&actual("n-1", "rates rise", "yes"), &forecasts).unwrap();
        assert_eq!(m.id, "f-1");
    }

    #[test]
    fn matching_falls_back_to_case_insensitive_substring() {
        let forecasts = vec![
            forecast("f-1", "n-1", "Will GPT-6 ship before July?", "0.7", "Preregistered"),
            forecast("f-2", "n-2", "Will rates rise?", "0.4", "Preregistered"),
        ];
        let m = match_forecast(&actual("", "gpt-6 SHIP", "yes"), &forecasts).unwrap();
        assert_eq!(m.id, "f-1");
    }

    #[test]
    fn unmatched_actuals_match_nothing() {
        let forecasts = vec![forecast("f-1", "n-1", "Will rates fall?", "0.6", "Preregistered")];
        assert!(match_forecast(&actual("n-9", "", "yes"), &forecasts).is_none());
        assert!(match_forecast(&actual("", "quantum", "no"), &forecasts).is_none());
        // Neither key given: nothing to match on.
        assert!(match_forecast(&actual("", "", "yes"), &forecasts).is_none());
    }

    #[test]
    fn brier_and_mean_compute_squared_error() {
        assert_eq!(brier(0.7, true), (0.7f64 - 1.0).powi(2));
        assert_eq!(brier(0.7, false), 0.7f64.powi(2));
        let m = mean(&[0.09, 0.49]).unwrap();
        assert!((m - 0.29).abs() < 1e-12);
        assert_eq!(mean(&[]), None);
    }

    #[test]
    fn empty_actuals_still_report_honestly() {
        assert_eq!(parse_actuals("[]").unwrap(), Vec::<Actual>::new());
        let params = score_complete_params(&[], 7, "file-9");
        assert_eq!(params["graded_count"], "0");
        assert_eq!(params["mean_brier"], "");
        assert_eq!(params["actuals_file_id"], "file-9");
        let note = params["coverage_note"].as_str().unwrap();
        assert!(note.contains("graded 0/7 forecasts"));
        assert!(note.contains("anachronism check not yet implemented"));
        assert!(note.contains("model-prior contamination"));
    }

    #[test]
    fn actuals_parsing_is_strict_about_shape_and_outcomes() {
        assert!(parse_actuals("not json").is_err());
        assert!(parse_actuals("{\"outcome\": \"yes\"}").is_err()); // not an array
        // Invalid outcomes are dropped, valid ones kept.
        let parsed = parse_actuals(
            r#"[
                {"event_node_id": "n-1", "outcome": "yes"},
                {"question_contains": "rates", "outcome": "maybe"},
                {"question_contains": "rates", "outcome": "no"}
            ]"#,
        )
        .unwrap();
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0], actual("n-1", "", "yes"));
        assert_eq!(parsed[1], actual("", "rates", "no"));
    }

    #[test]
    fn score_complete_params_carry_the_four_decimal_mean() {
        let params = score_complete_params(&[0.25, 0.0625], 4, "file-1");
        assert_eq!(params["mean_brier"], "0.1562"); // (0.25+0.0625)/2 = 0.15625
        assert_eq!(params["graded_count"], "2");
        assert!(
            params["coverage_note"]
                .as_str()
                .unwrap()
                .contains("graded 2/4 forecasts")
        );
    }
}
