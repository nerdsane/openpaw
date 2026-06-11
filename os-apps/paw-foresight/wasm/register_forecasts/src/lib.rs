//! register_forecasts — preregister the engine's gradeable word (ADR-002).
//!
//! On World.PathsScored: every EventNode that is a live, genuinely
//! probabilistic, non-determined claim resolving inside the world's frontier
//! becomes a Forecast — preregistered, immutable, graded later by
//! evidence_ingest. Nodes already carrying a Forecast are skipped: one
//! registration per node, ever. No follow-up action is dispatched —
//! registration is a side effect of scoring, not a state transition.
//!
//! Build: `cargo build --target wasm32-unknown-unknown --release`

use temper_wasm_sdk::prelude::*;

/// Engine version stamped on every registration.
const ENGINE_VERSION: &str = "0.2.0";

/// Pure selection rule: is this node a registrable forecast?
///
/// Registrable means: still live (Proposed or Confirmed), genuinely
/// probabilistic (probability parses into (0, 1) exclusive — 1.0 is a
/// determined fact, not a forecast), not skeleton provenance, and resolving
/// on or before the world's frontier date (ISO dates compare
/// lexicographically, so a plain string compare is correct).
fn should_register(
    status: &str,
    probability: &str,
    provenance: &str,
    resolve_by: &str,
    frontier: &str,
) -> bool {
    if status != "Proposed" && status != "Confirmed" {
        return false;
    }
    if provenance == "determined" {
        return false;
    }
    let p = match probability.trim().parse::<f64>() {
        Ok(p) => p,
        Err(_) => return false,
    };
    if p <= 0.0 || p >= 1.0 {
        return false;
    }
    !resolve_by.is_empty() && !frontier.is_empty() && resolve_by <= frontier
}

/// First entry of a source_refs JSON array — the market_ref for market nodes.
fn first_source_ref(source_refs: &str) -> String {
    serde_json::from_str::<Value>(source_refs)
        .ok()
        .and_then(|v| {
            v.as_array()
                .and_then(|arr| arr.first().and_then(|x| x.as_str().map(str::to_string)))
        })
        .unwrap_or_default()
}

/// Entry point.
#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));
        let get = |k: &str| -> String {
            fields
                .get(k)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string()
        };

        let world_id = ctx.entity_id.clone();
        let frontier = get("frontier_date");
        // The as-of date of the most recent evidence ingest, if any: WASM has
        // no clock, so registered_at can only be as fresh as the last ingest.
        let registered_at = get("last_ingest_date");

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
            ("x-temper-principal-id".to_string(), world_id.clone()),
            ("x-temper-agent-type".to_string(), "system".to_string()),
        ];

        // 1. Load every EventNode in this world.
        let nodes_url = format!("{api}/tdata/EventNodes?$filter=world_id eq '{world_id}'");
        let resp = ctx.http_call("GET", &nodes_url, &headers, "")?;
        if resp.status < 200 || resp.status >= 300 {
            return Err(format!("failed to list EventNodes (HTTP {})", resp.status));
        }
        let body: Value = serde_json::from_str(&resp.body).unwrap_or(json!({}));
        let nodes = body
            .get("value")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        let mut registered = 0usize;
        for node in &nodes {
            let str_of = |k: &str| node.get(k).and_then(|v| v.as_str()).unwrap_or("");
            let node_id = node
                .get("Id")
                .or_else(|| node.get("entity_id"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if node_id.is_empty() {
                continue;
            }
            if !should_register(
                str_of("Status"),
                str_of("Probability"),
                str_of("Provenance"),
                str_of("ResolveBy"),
                &frontier,
            ) {
                continue;
            }

            // 2. One Forecast per node, ever: skip nodes already registered.
            let existing_url =
                format!("{api}/tdata/Forecasts?$filter=event_node_id eq '{node_id}'");
            let existing = ctx.http_call("GET", &existing_url, &headers, "")?;
            if existing.status < 200 || existing.status >= 300 {
                ctx.log(
                    "warn",
                    &format!(
                        "register_forecasts: Forecast lookup for node {node_id} failed (HTTP {}); skipping",
                        existing.status
                    ),
                );
                continue;
            }
            let has_forecast = serde_json::from_str::<Value>(&existing.body)
                .ok()
                .and_then(|v| {
                    v.get("value")
                        .and_then(|x| x.as_array())
                        .map(|a| !a.is_empty())
                })
                .unwrap_or(false);
            if has_forecast {
                continue;
            }

            // 3. Preregister.
            let market_ref = if str_of("Provenance") == "market" {
                first_source_ref(str_of("SourceRefs"))
            } else {
                String::new()
            };
            let create_body = json!({
                "world_id": world_id,
                "event_node_id": node_id,
                "question": str_of("Statement"),
                "probability": str_of("Probability"),
                "resolve_by": str_of("ResolveBy"),
                "market_ref": market_ref,
                "engine_version": ENGINE_VERSION,
                "registered_at": registered_at,
            });
            let created = ctx.http_call(
                "POST",
                &format!("{api}/tdata/Forecasts"),
                &headers,
                &create_body.to_string(),
            )?;
            if created.status < 200 || created.status >= 300 {
                ctx.log(
                    "warn",
                    &format!(
                        "register_forecasts: Forecast create for node {node_id} failed (HTTP {})",
                        created.status
                    ),
                );
                continue;
            }
            registered += 1;
            ctx.log(
                "info",
                &format!(
                    "register_forecasts: registered forecast for node {node_id} (p={}, resolve_by={})",
                    str_of("Probability"),
                    str_of("ResolveBy")
                ),
            );
        }

        ctx.log(
            "info",
            &format!(
                "register_forecasts: world {world_id} — {registered} forecast(s) registered from {} node(s)",
                nodes.len()
            ),
        );
        Ok(())
    })();

    if let Err(e) = result {
        set_error_result(&e);
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRONTIER: &str = "2026-09-30";

    #[test]
    fn determined_provenance_is_never_registered() {
        assert!(!should_register(
            "Confirmed",
            "0.50",
            "determined",
            "2026-06-30",
            FRONTIER
        ));
    }

    #[test]
    fn probability_one_is_a_fact_not_a_forecast() {
        assert!(!should_register(
            "Confirmed",
            "1.0",
            "authored",
            "2026-06-30",
            FRONTIER
        ));
        // ...and the other degenerate end is no forecast either.
        assert!(!should_register(
            "Confirmed",
            "0.0",
            "authored",
            "2026-06-30",
            FRONTIER
        ));
    }

    #[test]
    fn past_frontier_claims_are_structured_speculation_not_forecasts() {
        assert!(!should_register(
            "Confirmed",
            "0.50",
            "authored",
            "2026-12-31",
            FRONTIER
        ));
        // No resolve-by date at all means nothing to grade against.
        assert!(!should_register(
            "Confirmed",
            "0.50",
            "authored",
            "",
            FRONTIER
        ));
    }

    #[test]
    fn resolved_nodes_are_excluded() {
        assert!(!should_register(
            "Resolved",
            "0.50",
            "authored",
            "2026-06-30",
            FRONTIER
        ));
        assert!(!should_register(
            "Retired",
            "0.50",
            "market",
            "2026-06-30",
            FRONTIER
        ));
    }

    #[test]
    fn live_probabilistic_inside_frontier_claims_register() {
        assert!(should_register(
            "Confirmed",
            "0.65",
            "market",
            "2026-06-30",
            FRONTIER
        ));
        // Resolving exactly on the frontier still counts.
        assert!(should_register(
            "Proposed", "0.10", "authored", FRONTIER, FRONTIER
        ));
        // Unparseable probabilities never register.
        assert!(!should_register(
            "Confirmed",
            "likely",
            "authored",
            "2026-06-30",
            FRONTIER
        ));
    }

    #[test]
    fn market_ref_is_the_first_source_ref() {
        assert_eq!(
            first_source_ref(r#"["https://polymarket.com/event/x", "file:abc"]"#),
            "https://polymarket.com/event/x"
        );
        assert_eq!(first_source_ref("[]"), "");
        assert_eq!(first_source_ref("not json"), "");
    }
}
