//! aggregate_costs — deterministic costing and classification for corridor
//! paths (ADR-002). Sessions flag costs; this module computes them.
//!
//! Two triggers, distinguished by the path's status when the module runs:
//! - Path.ChallengeComplete (status Challenged): compute repair_cost from the
//!   union of repairer and adversary flags, dispatch Path.Score.
//! - Path.Score (status Scored): if every path in this world's pass is
//!   settled, classify all scored paths (Canonical / Tail / Reject), derive
//!   endpoint weights, and dispatch World.PathsScored.
//!
//! Build: `cargo build --target wasm32-unknown-unknown --release`

use temper_wasm_sdk::prelude::*;

/// Base cost per flag kind. A miracle (unexplained discontinuity) costs the
/// most; a compressed lag the least. Unknown kinds cost like contradictions
/// so malformed flags are never free.
fn kind_weight(kind: &str) -> f64 {
    match kind {
        "miracle" => 40.0,
        "contradiction" => 25.0,
        "incentive" => 15.0,
        "lag" => 10.0,
        _ => 25.0,
    }
}

fn severity_multiplier(severity: &str) -> f64 {
    match severity {
        "low" => 0.5,
        "high" => 2.0,
        _ => 1.0,
    }
}

fn parse_flags(raw: &Value) -> Vec<Value> {
    if let Some(arr) = raw.as_array() {
        return arr.clone();
    }
    if let Some(s) = raw.as_str() {
        if let Ok(Value::Array(arr)) = serde_json::from_str::<Value>(s) {
            return arr;
        }
    }
    Vec::new()
}

/// Repair cost = sum over the union of both sides' flags of
/// kind_weight x severity_multiplier. Pure and order-independent.
fn repair_cost(cost_flags: &Value, challenge_flags: &Value) -> f64 {
    parse_flags(cost_flags)
        .iter()
        .chain(parse_flags(challenge_flags).iter())
        .map(|flag| {
            let kind = flag.get("kind").and_then(|v| v.as_str()).unwrap_or("");
            let severity = flag
                .get("severity")
                .and_then(|v| v.as_str())
                .unwrap_or("medium");
            kind_weight(kind) * severity_multiplier(severity)
        })
        .sum()
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
enum Classification {
    Canonical,
    Tail,
    Reject,
}

/// Classify scored paths. Exactly one Canonical: the cheapest path (ties
/// broken by lexicographic path id so reruns agree). A path is Tail when its
/// cost is within 2x of the canonical cost plus a 20-point grace; beyond
/// that it is rejected.
fn classify(paths: &[(String, f64)]) -> Vec<(String, Classification)> {
    if paths.is_empty() {
        return Vec::new();
    }
    let mut sorted: Vec<(String, f64)> = paths.to_vec();
    sorted.sort_by(|a, b| {
        a.1.partial_cmp(&b.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.0.cmp(&b.0))
    });
    let canonical_id = sorted[0].0.clone();
    let min_cost = sorted[0].1;
    let tail_ceiling = min_cost * 2.0 + 20.0;
    sorted
        .into_iter()
        .map(|(id, cost)| {
            let class = if id == canonical_id {
                Classification::Canonical
            } else if cost <= tail_ceiling {
                Classification::Tail
            } else {
                Classification::Reject
            };
            (id, class)
        })
        .collect()
}

/// Endpoint weight from its best path cost: exp(-cost / 25), normalized over
/// all endpoints, rounded to 4 decimals. Deterministic for equal inputs.
fn endpoint_weights(best_costs: &[(String, f64)]) -> Vec<(String, f64)> {
    if best_costs.is_empty() {
        return Vec::new();
    }
    let raw: Vec<(String, f64)> = best_costs
        .iter()
        .map(|(id, cost)| (id.clone(), (-cost / 25.0).exp()))
        .collect();
    let total: f64 = raw.iter().map(|(_, w)| w).sum();
    let mut weights: Vec<(String, f64)> = raw
        .into_iter()
        .map(|(id, w)| (id, ((w / total) * 10000.0).round() / 10000.0))
        .collect();
    weights.sort_by(|a, b| a.0.cmp(&b.0));
    weights
}

fn fmt_cost(cost: f64) -> String {
    format!("{:.2}", cost)
}

/// Entry point.
#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));
        let status = ctx
            .entity_state
            .get("status")
            .and_then(|v| v.as_str())
            .or_else(|| fields.get("Status").and_then(|v| v.as_str()))
            .unwrap_or("")
            .to_string();

        match status.as_str() {
            "Challenged" => cost_phase(&ctx, &fields),
            "Scored" => classify_phase(&ctx, &fields),
            other => {
                ctx.log(
                    "warn",
                    &format!("aggregate_costs: unexpected path status {other}, nothing to do"),
                );
                Ok(())
            }
        }
    })();

    if let Err(e) = result {
        set_error_result(&e);
    }
    0
}

/// Status Challenged: compute the deterministic cost and dispatch Score.
fn cost_phase(ctx: &Context, fields: &Value) -> Result<(), String> {
    let cost_flags = fields.get("cost_flags").cloned().unwrap_or(json!([]));
    let challenge_flags = fields.get("challenge_flags").cloned().unwrap_or(json!([]));
    let cost = repair_cost(&cost_flags, &challenge_flags);
    ctx.log(
        "info",
        &format!(
            "aggregate_costs: path {} cost={} (repairer + adversary flags)",
            ctx.entity_id,
            fmt_cost(cost)
        ),
    );
    set_success_result("Score", &json!({ "repair_cost": fmt_cost(cost) }));
    Ok(())
}

/// Status Scored: if the whole pass is settled, classify and report upward.
fn classify_phase(ctx: &Context, fields: &Value) -> Result<(), String> {
    let world_id = fields
        .get("world_id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .ok_or("Path.world_id is required for classification")?
        .to_string();

    let temper_api_url = ctx
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

    // 1. Load every path for this world.
    let paths_url = format!("{temper_api_url}/tdata/Paths?$filter=world_id eq '{world_id}'");
    let resp = ctx.http_call("GET", &paths_url, &headers, "")?;
    if resp.status < 200 || resp.status >= 300 {
        return Err(format!("failed to list paths (HTTP {})", resp.status));
    }
    let body: Value = serde_json::from_str(&resp.body).unwrap_or(json!({}));
    let paths = body
        .get("value")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    // 2. The pass is settled when no path is still in flight.
    let mut scored: Vec<(String, f64, String)> = Vec::new(); // (path_id, cost, endpoint_id)
    for p in &paths {
        let id = p
            .get("Id")
            .or_else(|| p.get("entity_id"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let status = p.get("Status").and_then(|v| v.as_str()).unwrap_or("");
        match status {
            "Solving" | "Repaired" | "Challenged" => {
                ctx.log(
                    "info",
                    &format!("aggregate_costs: path {id} still {status}; pass not settled"),
                );
                return Ok(());
            }
            "Scored" => {
                let cost = p
                    .get("RepairCost")
                    .and_then(|v| v.as_str())
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(f64::MAX);
                let endpoint_id = p
                    .get("EndpointId")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                scored.push((id, cost, endpoint_id));
            }
            _ => {} // Canonical/Tail/Rejected/Failed from earlier passes
        }
    }

    if scored.is_empty() {
        ctx.log("info", "aggregate_costs: no freshly scored paths to classify");
        return Ok(());
    }

    // 3. Classify deterministically.
    let pairs: Vec<(String, f64)> = scored.iter().map(|(id, c, _)| (id.clone(), *c)).collect();
    let classified = classify(&pairs);
    let mut canonical_path_id = String::new();
    for (id, class) in &classified {
        let (action, note) = match class {
            Classification::Canonical => {
                canonical_path_id = id.clone();
                ("ClassifyCanonical", "lowest repair cost in pass")
            }
            Classification::Tail => ("ClassifyTail", "coherent but expensive: tail sample"),
            Classification::Reject => ("Reject", "repair cost beyond world tolerance"),
        };
        let url = format!("{temper_api_url}/tdata/Paths('{id}')/TemperPaw.{action}");
        let body = json!({ "classification_note": note });
        let r = ctx.http_call("POST", &url, &headers, &body.to_string())?;
        if r.status < 200 || r.status >= 300 {
            ctx.log(
                "warn",
                &format!(
                    "aggregate_costs: {action} on {id} failed (HTTP {})",
                    r.status
                ),
            );
        }
    }

    // 4. Endpoint weights from each endpoint's best path cost.
    let mut best: std::collections::BTreeMap<String, f64> = std::collections::BTreeMap::new();
    for (_, cost, endpoint_id) in &scored {
        if endpoint_id.is_empty() {
            continue;
        }
        let entry = best.entry(endpoint_id.clone()).or_insert(f64::MAX);
        if *cost < *entry {
            *entry = *cost;
        }
    }
    let best_costs: Vec<(String, f64)> = best.into_iter().collect();
    for (endpoint_id, weight) in endpoint_weights(&best_costs) {
        let url =
            format!("{temper_api_url}/tdata/Endpoints('{endpoint_id}')/TemperPaw.ScoreComplete");
        let body = json!({ "weight": format!("{:.4}", weight) });
        let r = ctx.http_call("POST", &url, &headers, &body.to_string())?;
        if r.status >= 200 && r.status < 300 {
            let mark_url = format!(
                "{temper_api_url}/tdata/Endpoints('{endpoint_id}')/TemperPaw.MarkWeighted"
            );
            let _ = ctx.http_call("POST", &mark_url, &headers, &json!({}).to_string());
        } else {
            ctx.log(
                "warn",
                &format!(
                    "aggregate_costs: ScoreComplete on endpoint {endpoint_id} failed (HTTP {})",
                    r.status
                ),
            );
        }
    }

    // 5. Report the settled pass to the world.
    let url = format!("{temper_api_url}/tdata/Worlds('{world_id}')/TemperPaw.PathsScored");
    let body = json!({ "canonical_path_id": canonical_path_id });
    let r = ctx.http_call("POST", &url, &headers, &body.to_string())?;
    if r.status < 200 || r.status >= 300 {
        return Err(format!(
            "World.PathsScored dispatch failed (HTTP {})",
            r.status
        ));
    }
    ctx.log(
        "info",
        &format!(
            "aggregate_costs: pass settled for world {world_id}; canonical={canonical_path_id}"
        ),
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flag(kind: &str, severity: &str) -> Value {
        json!({"kind": kind, "severity": severity, "note": "t"})
    }

    #[test]
    fn cost_sums_kind_weights_times_severity() {
        let cost_flags = json!([flag("contradiction", "high"), flag("lag", "low")]);
        let challenge_flags = json!([flag("incentive", "medium")]);
        // 25*2 + 10*0.5 + 15*1 = 70
        assert_eq!(repair_cost(&cost_flags, &challenge_flags), 70.0);
    }

    #[test]
    fn cost_accepts_json_strings_and_is_order_independent() {
        let a =
            json!(r#"[{"kind":"miracle","severity":"medium"},{"kind":"lag","severity":"high"}]"#);
        let b = json!([]);
        let forward = repair_cost(&a, &b);
        let reversed = repair_cost(&b, &a);
        assert_eq!(forward, 60.0); // 40 + 20
        assert_eq!(forward, reversed);
    }

    #[test]
    fn empty_and_malformed_flags_cost_nothing_but_unknown_kinds_are_not_free() {
        assert_eq!(repair_cost(&json!([]), &json!("not json")), 0.0);
        assert_eq!(
            repair_cost(&json!([flag("mystery", "medium")]), &json!([])),
            25.0
        );
    }

    #[test]
    fn classification_has_exactly_one_canonical_with_lexicographic_tiebreak() {
        let paths = vec![
            ("p-b".to_string(), 30.0),
            ("p-a".to_string(), 30.0),
            ("p-c".to_string(), 75.0),
            ("p-d".to_string(), 200.0),
        ];
        let out = classify(&paths);
        let canon: Vec<_> = out
            .iter()
            .filter(|(_, c)| *c == Classification::Canonical)
            .collect();
        assert_eq!(canon.len(), 1);
        assert_eq!(canon[0].0, "p-a"); // tie with p-b broken lexicographically
        // ceiling = 30*2+20 = 80: p-c is Tail, p-d Reject
        assert!(out.contains(&("p-c".to_string(), Classification::Tail)));
        assert!(out.contains(&("p-d".to_string(), Classification::Reject)));
    }

    #[test]
    fn classification_is_deterministic_across_input_orderings() {
        let a = vec![("x".to_string(), 10.0), ("y".to_string(), 50.0)];
        let b = vec![("y".to_string(), 50.0), ("x".to_string(), 10.0)];
        let mut ca = classify(&a);
        let mut cb = classify(&b);
        ca.sort();
        cb.sort();
        assert_eq!(ca, cb);
    }

    #[test]
    fn endpoint_weights_normalize_and_favor_cheap_repairs() {
        let weights = endpoint_weights(&[
            ("e-cheap".to_string(), 10.0),
            ("e-dear".to_string(), 60.0),
        ]);
        let total: f64 = weights.iter().map(|(_, w)| w).sum();
        assert!((total - 1.0).abs() < 0.001, "weights sum to ~1, got {total}");
        let cheap = weights.iter().find(|(id, _)| id == "e-cheap").unwrap().1;
        let dear = weights.iter().find(|(id, _)| id == "e-dear").unwrap().1;
        assert!(cheap > dear);
        // exp(-10/25) vs exp(-60/25): ratio e^2
        assert!((cheap / dear - (2.0f64).exp()).abs() < 0.05);
    }

    #[test]
    fn single_endpoint_takes_full_weight() {
        let weights = endpoint_weights(&[("only".to_string(), 999.0)]);
        assert_eq!(weights, vec![("only".to_string(), 1.0)]);
    }
}
