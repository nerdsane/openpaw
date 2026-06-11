//! evidence_ingest — feed reality back into a world (ADR-002).
//!
//! On World.IngestEvidence: re-fetch market prices for market-provenance
//! EventNodes, snapshot every raw response to paw-fs, update probabilities,
//! resolve overdue nodes the evidence settles, grade the forecasts those
//! resolutions touch, and — if a resolved-"no" node is load-bearing for the
//! canonical path — dispatch World.BeginUpdate.
//!
//! The as-of date comes from the world's last_ingest_date field (set by the
//! IngestEvidence action params): WASM has no clock, the date is data.
//!
//! v1 limitation (honest, by design): only market-provenance nodes with a
//! polymarket.com or kalshi.com source URL get fresh evidence. Overdue
//! authored nodes, and overdue market nodes whose price sits between the
//! resolve thresholds, are logged and left open — resolving them needs
//! judgment this module does not pretend to have.
//!
//! Build: `cargo build --target wasm32-unknown-unknown --release`

use temper_wasm_sdk::prelude::*;

// --- Pure rules -----------------------------------------------------------

/// Brier score for a registered probability against a yes/no outcome,
/// formatted to 4 decimals (yes = 1.0, no = 0.0).
fn brier(probability: f64, outcome_yes: bool) -> String {
    let outcome = if outcome_yes { 1.0 } else { 0.0 };
    format!("{:.4}", (probability - outcome).powi(2))
}

/// The resolve-threshold rule: a market price only settles a question when
/// it is overwhelming. Anything in between needs judgment — leave it open.
fn resolution_for_price(price: f64) -> Option<&'static str> {
    if price >= 0.95 {
        Some("yes")
    } else if price <= 0.05 {
        Some("no")
    } else {
        None
    }
}

/// First polymarket/kalshi URL inside a source_refs JSON array.
fn market_url(source_refs: &str) -> Option<String> {
    serde_json::from_str::<Value>(source_refs)
        .ok()?
        .as_array()?
        .iter()
        .filter_map(|v| v.as_str())
        .find(|u| u.contains("polymarket.com") || u.contains("kalshi.com"))
        .map(str::to_string)
}

/// Gamma-API slug from a polymarket.com URL: the path segment after /event/.
fn polymarket_slug(url: &str) -> Option<String> {
    if !url.contains("polymarket.com") {
        return None;
    }
    let path = url.split(['?', '#']).next().unwrap_or("");
    let mut segments = path.split('/').skip_while(|s| *s != "event");
    segments.next()?; // the "event" segment itself
    let slug = segments.next()?.trim();
    if slug.is_empty() {
        None
    } else {
        Some(slug.to_string())
    }
}

/// Trade-API ticker from a kalshi.com URL: the last path segment, uppercased.
fn kalshi_ticker(url: &str) -> Option<String> {
    if !url.contains("kalshi.com") {
        return None;
    }
    let path = url.split(['?', '#']).next().unwrap_or("");
    let segment = path.trim_end_matches('/').rsplit('/').next()?.trim();
    // A bare domain (or anything dotted) is not a ticker.
    if segment.is_empty() || segment.contains('.') {
        None
    } else {
        Some(segment.to_uppercase())
    }
}

/// Best-effort Polymarket gamma /events?slug= parse: first event → first
/// market → first outcomePrices entry (the Yes price). outcomePrices arrives
/// as a JSON-encoded string; tolerate a plain array too.
fn parse_polymarket_price(body: &str) -> Option<f64> {
    let v: Value = serde_json::from_str(body).ok()?;
    let market = v
        .as_array()?
        .first()?
        .get("markets")?
        .as_array()?
        .first()?
        .clone();
    let prices_raw = market.get("outcomePrices")?.clone();
    let prices: Value = match prices_raw {
        Value::String(s) => serde_json::from_str(&s).ok()?,
        other => other,
    };
    let first = prices.as_array()?.first()?.clone();
    let price = match first {
        Value::String(s) => s.trim().parse::<f64>().ok()?,
        Value::Number(n) => n.as_f64()?,
        _ => return None,
    };
    (0.0..=1.0).contains(&price).then_some(price)
}

/// Best-effort Kalshi /markets/<ticker> parse: market.last_price in cents,
/// falling back to the yes bid/ask midpoint.
fn parse_kalshi_price(body: &str) -> Option<f64> {
    let v: Value = serde_json::from_str(body).ok()?;
    let market = v.get("market")?;
    let cents = market
        .get("last_price")
        .and_then(Value::as_f64)
        .or_else(|| {
            let bid = market.get("yes_bid").and_then(Value::as_f64)?;
            let ask = market.get("yes_ask").and_then(Value::as_f64)?;
            Some((bid + ask) / 2.0)
        })?;
    (0.0..=100.0).contains(&cents).then_some(cents / 100.0)
}

// --- Host plumbing ---------------------------------------------------------

fn system_headers(ctx: &Context) -> Vec<(String, String)> {
    vec![
        ("content-type".to_string(), "application/json".to_string()),
        ("x-tenant-id".to_string(), ctx.tenant.clone()),
        ("x-temper-principal-kind".to_string(), "agent".to_string()),
        ("x-temper-principal-id".to_string(), ctx.entity_id.clone()),
        ("x-temper-agent-type".to_string(), "system".to_string()),
    ]
}

/// Snapshot a raw evidence body into paw-fs. Never fatal: a lost snapshot
/// costs provenance, not correctness — warn and continue.
fn snapshot_evidence(
    ctx: &Context,
    api: &str,
    headers: &[(String, String)],
    node_id: &str,
    raw_body: &str,
) -> Option<String> {
    let create_body = json!({ "name": format!("evidence-snapshot-{node_id}") });
    let created = match ctx.http_call(
        "POST",
        &format!("{api}/tdata/Files"),
        headers,
        &create_body.to_string(),
    ) {
        Ok(r) if (200..300).contains(&r.status) => r,
        Ok(r) => {
            ctx.log(
                "warn",
                &format!(
                    "evidence_ingest: snapshot File create for node {node_id} failed (HTTP {})",
                    r.status
                ),
            );
            return None;
        }
        Err(e) => {
            ctx.log(
                "warn",
                &format!("evidence_ingest: snapshot File create for node {node_id} failed: {e}"),
            );
            return None;
        }
    };
    let file_id = serde_json::from_str::<Value>(&created.body)
        .ok()
        .and_then(|v| {
            v.get("entity_id")
                .and_then(|x| x.as_str())
                .map(str::to_string)
        })?;
    let put_url = format!("{api}/tdata/Files('{file_id}')/$value");
    match ctx.http_call("PUT", &put_url, headers, raw_body) {
        Ok(r) if (200..300).contains(&r.status) => Some(file_id),
        Ok(r) => {
            ctx.log(
                "warn",
                &format!(
                    "evidence_ingest: snapshot PUT for node {node_id} failed (HTTP {})",
                    r.status
                ),
            );
            None
        }
        Err(e) => {
            ctx.log(
                "warn",
                &format!("evidence_ingest: snapshot PUT for node {node_id} failed: {e}"),
            );
            None
        }
    }
}

/// A fresh market read for one node.
struct FreshPrice {
    price: f64,
    /// JSON source_refs: the original URL plus the snapshot ref when taken.
    refs: String,
}

/// Entry point.
#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        ingest(&ctx)
    })();

    if let Err(e) = result {
        set_error_result(&e);
    }
    0
}

fn ingest(ctx: &Context) -> Result<(), String> {
    let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));
    let get = |k: &str| -> String {
        fields
            .get(k)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    };

    let world_id = ctx.entity_id.clone();
    let as_of = get("last_ingest_date");
    if as_of.trim().is_empty() {
        return Err(
            "World.last_ingest_date is required: the caller of IngestEvidence supplies the \
             as-of date (WASM has no clock)"
                .to_string(),
        );
    }

    let api = ctx
        .config
        .get("temper_api_url")
        .filter(|s| !s.is_empty() && !s.contains("{secret:"))
        .cloned()
        .unwrap_or_else(|| "http://127.0.0.1:3000".to_string());
    let headers = system_headers(ctx);
    let external_headers = vec![("accept".to_string(), "application/json".to_string())];

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

    // 2. Re-fetch market prices for live market nodes; snapshot raw bodies.
    let mut fresh: std::collections::BTreeMap<String, FreshPrice> =
        std::collections::BTreeMap::new();
    for node in &nodes {
        let str_of = |k: &str| node.get(k).and_then(|v| v.as_str()).unwrap_or("");
        let node_id = node
            .get("Id")
            .or_else(|| node.get("entity_id"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if node_id.is_empty() || str_of("Provenance") != "market" {
            continue;
        }
        // Confirmed only: UpdateProbability's automaton is Confirmed-only, so
        // a fresh price for a Proposed node has no consumer.
        let status = str_of("Status");
        if status != "Confirmed" {
            continue;
        }
        let Some(url) = market_url(str_of("SourceRefs")) else {
            continue;
        };
        let fetch_url = if let Some(slug) = polymarket_slug(&url) {
            format!("https://gamma-api.polymarket.com/events?slug={slug}")
        } else if let Some(ticker) = kalshi_ticker(&url) {
            format!("https://api.elections.kalshi.com/trade-api/v2/markets/{ticker}")
        } else {
            ctx.log(
                "warn",
                &format!(
                    "evidence_ingest: node {node_id} market URL {url} has no extractable slug/ticker; skipping"
                ),
            );
            continue;
        };
        let market_resp = match ctx.http_call("GET", &fetch_url, &external_headers, "") {
            Ok(r) if (200..300).contains(&r.status) => r,
            Ok(r) => {
                ctx.log(
                    "warn",
                    &format!(
                        "evidence_ingest: market fetch for node {node_id} failed (HTTP {}); skipping",
                        r.status
                    ),
                );
                continue;
            }
            Err(e) => {
                ctx.log(
                    "warn",
                    &format!("evidence_ingest: market fetch for node {node_id} failed: {e}; skipping"),
                );
                continue;
            }
        };
        let price = if url.contains("polymarket.com") {
            parse_polymarket_price(&market_resp.body)
        } else {
            parse_kalshi_price(&market_resp.body)
        };
        let Some(price) = price else {
            ctx.log(
                "warn",
                &format!(
                    "evidence_ingest: market response shape for node {node_id} surprised the parser; skipping"
                ),
            );
            continue;
        };
        let snapshot_id = snapshot_evidence(ctx, &api, &headers, &node_id, &market_resp.body);
        let refs = match &snapshot_id {
            Some(id) => json!([url, format!("file:{id}")]),
            None => json!([url]),
        }
        .to_string();
        ctx.log(
            "info",
            &format!("evidence_ingest: node {node_id} fresh market price {price:.2} from {url}"),
        );
        fresh.insert(node_id, FreshPrice { price, refs });
    }

    // 3. Apply fresh prices.
    for (node_id, fp) in &fresh {
        let update_url = format!("{api}/tdata/EventNodes('{node_id}')/TemperPaw.UpdateProbability");
        let update_body = json!({
            "probability": format!("{:.2}", fp.price),
            "provenance": "market",
            "source_refs": fp.refs,
        });
        match ctx.http_call("POST", &update_url, &headers, &update_body.to_string()) {
            Ok(r) if (200..300).contains(&r.status) => {}
            Ok(r) => ctx.log(
                "warn",
                &format!(
                    "evidence_ingest: UpdateProbability on node {node_id} failed (HTTP {})",
                    r.status
                ),
            ),
            Err(e) => ctx.log(
                "warn",
                &format!("evidence_ingest: UpdateProbability on node {node_id} failed: {e}"),
            ),
        }
    }

    // 4. Resolution sweep over overdue Confirmed nodes.
    let mut resolved: Vec<(String, &'static str, String)> = Vec::new(); // (node_id, outcome, refs)
    for node in &nodes {
        let str_of = |k: &str| node.get(k).and_then(|v| v.as_str()).unwrap_or("");
        let node_id = node
            .get("Id")
            .or_else(|| node.get("entity_id"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let resolve_by = str_of("ResolveBy");
        if node_id.is_empty()
            || str_of("Status") != "Confirmed"
            || resolve_by.is_empty()
            || resolve_by > as_of.as_str()
        {
            continue;
        }
        let Some(fp) = fresh.get(&node_id) else {
            // v1 honest limitation: no fresh market evidence — authored nodes
            // and failed fetches alike need judgment we do not fake.
            ctx.log(
                "info",
                &format!(
                    "evidence_ingest: node {node_id} overdue (resolve_by {resolve_by}) with no fresh market evidence; leaving open"
                ),
            );
            continue;
        };
        let Some(outcome) = resolution_for_price(fp.price) else {
            ctx.log(
                "info",
                &format!(
                    "evidence_ingest: node {node_id} overdue at price {:.2} — needs judgment, leaving open",
                    fp.price
                ),
            );
            continue;
        };
        let resolve_url = format!("{api}/tdata/EventNodes('{node_id}')/TemperPaw.Resolve");
        let resolve_body = json!({
            "resolution": outcome,
            "resolved_at": as_of,
            "source_refs": fp.refs,
        });
        match ctx.http_call("POST", &resolve_url, &headers, &resolve_body.to_string()) {
            Ok(r) if (200..300).contains(&r.status) => {
                ctx.log(
                    "info",
                    &format!("evidence_ingest: node {node_id} resolved {outcome} at price {:.2}", fp.price),
                );
                resolved.push((node_id.clone(), outcome, fp.refs.clone()));
            }
            Ok(r) => ctx.log(
                "warn",
                &format!(
                    "evidence_ingest: Resolve on node {node_id} failed (HTTP {})",
                    r.status
                ),
            ),
            Err(e) => ctx.log(
                "warn",
                &format!("evidence_ingest: Resolve on node {node_id} failed: {e}"),
            ),
        }
    }

    // 5. Grade the forecasts those resolutions settle.
    if !resolved.is_empty() {
        let forecasts_url = format!("{api}/tdata/Forecasts?$filter=world_id eq '{world_id}'");
        let fresp = ctx.http_call("GET", &forecasts_url, &headers, "")?;
        if fresp.status < 200 || fresp.status >= 300 {
            return Err(format!("failed to list Forecasts (HTTP {})", fresp.status));
        }
        let fbody: Value = serde_json::from_str(&fresp.body).unwrap_or(json!({}));
        let forecasts = fbody
            .get("value")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        for forecast in &forecasts {
            let str_of = |k: &str| forecast.get(k).and_then(|v| v.as_str()).unwrap_or("");
            if str_of("Status") != "Preregistered" {
                continue;
            }
            let forecast_id = forecast
                .get("Id")
                .or_else(|| forecast.get("entity_id"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let node_id = str_of("EventNodeId");
            let Some((_, outcome, refs)) = resolved.iter().find(|(id, _, _)| id == node_id)
            else {
                continue;
            };
            let Ok(probability) = str_of("Probability").trim().parse::<f64>() else {
                ctx.log(
                    "warn",
                    &format!(
                        "evidence_ingest: forecast {forecast_id} has unparseable probability {:?}; cannot grade, leaving preregistered",
                        str_of("Probability")
                    ),
                );
                continue;
            };
            let resolve_url = format!("{api}/tdata/Forecasts('{forecast_id}')/TemperPaw.Resolve");
            let resolve_body = json!({ "outcome": outcome, "outcome_source_refs": refs });
            match ctx.http_call("POST", &resolve_url, &headers, &resolve_body.to_string()) {
                Ok(r) if (200..300).contains(&r.status) => {}
                Ok(r) => {
                    ctx.log(
                        "warn",
                        &format!(
                            "evidence_ingest: Forecast.Resolve on {forecast_id} failed (HTTP {})",
                            r.status
                        ),
                    );
                    continue;
                }
                Err(e) => {
                    ctx.log(
                        "warn",
                        &format!("evidence_ingest: Forecast.Resolve on {forecast_id} failed: {e}"),
                    );
                    continue;
                }
            }
            let score = brier(probability, *outcome == "yes");
            let score_url = format!("{api}/tdata/Forecasts('{forecast_id}')/TemperPaw.Score");
            match ctx.http_call(
                "POST",
                &score_url,
                &headers,
                &json!({ "brier": score }).to_string(),
            ) {
                Ok(r) if (200..300).contains(&r.status) => ctx.log(
                    "info",
                    &format!("evidence_ingest: forecast {forecast_id} graded — outcome {outcome}, brier {score}"),
                ),
                Ok(r) => ctx.log(
                    "warn",
                    &format!(
                        "evidence_ingest: Forecast.Score on {forecast_id} failed (HTTP {})",
                        r.status
                    ),
                ),
                Err(e) => ctx.log(
                    "warn",
                    &format!("evidence_ingest: Forecast.Score on {forecast_id} failed: {e}"),
                ),
            }
        }
    }

    // 6. Drift: a load-bearing node resolving "no" demands a re-solve.
    let no_nodes: Vec<&String> = resolved
        .iter()
        .filter(|(_, outcome, _)| *outcome == "no")
        .map(|(id, _, _)| id)
        .collect();
    let canonical_path_id = get("canonical_path_id");
    if !no_nodes.is_empty() && !canonical_path_id.is_empty() {
        let path_url = format!("{api}/tdata/Paths('{canonical_path_id}')");
        let presp = ctx.http_call("GET", &path_url, &headers, "")?;
        if presp.status < 200 || presp.status >= 300 {
            ctx.log(
                "warn",
                &format!(
                    "evidence_ingest: could not read canonical path {canonical_path_id} (HTTP {}); drift check skipped",
                    presp.status
                ),
            );
            return Ok(());
        }
        let path: Value = serde_json::from_str(&presp.body).unwrap_or(json!({}));
        let required: Vec<String> = path
            .get("RequiredNodeIds")
            .and_then(|v| v.as_str())
            .and_then(|s| serde_json::from_str::<Value>(s).ok())
            .and_then(|v| v.as_array().cloned())
            .map(|arr| {
                arr.iter()
                    .filter_map(|x| x.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        if let Some(culprit) = no_nodes.iter().find(|id| required.contains(id)) {
            ctx.log(
                "info",
                &format!(
                    "evidence_ingest: node {culprit} resolved no and is load-bearing for canonical path {canonical_path_id}; dispatching BeginUpdate"
                ),
            );
            set_success_result("BeginUpdate", &json!({ "update_cause_node_id": culprit }));
            return Ok(());
        }
    }

    ctx.log(
        "info",
        &format!(
            "evidence_ingest: world {world_id} ingest as of {as_of} — {} fresh price(s), {} resolution(s), no drift",
            fresh.len(),
            resolved.len()
        ),
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brier_is_squared_error_to_four_decimals() {
        assert_eq!(brier(0.65, true), "0.1225"); // (0.65-1)^2
        assert_eq!(brier(0.65, false), "0.4225"); // (0.65-0)^2
        assert_eq!(brier(1.0, true), "0.0000");
        assert_eq!(brier(0.0, true), "1.0000");
        assert_eq!(brier(0.3333, false), "0.1111");
    }

    #[test]
    fn resolve_thresholds_demand_overwhelming_evidence() {
        assert_eq!(resolution_for_price(0.99), Some("yes"));
        assert_eq!(resolution_for_price(0.95), Some("yes"));
        assert_eq!(resolution_for_price(0.01), Some("no"));
        assert_eq!(resolution_for_price(0.05), Some("no"));
        // Anything in between needs judgment: leave open.
        assert_eq!(resolution_for_price(0.94), None);
        assert_eq!(resolution_for_price(0.5), None);
        assert_eq!(resolution_for_price(0.06), None);
    }

    #[test]
    fn polymarket_slug_comes_from_the_event_path_segment() {
        assert_eq!(
            polymarket_slug("https://polymarket.com/event/fed-cuts-rates-june?tid=99"),
            Some("fed-cuts-rates-june".to_string())
        );
        assert_eq!(
            polymarket_slug("https://polymarket.com/event/fed-cuts/market-slug"),
            Some("fed-cuts".to_string())
        );
        assert_eq!(polymarket_slug("https://polymarket.com/"), None);
        assert_eq!(polymarket_slug("https://kalshi.com/markets/KXFED"), None);
    }

    #[test]
    fn kalshi_ticker_is_the_last_path_segment_uppercased() {
        assert_eq!(
            kalshi_ticker("https://kalshi.com/markets/kxfed/fed-decision/kxfed-26jun-t4"),
            Some("KXFED-26JUN-T4".to_string())
        );
        assert_eq!(
            kalshi_ticker("https://kalshi.com/markets/INXD-26DEC31-B5000?ref=x"),
            Some("INXD-26DEC31-B5000".to_string())
        );
        assert_eq!(kalshi_ticker("https://kalshi.com/"), None);
        assert_eq!(kalshi_ticker("https://polymarket.com/event/x"), None);
    }

    #[test]
    fn market_url_finds_the_first_supported_market_in_source_refs() {
        assert_eq!(
            market_url(r#"["https://example.com/a", "https://kalshi.com/markets/T-1"]"#),
            Some("https://kalshi.com/markets/T-1".to_string())
        );
        assert_eq!(market_url(r#"["https://example.com/a"]"#), None);
        assert_eq!(market_url("not json"), None);
    }

    #[test]
    fn market_response_parsers_are_best_effort() {
        // Polymarket: outcomePrices is a JSON-encoded string.
        let poly = r#"[{"markets": [{"outcomePrices": "[\"0.97\", \"0.03\"]"}]}]"#;
        assert_eq!(parse_polymarket_price(poly), Some(0.97));
        assert_eq!(parse_polymarket_price("[]"), None);
        assert_eq!(parse_polymarket_price("{\"weird\": true}"), None);

        // Kalshi: cents → probability, midpoint fallback.
        let kalshi = r#"{"market": {"last_price": 96}}"#;
        assert_eq!(parse_kalshi_price(kalshi), Some(0.96));
        let kalshi_mid = r#"{"market": {"yes_bid": 4, "yes_ask": 6}}"#;
        assert_eq!(parse_kalshi_price(kalshi_mid), Some(0.05));
        assert_eq!(parse_kalshi_price("{}"), None);
    }
}
