//! Datadog API tool ported from tool_runner/datadog.rs.
//!
//! Dispatched as `temper.datadog_query(...)` from Monty code.

use serde_json::{Value, json};
use temper_wasm_sdk::context::Context;

#[derive(Debug, PartialEq)]
struct DatadogRequest {
    method: &'static str,
    url: String,
    content_type: &'static str,
    body: Value,
}

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
    let app_url = datadog_app_url(site);
    let request = build_datadog_request(&input, &base_url, &app_url, limit)?;
    let headers = vec![
        ("DD-API-KEY".to_string(), api_key),
        ("DD-APPLICATION-KEY".to_string(), app_key),
        ("accept".to_string(), "application/json".to_string()),
        ("content-type".to_string(), request.content_type.to_string()),
    ];

    ctx.log(
        "info",
        &format!("monty_repl: querying Datadog, query_kind={query_kind}, limit={limit}"),
    );

    let request_body = if request.body.is_null() {
        String::new()
    } else {
        request.body.to_string()
    };
    let resp = ctx.http_call(request.method, &request.url, &headers, &request_body)?;
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

fn datadog_app_url(site: &str) -> String {
    let trimmed = site.trim().trim_end_matches('/');
    let without_scheme = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))
        .unwrap_or(trimmed);
    let bare = without_scheme
        .strip_prefix("api.")
        .or_else(|| without_scheme.strip_prefix("app."))
        .unwrap_or(without_scheme);
    format!("https://app.{bare}")
}

fn build_datadog_request(
    input: &Value,
    base_url: &str,
    app_url: &str,
    limit: usize,
) -> Result<DatadogRequest, String> {
    let query_kind = input
        .get("query_kind")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("monitor_status");

    match query_kind {
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
            Ok(DatadogRequest {
                method: "GET",
                url: format!("{base_url}/api/v1/monitor/{monitor_id}"),
                content_type: "application/json",
                body: Value::Null,
            })
        }
        "recent_events" => {
            let query = input.get("query").and_then(Value::as_str).unwrap_or("");
            let start = input.get("from_ts").and_then(Value::as_i64).unwrap_or(0);
            let end = input
                .get("to_ts")
                .and_then(Value::as_i64)
                .unwrap_or(4_102_444_800);
            let tags = input.get("tags").and_then(Value::as_str).unwrap_or("");
            let mut url = format!("{base_url}/api/v1/events?start={start}&end={end}&unparsed=true");
            if !query.trim().is_empty() {
                url.push_str("&query=");
                url.push_str(&urlenc(query));
            }
            if !tags.trim().is_empty() {
                url.push_str("&tags=");
                url.push_str(&urlenc(tags));
            }
            Ok(DatadogRequest {
                method: "GET",
                url,
                content_type: "application/json",
                body: Value::Null,
            })
        }
        "metrics_query" => build_metric_query_request(input, base_url, None),
        "profiling_query" => build_metric_query_request(
            input,
            base_url,
            Some("sum:datadog.profiling.rust.profiles_uploaded{service:temperpaw}.as_count()"),
        ),
        "logs_query" => Ok(DatadogRequest {
            method: "POST",
            url: format!("{base_url}/api/v2/logs/events/search"),
            content_type: "application/json",
            body: logs_search_body(input, limit),
        }),
        "trace_query" => Ok(DatadogRequest {
            method: "POST",
            url: format!("{base_url}/api/v2/spans/events/search"),
            content_type: "application/json",
            body: spans_search_body(input, limit),
        }),
        "llmobs_query" => Ok(DatadogRequest {
            method: "POST",
            url: format!("{base_url}/api/v2/llm-obs/v1/spans/events/search"),
            content_type: "application/vnd.api+json",
            body: llmobs_search_body(input, limit),
        }),
        "dbm_query" => Ok(DatadogRequest {
            method: "POST",
            url: format!("{app_url}/api/v1/logs-analytics/list?type=databasequery"),
            content_type: "application/json",
            body: dbm_search_body(input, limit),
        }),
        other => Err(format!("datadog_query: unsupported query_kind '{other}'")),
    }
}

fn build_metric_query_request(
    input: &Value,
    base_url: &str,
    default_query: Option<&str>,
) -> Result<DatadogRequest, String> {
    let query = input
        .get("query")
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .or(default_query)
        .ok_or("datadog_query: metrics_query requires query")?;
    let from = input.get("from_ts").and_then(Value::as_i64).unwrap_or(0);
    let to = input
        .get("to_ts")
        .and_then(Value::as_i64)
        .unwrap_or(4_102_444_800);
    Ok(DatadogRequest {
        method: "GET",
        url: format!(
            "{base_url}/api/v1/query?from={from}&to={to}&query={}",
            urlenc(query)
        ),
        content_type: "application/json",
        body: Value::Null,
    })
}

fn logs_search_body(input: &Value, limit: usize) -> Value {
    let mut filter = json!({
        "query": text_arg(input, "query", "*"),
        "from": text_arg(input, "from", "now-15m"),
        "to": text_arg(input, "to", "now"),
    });
    if let Some(indexes) = indexes_arg(input) {
        filter["indexes"] = indexes;
    }
    json!({
        "filter": filter,
        "sort": text_arg(input, "sort", "-timestamp"),
        "page": {"limit": limit},
    })
}

fn spans_search_body(input: &Value, limit: usize) -> Value {
    json!({
        "data": {
            "type": "search_request",
            "attributes": {
                "filter": {
                    "query": text_arg(input, "query", "*"),
                    "from": text_arg(input, "from", "now-15m"),
                    "to": text_arg(input, "to", "now"),
                },
                "options": {
                    "timezone": text_arg(input, "timezone", "GMT"),
                },
                "page": {"limit": limit},
                "sort": text_arg(input, "sort", "-timestamp"),
            }
        }
    })
}

fn llmobs_search_body(input: &Value, limit: usize) -> Value {
    let mut filter = json!({
        "from": text_arg(input, "from", "now-15m"),
        "to": text_arg(input, "to", "now"),
    });
    for key in [
        "query",
        "span_id",
        "trace_id",
        "span_kind",
        "span_name",
        "ml_app",
    ] {
        if let Some(value) = input.get(key).and_then(Value::as_str) {
            if !value.trim().is_empty() {
                filter[key] = json!(value);
            }
        }
    }
    json!({
        "data": {
            "type": "spans",
            "attributes": {
                "filter": filter,
                "options": {
                    "include_attachments": input
                        .get("include_attachments")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                },
                "page": {"limit": limit},
                "sort": text_arg(input, "sort", "-timestamp"),
            }
        }
    })
}

fn dbm_search_body(input: &Value, limit: usize) -> Value {
    let from = input
        .get("from_ms")
        .and_then(Value::as_i64)
        .or_else(|| {
            input
                .get("from_ts")
                .and_then(Value::as_i64)
                .map(|value| value * 1000)
        })
        .unwrap_or(0);
    let to = input
        .get("to_ms")
        .and_then(Value::as_i64)
        .or_else(|| {
            input
                .get("to_ts")
                .and_then(Value::as_i64)
                .map(|value| value * 1000)
        })
        .unwrap_or(4_102_444_800_000);
    json!({
        "list": {
            "indexes": ["databasequery"],
            "limit": limit,
            "search": {
                "query": text_arg(input, "query", "dbm_type:activity service:temperpaw"),
            },
            "sorts": [{"time": {"order": "desc"}}],
            "time": {
                "from": from,
                "to": to,
            }
        }
    })
}

fn text_arg<'a>(input: &'a Value, key: &str, default: &'a str) -> &'a str {
    input
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(default)
}

fn indexes_arg(input: &Value) -> Option<Value> {
    let raw = input.get("indexes")?;
    if let Some(values) = raw.as_array() {
        let indexes = values
            .iter()
            .filter_map(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| json!(value))
            .collect::<Vec<_>>();
        return (!indexes.is_empty()).then(|| json!(indexes));
    }
    raw.as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| {
            json!(
                value
                    .split(',')
                    .map(str::trim)
                    .filter(|part| !part.is_empty())
                    .collect::<Vec<_>>()
            )
        })
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
        "logs_query" => {
            if let Some(logs) = parsed.get("data").and_then(Value::as_array) {
                let compact: Vec<Value> = logs
                    .iter()
                    .take(limit)
                    .map(|log| {
                        let attrs = log.get("attributes").unwrap_or(&Value::Null);
                        json!({
                            "id": log.get("id"),
                            "timestamp": attrs.get("timestamp"),
                            "service": attrs.get("service"),
                            "status": attrs.get("status"),
                            "message": attrs
                                .get("message")
                                .and_then(Value::as_str)
                                .map(|message| truncate(message, 500)),
                            "session_id": value_at(attrs, &["attributes", "session_id"]),
                            "trace_id": value_at(attrs, &["attributes", "dd.trace_id"]),
                            "span_id": value_at(attrs, &["attributes", "dd.span_id"]),
                            "sandbox_operation": value_at(attrs, &["attributes", "sandbox", "operation"]),
                            "modal_bridge_operation": value_at(attrs, &["attributes", "modal_bridge", "operation"]),
                            "gen_ai_operation": value_at(attrs, &["attributes", "gen_ai.operation.name"]),
                            "error_kind": value_at(attrs, &["attributes", "error", "kind"]),
                        })
                    })
                    .collect();
                return json!({
                    "log_count": logs.len(),
                    "logs": compact,
                    "truncated": logs.len() > compact.len(),
                    "meta": parsed.get("meta"),
                })
                .to_string();
            }
        }
        "trace_query" => {
            if let Some(spans) = parsed.get("data").and_then(Value::as_array) {
                let compact: Vec<Value> = spans
                    .iter()
                    .take(limit)
                    .map(|span| {
                        let attrs = span.get("attributes").unwrap_or(&Value::Null);
                        json!({
                            "id": span.get("id"),
                            "service": attrs.get("service"),
                            "operation_name": attrs
                                .get("operation_name")
                                .or_else(|| attrs.get("name")),
                            "resource_name": attrs.get("resource_name"),
                            "trace_id": attrs.get("trace_id"),
                            "span_id": attrs.get("span_id"),
                            "parent_id": attrs.get("parent_id"),
                            "duration": attrs.get("duration"),
                            "status": attrs.get("status"),
                            "session_id": value_at(attrs, &["attributes", "session_id"]),
                            "managed_session_id": value_at(attrs, &["attributes", "managed_session_id"]),
                            "inner_session_id": value_at(attrs, &["attributes", "inner_session_id"]),
                            "tool_name": value_at(attrs, &["attributes", "tool.name"]),
                            "gen_ai_operation": value_at(attrs, &["attributes", "gen_ai.operation.name"]),
                        })
                    })
                    .collect();
                return json!({
                    "span_count": spans.len(),
                    "spans": compact,
                    "truncated": spans.len() > compact.len(),
                    "meta": parsed.get("meta"),
                })
                .to_string();
            }
        }
        "llmobs_query" => {
            if let Some(spans) = parsed.get("data").and_then(Value::as_array) {
                let compact: Vec<Value> = spans
                    .iter()
                    .take(limit)
                    .map(|span| {
                        let attrs = span.get("attributes").unwrap_or(&Value::Null);
                        json!({
                            "id": span.get("id"),
                            "trace_id": attrs.get("trace_id"),
                            "span_id": attrs.get("span_id"),
                            "parent_id": attrs.get("parent_id"),
                            "name": attrs.get("name"),
                            "span_kind": attrs.get("span_kind"),
                            "ml_app": attrs.get("ml_app"),
                            "status": attrs.get("status"),
                            "duration": attrs.get("duration"),
                            "model_provider": attrs
                                .get("model_provider")
                                .or_else(|| attrs.get("provider")),
                            "model_name": attrs
                                .get("model_name")
                                .or_else(|| attrs.get("model")),
                            "session_id": value_at(attrs, &["metadata", "session_id"])
                                .or_else(|| value_at(attrs, &["tags", "session_id"])),
                            "tool_name": value_at(attrs, &["metadata", "tool.name"])
                                .or_else(|| value_at(attrs, &["tags", "tool.name"])),
                        })
                    })
                    .collect();
                return json!({
                    "span_count": spans.len(),
                    "spans": compact,
                    "truncated": spans.len() > compact.len(),
                    "meta": parsed.get("meta"),
                })
                .to_string();
            }
        }
        "dbm_query" => {
            let events = parsed
                .get("result")
                .and_then(|result| result.get("events"))
                .or_else(|| parsed.get("events"))
                .and_then(Value::as_array);
            if let Some(events) = events {
                let compact: Vec<Value> = events
                    .iter()
                    .take(limit)
                    .map(|event| {
                        let event_body = event.get("event").unwrap_or(event);
                        let db = value_at(event_body, &["custom", "db"]).unwrap_or(Value::Null);
                        json!({
                            "id": event.get("id"),
                            "tags": event_body.get("tags"),
                            "statement": db
                                .get("statement")
                                .or_else(|| db.get("query"))
                                .map(|statement| {
                                    statement
                                        .as_str()
                                        .map(|text| json!(truncate(text, 500)))
                                        .unwrap_or_else(|| statement.clone())
                                })
                                .unwrap_or(Value::Null),
                            "query_signature": db.get("query_signature"),
                            "wait_event": db.get("wait_event"),
                            "plan_signature": value_at(&db, &["plan", "signature"]),
                            "service": value_at(event_body, &["custom", "service"]),
                            "trace_id": value_at(event_body, &["custom", "dd", "trace_id"])
                                .or_else(|| value_at(event_body, &["custom", "dd.trace_id"])),
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
        "metrics_query" | "profiling_query" => {
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

fn value_at(value: &Value, path: &[&str]) -> Option<Value> {
    let mut current = value;
    for key in path {
        current = current.get(*key)?;
    }
    Some(current.clone())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn datadog_agent_query_kinds_build_documented_endpoints() {
        let base_url = "https://api.datadoghq.com";
        let app_url = "https://app.datadoghq.com";

        let logs = build_datadog_request(
            &json!({
                "query_kind": "logs_query",
                "query": "service:temperpaw @session_id:ss-1",
                "from": "now-30m",
                "to": "now",
                "limit": 7
            }),
            base_url,
            app_url,
            7,
        )
        .expect("logs request should build");
        assert_eq!(logs.method, "POST");
        assert_eq!(
            logs.url,
            "https://api.datadoghq.com/api/v2/logs/events/search"
        );
        assert_eq!(
            logs.body["filter"]["query"],
            "service:temperpaw @session_id:ss-1"
        );
        assert_eq!(logs.body["page"]["limit"], 7);

        let traces = build_datadog_request(
            &json!({
                "query_kind": "trace_query",
                "query": "service:temperpaw operation_name:temperpaw.agent.session",
            }),
            base_url,
            app_url,
            25,
        )
        .expect("trace request should build");
        assert_eq!(traces.method, "POST");
        assert_eq!(
            traces.url,
            "https://api.datadoghq.com/api/v2/spans/events/search"
        );
        assert_eq!(traces.body["data"]["type"], "search_request");
        assert_eq!(
            traces.body["data"]["attributes"]["filter"]["query"],
            "service:temperpaw operation_name:temperpaw.agent.session"
        );

        let llmobs = build_datadog_request(
            &json!({
                "query_kind": "llmobs_query",
                "ml_app": "temperpaw",
                "span_kind": "agent",
                "include_attachments": false
            }),
            base_url,
            app_url,
            10,
        )
        .expect("LLMObs request should build");
        assert_eq!(llmobs.method, "POST");
        assert_eq!(
            llmobs.url,
            "https://api.datadoghq.com/api/v2/llm-obs/v1/spans/events/search"
        );
        assert_eq!(llmobs.content_type, "application/vnd.api+json");
        assert_eq!(llmobs.body["data"]["type"], "spans");
        assert_eq!(
            llmobs.body["data"]["attributes"]["filter"]["ml_app"],
            "temperpaw"
        );
        assert_eq!(
            llmobs.body["data"]["attributes"]["filter"]["span_kind"],
            "agent"
        );
        assert_eq!(
            llmobs.body["data"]["attributes"]["options"]["include_attachments"],
            false
        );
    }

    #[test]
    fn datadog_dbm_and_profiling_queries_build_agent_usable_requests() {
        let dbm = build_datadog_request(
            &json!({
                "query_kind": "dbm_query",
                "query": "dbm_type:activity service:temperpaw",
                "from_ms": 1778510000000i64,
                "to_ms": 1778513600000i64,
                "limit": 5
            }),
            "https://api.datadoghq.com",
            "https://app.datadoghq.com",
            5,
        )
        .expect("DBM request should build");
        assert_eq!(dbm.method, "POST");
        assert_eq!(
            dbm.url,
            "https://app.datadoghq.com/api/v1/logs-analytics/list?type=databasequery"
        );
        assert_eq!(dbm.body["list"]["indexes"][0], "databasequery");
        assert_eq!(
            dbm.body["list"]["search"]["query"],
            "dbm_type:activity service:temperpaw"
        );
        assert_eq!(dbm.body["list"]["time"]["from"], 1778510000000i64);

        let profiling = build_datadog_request(
            &json!({
                "query_kind": "profiling_query",
                "from_ts": 1778510000i64,
                "to_ts": 1778513600i64,
            }),
            "https://api.datadoghq.com",
            "https://app.datadoghq.com",
            25,
        )
        .expect("profiling request should build");
        assert_eq!(profiling.method, "GET");
        assert!(
            profiling
                .url
                .contains("/api/v1/query?from=1778510000&to=1778513600")
        );
        assert!(
            profiling
                .url
                .contains("datadog.profiling.rust.profiles_uploaded")
        );
    }

    #[test]
    fn datadog_summaries_are_compact_for_agent_diagnostics() {
        let logs_summary = summarize_datadog_response(
            "logs_query",
            r#"{
                "data": [{
                    "id": "log-1",
                    "attributes": {
                        "timestamp": "2026-05-11T20:00:00Z",
                        "service": "temperpaw",
                        "status": "error",
                        "message": "sandbox failed",
                        "attributes": {
                            "session_id": "ss-1",
                            "dd.trace_id": "tr-1",
                            "sandbox": {"operation": "bash"}
                        }
                    }
                }],
                "meta": {"status": "done"}
            }"#,
            5,
        );
        let parsed: Value = serde_json::from_str(&logs_summary).expect("logs summary json");
        assert_eq!(parsed["log_count"], 1);
        assert_eq!(parsed["logs"][0]["session_id"], "ss-1");
        assert_eq!(parsed["logs"][0]["trace_id"], "tr-1");

        let trace_summary = summarize_datadog_response(
            "trace_query",
            r#"{
                "data": [{
                    "id": "span-1",
                    "attributes": {
                        "service": "temperpaw",
                        "operation_name": "temperpaw.agent.session",
                        "resource_name": "ManagedAgents.StartSession",
                        "trace_id": "tr-1",
                        "span_id": "sp-1",
                        "parent_id": "root",
                        "duration": 1200000,
                        "status": "ok",
                        "attributes": {"session_id": "ss-1"}
                    }
                }]
            }"#,
            5,
        );
        let parsed: Value = serde_json::from_str(&trace_summary).expect("trace summary json");
        assert_eq!(parsed["span_count"], 1);
        assert_eq!(
            parsed["spans"][0]["operation_name"],
            "temperpaw.agent.session"
        );
        assert_eq!(parsed["spans"][0]["session_id"], "ss-1");

        let llmobs_summary = summarize_datadog_response(
            "llmobs_query",
            r#"{
                "data": [{
                    "id": "llm-1",
                    "attributes": {
                        "trace_id": "tr-1",
                        "span_id": "sp-1",
                        "parent_id": "",
                        "name": "temperpaw.agent.session",
                        "span_kind": "agent",
                        "ml_app": "temperpaw",
                        "status": "ok",
                        "duration": 1400000,
                        "model_name": "gpt-5.5",
                        "model_provider": "openai",
                        "metadata": {"session_id": "ss-1"}
                    }
                }]
            }"#,
            5,
        );
        let parsed: Value = serde_json::from_str(&llmobs_summary).expect("llmobs summary json");
        assert_eq!(parsed["span_count"], 1);
        assert_eq!(parsed["spans"][0]["span_kind"], "agent");
        assert_eq!(parsed["spans"][0]["ml_app"], "temperpaw");

        let dbm_summary = summarize_datadog_response(
            "dbm_query",
            r#"{
                "result": {
                    "events": [{
                        "id": "dbm-1",
                        "event": {
                            "custom": {
                                "db": {
                                    "statement": "select * from sessions",
                                    "query_signature": "abc123",
                                    "wait_event": "Lock",
                                    "plan": {"signature": "plan-1"}
                                }
                            },
                            "tags": ["service:temperpaw"]
                        }
                    }]
                }
            }"#,
            5,
        );
        let parsed: Value = serde_json::from_str(&dbm_summary).expect("dbm summary json");
        assert_eq!(parsed["event_count"], 1);
        assert_eq!(parsed["events"][0]["query_signature"], "abc123");
        assert_eq!(parsed["events"][0]["plan_signature"], "plan-1");
    }
}
