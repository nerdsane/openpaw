//! Datadog API tool ported from tool_runner/datadog.rs.
//!
//! Dispatched as `temper.datadog_query(...)` from Monty code.

use serde_json::{Value, json};
use temper_wasm_sdk::context::Context;

pub fn datadog_query(ctx: &Context, args: &[Value]) -> Result<Value, String> {
    let input = args
        .first()
        .filter(|v| v.is_object())
        .cloned()
        .unwrap_or(json!({}));

    let query_kind = input
        .get("query_kind")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("monitor_status");
    let limit = input
        .get("limit")
        .and_then(Value::as_u64)
        .unwrap_or(25)
        .clamp(1, 100) as usize;

    let api_key = ctx.config.get("dd_api_key").cloned().unwrap_or_default();
    let app_key = ctx.config.get("dd_app_key").cloned().unwrap_or_default();
    if api_key.trim().is_empty()
        || api_key.contains("{secret:")
        || app_key.trim().is_empty()
        || app_key.contains("{secret:")
    {
        return Err(
            "datadog_query: missing Datadog credentials; configure dd_api_key and dd_app_key secrets"
                .into(),
        );
    }

    let site = ctx
        .config
        .get("dd_site")
        .map(String::as_str)
        .unwrap_or("datadoghq.com");
    let base_url = datadog_base_url(site);
    let headers = vec![
        ("DD-API-KEY".to_string(), api_key),
        ("DD-APPLICATION-KEY".to_string(), app_key),
        ("accept".to_string(), "application/json".to_string()),
    ];

    let url = match query_kind {
        "monitor_status" => {
            let monitor_id = input
                .get("monitor_id")
                .and_then(|value| {
                    value
                        .as_str()
                        .map(ToOwned::to_owned)
                        .or_else(|| value.as_i64().map(|v| v.to_string()))
                })
                .ok_or("datadog_query: monitor_status requires monitor_id")?;
            format!("{base_url}/api/v1/monitor/{monitor_id}")
        }
        "recent_events" => {
            let query = input.get("query").and_then(Value::as_str).unwrap_or("");
            let start = input.get("from_ts").and_then(Value::as_i64).unwrap_or(0);
            let end = input
                .get("to_ts")
                .and_then(Value::as_i64)
                .unwrap_or(4_102_444_800);
            let tags = input.get("tags").and_then(Value::as_str).unwrap_or("");
            let mut u = format!("{base_url}/api/v1/events?start={start}&end={end}&unparsed=true");
            if !query.trim().is_empty() {
                u.push_str("&query=");
                u.push_str(&urlenc(query));
            }
            if !tags.trim().is_empty() {
                u.push_str("&tags=");
                u.push_str(&urlenc(tags));
            }
            u
        }
        "metrics_query" => {
            let query = input
                .get("query")
                .and_then(Value::as_str)
                .filter(|s| !s.trim().is_empty())
                .ok_or("datadog_query: metrics_query requires query")?;
            let from = input.get("from_ts").and_then(Value::as_i64).unwrap_or(0);
            let to = input
                .get("to_ts")
                .and_then(Value::as_i64)
                .unwrap_or(4_102_444_800);
            format!(
                "{base_url}/api/v1/query?from={from}&to={to}&query={}",
                urlenc(query)
            )
        }
        other => return Err(format!("datadog_query: unsupported query_kind '{other}'")),
    };

    ctx.log(
        "info",
        &format!("monty_repl: querying Datadog, query_kind={query_kind}, limit={limit}"),
    );

    let resp = ctx.http_call("GET", &url, &headers, "")?;
    if resp.status < 200 || resp.status >= 300 {
        return Err(format!(
            "datadog_query failed (HTTP {}): {}",
            resp.status,
            truncate(&resp.body, 1200)
        ));
    }

    let summarized = summarize_datadog_response(query_kind, &resp.body, limit);
    let output = truncate(&summarized, 6_000);
    // Return as JSON value (string content)
    Ok(json!(output))
}

fn datadog_base_url(site: &str) -> String {
    let trimmed = site.trim().trim_end_matches('/');
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else if trimmed.starts_with("api.") {
        format!("https://{trimmed}")
    } else {
        format!("https://api.{trimmed}")
    }
}

fn truncate(body: &str, max_chars: usize) -> String {
    if body.chars().count() <= max_chars {
        return body.to_string();
    }
    let truncated: String = body.chars().take(max_chars).collect();
    format!(
        "{truncated}\n\n[truncated {} chars]",
        body.chars().count().saturating_sub(max_chars)
    )
}

fn summarize_datadog_response(query_kind: &str, body: &str, limit: usize) -> String {
    let Ok(parsed) = serde_json::from_str::<Value>(body) else {
        return body.to_string();
    };

    match query_kind {
        "monitor_status" => {
            return json!({
                "id": parsed.get("id"),
                "name": parsed.get("name"),
                "overall_state": parsed.get("overall_state"),
                "overall_state_modified": parsed.get("overall_state_modified"),
                "priority": parsed.get("priority"),
                "query": parsed.get("query"),
                "message": parsed.get("message"),
                "tags": parsed.get("tags"),
            })
            .to_string();
        }
        "recent_events" => {
            if let Some(events) = parsed.get("events").and_then(Value::as_array) {
                let compact: Vec<Value> = events
                    .iter()
                    .take(limit)
                    .map(|event| {
                        json!({
                            "id": event.get("id"),
                            "date_happened": event.get("date_happened"),
                            "priority": event.get("priority"),
                            "title": event.get("title"),
                            "text": event.get("text"),
                            "source": event.get("source"),
                            "tags": event.get("tags"),
                            "alert_type": event.get("alert_type"),
                        })
                    })
                    .collect();
                return json!({
                    "event_count": events.len(),
                    "events": compact,
                    "truncated": events.len() > compact.len(),
                })
                .to_string();
            }
        }
        "metrics_query" => {
            if let Some(series) = parsed.get("series").and_then(Value::as_array) {
                let compact: Vec<Value> = series
                    .iter()
                    .take(limit)
                    .map(|entry| {
                        json!({
                            "metric": entry.get("metric"),
                            "scope": entry.get("scope"),
                            "interval": entry.get("interval"),
                            "unit": entry.get("unit"),
                            "point_count": entry.get("pointlist").and_then(Value::as_array).map(|pts| pts.len()),
                        })
                    })
                    .collect();
                return json!({
                    "from_date": parsed.get("from_date"),
                    "to_date": parsed.get("to_date"),
                    "series_count": series.len(),
                    "series": compact,
                    "truncated": series.len() > compact.len(),
                })
                .to_string();
            }
        }
        _ => {}
    }

    parsed.to_string()
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
