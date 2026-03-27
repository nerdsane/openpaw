//! Tool Runner — WASM module for executing tool calls in a sandbox.
//!
//! Reads pending_tool_calls from trigger params, executes each tool via
//! HTTP calls to the sandbox API, and returns tool results as callback params.
//!
//! Build: `cargo build --target wasm32-unknown-unknown --release`

use std::collections::BTreeMap;
use temper_wasm_sdk::prelude::*;

/// Entry point.
#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        ctx.log("info", "tool_runner: starting");

        let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));

        let sandbox_url = fields
            .get("sandbox_url")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if sandbox_url.is_empty() {
            return Err("sandbox_url is empty — cannot execute tools".to_string());
        }

        let workdir = fields
            .get("workdir")
            .and_then(|v| v.as_str())
            .unwrap_or("/workspace");

        // Read pending tool calls from trigger params
        let tool_calls_json = ctx
            .trigger_params
            .get("pending_tool_calls")
            .and_then(|v| v.as_str())
            .unwrap_or("[]");

        let tool_calls: Vec<Value> = serde_json::from_str(tool_calls_json)
            .map_err(|e| format!("failed to parse pending_tool_calls: {e}"))?;

        ctx.log(
            "info",
            &format!("tool_runner: executing {} tool calls", tool_calls.len()),
        );

        // Execute each tool call and collect results
        let mut tool_results: Vec<Value> = Vec::new();

        for call in &tool_calls {
            let tool_id = call.get("id").and_then(|v| v.as_str()).unwrap_or("unknown");
            let tool_name = call
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let input = call.get("input").cloned().unwrap_or(json!({}));

            ctx.log(
                "info",
                &format!("tool_runner: executing tool '{tool_name}' id={tool_id}"),
            );

            let result = execute_tool(&ctx, sandbox_url, workdir, tool_name, &input);

            let (content, is_error) = match result {
                Ok(output) => (output, false),
                Err(e) => (format!("Error: {e}"), true),
            };

            tool_results.push(json!({
                "type": "tool_result",
                "tool_use_id": tool_id,
                "content": content,
                "is_error": is_error,
            }));
        }

        // TemperFS conversation storage
        let conversation_file_id = fields
            .get("conversation_file_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        // Temper API URL: prefer Configure override in state, then integration config.
        let temper_api_url = resolve_temper_api_url(&ctx, &fields);
        let tenant = &ctx.tenant;

        // Read current conversation and append tool results
        let mut messages: Vec<Value> = if !conversation_file_id.is_empty() {
            read_conversation_from_temperfs(&ctx, &temper_api_url, tenant, conversation_file_id)?
        } else {
            let conversation_json = fields
                .get("conversation")
                .and_then(|v| v.as_str())
                .unwrap_or("[]");
            serde_json::from_str(conversation_json).unwrap_or_default()
        };

        // Append tool results as a user message (Anthropic API format)
        messages.push(json!({
            "role": "user",
            "content": tool_results,
        }));

        // Write back to TemperFS or pass inline
        let updated_conversation = serde_json::to_string(&messages).unwrap_or_default();
        if !conversation_file_id.is_empty() {
            let body = format!("{{\"messages\":{updated_conversation}}}");
            let url = format!("{temper_api_url}/tdata/Files('{conversation_file_id}')/$value");
            let headers = vec![
                ("content-type".to_string(), "application/json".to_string()),
                ("x-tenant-id".to_string(), tenant.to_string()),
                ("x-temper-principal-kind".to_string(), "system".to_string()),
            ];
            match ctx.http_call("PUT", &url, &headers, &body) {
                Ok(resp) if resp.status >= 200 && resp.status < 300 => {
                    ctx.log(
                        "info",
                        &format!(
                            "tool_runner: wrote conversation to TemperFS ({} bytes)",
                            body.len()
                        ),
                    );
                }
                Ok(resp) => {
                    return Err(format!(
                        "TemperFS conversation write failed (HTTP {}): {}",
                        resp.status,
                        &resp.body[..resp.body.len().min(200)]
                    ));
                }
                Err(e) => {
                    return Err(format!("TemperFS conversation write failed: {e}"));
                }
            }
        }

        // Fsync sandbox files to TemperFS (best-effort)
        let file_manifest_id = fields
            .get("file_manifest_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let workspace_id = fields
            .get("workspace_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let max_sync_file_bytes: u64 = ctx
            .config
            .get("max_sync_file_bytes")
            .and_then(|s| s.parse().ok())
            .unwrap_or(61440);
        let sync_exclude = ctx.config.get("sync_exclude").cloned().unwrap_or_default();

        if !file_manifest_id.is_empty() && !workspace_id.is_empty() {
            let e2b = is_e2b_sandbox(sandbox_url);
            match sync_files_to_temperfs(
                &ctx,
                sandbox_url,
                &temper_api_url,
                tenant,
                workspace_id,
                file_manifest_id,
                workdir,
                e2b,
                max_sync_file_bytes,
                &sync_exclude,
            ) {
                Ok(count) => ctx.log(
                    "info",
                    &format!("tool_runner: fsync complete ({count} files synced)"),
                ),
                Err(e) => ctx.log(
                    "warn",
                    &format!("tool_runner: fsync failed (non-fatal): {e}"),
                ),
            }
        }

        let results_json = serde_json::to_string(&tool_results).unwrap_or_default();
        let mut params = json!({
            "pending_tool_calls": results_json,
        });
        if conversation_file_id.is_empty() {
            params["conversation"] = json!(updated_conversation);
        }
        set_success_result("HandleToolResults", &params);

        Ok(())
    })();

    if let Err(e) = result {
        set_error_result(&e);
    }
    0
}

/// Detect whether the sandbox is E2B (envd daemon) based on the URL.
fn is_e2b_sandbox(sandbox_url: &str) -> bool {
    sandbox_url.contains("e2b.app") || sandbox_url.contains("e2b.dev")
}

/// Execute a single tool call against the sandbox API.
/// Supports both local sandbox API (/v1/fs/file, /v1/processes/run)
/// and E2B envd API (/files, Connect protocol for processes).
fn execute_tool(
    ctx: &Context,
    sandbox_url: &str,
    workdir: &str,
    tool_name: &str,
    input: &Value,
) -> Result<String, String> {
    let e2b = is_e2b_sandbox(sandbox_url);
    match tool_name {
        "read" => {
            let path = input
                .get("path")
                .and_then(|v| v.as_str())
                .ok_or("read: missing 'path' parameter")?;

            let full_path = resolve_path(workdir, path);
            if e2b {
                read_file_e2b(ctx, sandbox_url, &full_path)
            } else {
                read_file_local(ctx, sandbox_url, &full_path)
            }
        }
        "write" => {
            let path = input
                .get("path")
                .and_then(|v| v.as_str())
                .ok_or("write: missing 'path' parameter")?;
            let content = input
                .get("content")
                .and_then(|v| v.as_str())
                .ok_or("write: missing 'content' parameter")?;

            let full_path = resolve_path(workdir, path);
            if e2b {
                write_file_e2b(ctx, sandbox_url, &full_path, content)
            } else {
                write_file_local(ctx, sandbox_url, &full_path, content)
            }
        }
        "edit" => {
            let path = input
                .get("path")
                .and_then(|v| v.as_str())
                .ok_or("edit: missing 'path' parameter")?;
            let old_string = input
                .get("old_string")
                .and_then(|v| v.as_str())
                .ok_or("edit: missing 'old_string' parameter")?;
            let new_string = input
                .get("new_string")
                .and_then(|v| v.as_str())
                .ok_or("edit: missing 'new_string' parameter")?;

            let full_path = resolve_path(workdir, path);
            // Read current file
            let current = if e2b {
                read_file_e2b(ctx, sandbox_url, &full_path)?
            } else {
                read_file_local(ctx, sandbox_url, &full_path)?
            };

            if !current.contains(old_string) {
                return Err(format!("edit: old_string not found in {full_path}"));
            }
            let updated = current.replacen(old_string, new_string, 1);

            // Write updated file
            if e2b {
                write_file_e2b(ctx, sandbox_url, &full_path, &updated)?;
            } else {
                write_file_local(ctx, sandbox_url, &full_path, &updated)?;
            }
            Ok(format!("File edited: {full_path}"))
        }
        "bash" => {
            let command = input
                .get("command")
                .and_then(|v| v.as_str())
                .ok_or("bash: missing 'command' parameter")?;

            if e2b {
                run_bash_e2b(ctx, sandbox_url, command, workdir)
            } else {
                run_bash_local(ctx, sandbox_url, command, workdir)
            }
        }
        "logfire_query" => query_logfire(ctx, input),
        unknown => Err(format!("unknown tool: {unknown}")),
    }
}

fn query_logfire(ctx: &Context, input: &Value) -> Result<String, String> {
    let sql = input
        .get("sql")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .or_else(|| build_logfire_sql(input).ok())
        .ok_or("logfire_query: provide either 'sql' or a supported 'query_kind'")?;

    let limit = input
        .get("limit")
        .and_then(Value::as_u64)
        .unwrap_or(50)
        .clamp(1, 200);
    let row_oriented = input
        .get("row_oriented")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let min_timestamp = input.get("min_timestamp").and_then(Value::as_str);
    let max_timestamp = input.get("max_timestamp").and_then(Value::as_str);
    let query_kind = input.get("query_kind").and_then(Value::as_str).unwrap_or("sql");

    let base_url = normalize_logfire_base_url(
        ctx.config
            .get("logfire_api_base")
            .map(String::as_str)
            .unwrap_or("https://logfire-us.pydantic.dev"),
    );
    let read_token = ctx
        .config
        .get("logfire_read_token")
        .cloned()
        .unwrap_or_default();
    if read_token.trim().is_empty() || read_token.contains("{secret:") {
        return Err(
            "logfire_query: missing Logfire read token; configure logfire_read_token secret"
                .to_string(),
        );
    }

    let mut url = format!(
        "{base_url}/v1/query?sql={}&limit={limit}&row_oriented={}",
        url_encode(&sql),
        if row_oriented { "true" } else { "false" }
    );
    if let Some(value) = min_timestamp.filter(|s| !s.trim().is_empty()) {
        url.push_str("&min_timestamp=");
        url.push_str(&url_encode(value));
    }
    if let Some(value) = max_timestamp.filter(|s| !s.trim().is_empty()) {
        url.push_str("&max_timestamp=");
        url.push_str(&url_encode(value));
    }

    ctx.log(
        "info",
        &format!(
            "tool_runner: querying Logfire, query_kind={query_kind}, limit={limit}, row_oriented={row_oriented}"
        ),
    );

    let headers = vec![
        ("authorization".to_string(), format!("Bearer {read_token}")),
        ("accept".to_string(), "application/json".to_string()),
    ];
    let resp = ctx.http_call("GET", &url, &headers, "")?;
    if resp.status < 200 || resp.status >= 300 {
        return Err(format!(
            "logfire_query failed (HTTP {}): {}",
            resp.status,
            truncate_tool_output(&resp.body, 1200)
        ));
    }

    let summarized = summarize_logfire_response(&resp.body, limit as usize);
    Ok(truncate_tool_output(&summarized, 6_000))
}

fn build_logfire_sql(input: &Value) -> Result<String, String> {
    let query_kind = input
        .get("query_kind")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or("logfire_query: missing 'query_kind'")?;
    let query_kind = normalize_query_kind(query_kind);
    let limit = input
        .get("limit")
        .and_then(Value::as_u64)
        .unwrap_or(25)
        .clamp(1, 200);
    let service_name = input
        .get("service_name")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("temper-platform");
    let lookback_minutes = input
        .get("lookback_minutes")
        .and_then(Value::as_u64)
        .unwrap_or(240)
        .clamp(1, 10_080);
    let environment = input.get("environment").and_then(Value::as_str);
    let entity_type = input.get("entity_type").and_then(Value::as_str);
    let action = input.get("action").and_then(Value::as_str);
    let intent_text = input.get("intent_text").and_then(Value::as_str);
    let agent_id = input.get("agent_id").and_then(Value::as_str);

    let mut filters = vec![format!("service_name = {}", sql_string(service_name))];
    filters.push(format!(
        "start_timestamp >= now() - INTERVAL '{} minutes'",
        lookback_minutes
    ));
    if let Some(environment) = environment.filter(|value| !value.trim().is_empty()) {
        filters.push(format!(
            "deployment_environment = {}",
            sql_string(environment)
        ));
    }
    if let Some(entity_type) = entity_type.filter(|value| !value.trim().is_empty()) {
        let pattern = format!("%{entity_type}%");
        filters.push(format!(
            "(attributes->>'resource_type' = {value} OR attributes->>'entity_type' = {value} OR message ILIKE {pattern})",
            value = sql_string(entity_type),
            pattern = sql_string(&pattern),
        ));
    }
    if let Some(action) = action.filter(|value| !value.trim().is_empty()) {
        let pattern = format!("%{action}%");
        filters.push(format!(
            "(attributes->>'action' = {value} OR message ILIKE {pattern})",
            value = sql_string(action),
            pattern = sql_string(&pattern),
        ));
    }
    if let Some(intent_text) = intent_text.filter(|value| !value.trim().is_empty()) {
        let pattern = format!("%{intent_text}%");
        filters.push(format!("message ILIKE {}", sql_string(&pattern)));
    }
    if let Some(agent_id) = agent_id.filter(|value| !value.trim().is_empty()) {
        let pattern = format!("%{agent_id}%");
        filters.push(format!(
            "(attributes->>'agent_id' = {value} OR message ILIKE {pattern})",
            value = sql_string(agent_id),
            pattern = sql_string(&pattern),
        ));
    }

    let where_clause = filters.join("\n  AND ");
    let sql = match query_kind {
        "intent_failure_cluster" => format!(
            "SELECT\n  message,\n  coalesce(attributes->>'action', '') AS action,\n  coalesce(attributes->>'resource_type', attributes->>'entity_type', '') AS resource_type,\n  coalesce(attributes->>'decision', '') AS decision,\n  count(*) AS event_count,\n  max(start_timestamp) AS last_seen\nFROM records\nWHERE {where_clause}\n  AND (\n    message ILIKE '%unmet_intent%'\n    OR message ILIKE '%authz.%'\n    OR attributes->>'decision' = 'Deny'\n    OR message ILIKE '%failed%'\n  )\nGROUP BY message, action, resource_type, decision\nORDER BY event_count DESC, last_seen DESC\nLIMIT {limit}"
        ),
        "workflow_retries" => format!(
            "SELECT\n  start_timestamp,\n  message,\n  coalesce(attributes->>'action', '') AS action,\n  coalesce(attributes->>'resource_type', attributes->>'entity_type', '') AS resource_type,\n  coalesce(attributes->>'temper.from_status', '') AS from_status,\n  coalesce(attributes->>'temper.to_status', '') AS to_status,\n  coalesce(attributes->>'decision', '') AS decision\nFROM records\nWHERE {where_clause}\n  AND (\n    message ILIKE '%trajectory%'\n    OR message ILIKE '%dispatch%'\n    OR message ILIKE '%unmet_intent%'\n    OR attributes->>'action' IS NOT NULL\n  )\nORDER BY start_timestamp DESC\nLIMIT {limit}"
        ),
        "alternate_success_paths" => format!(
            "SELECT\n  start_timestamp,\n  message,\n  coalesce(attributes->>'action', '') AS action,\n  coalesce(attributes->>'resource_type', attributes->>'entity_type', '') AS resource_type,\n  coalesce(attributes->>'temper.from_status', '') AS from_status,\n  coalesce(attributes->>'temper.to_status', '') AS to_status,\n  coalesce(attributes->>'decision', '') AS decision\nFROM records\nWHERE {where_clause}\n  AND (\n    message ILIKE '%trajectory%'\n    OR message ILIKE '%unmet_intent%'\n    OR message ILIKE '%authz.%'\n    OR attributes->>'action' IS NOT NULL\n  )\nORDER BY start_timestamp DESC\nLIMIT {limit}"
        ),
        "intent_abandonment" => format!(
            "SELECT\n  coalesce(attributes->>'action', message) AS activity,\n  count(*) AS failed_event_count,\n  max(start_timestamp) AS last_seen\nFROM records\nWHERE {where_clause}\n  AND (\n    message ILIKE '%unmet_intent%'\n    OR message ILIKE '%authz.%'\n    OR attributes->>'decision' = 'Deny'\n    OR message ILIKE '%failed%'\n  )\nGROUP BY activity\nORDER BY failed_event_count DESC, last_seen DESC\nLIMIT {limit}"
        ),
        "recent_events" => format!(
            "SELECT start_timestamp, message, attributes\nFROM records\nWHERE {where_clause}\nORDER BY start_timestamp DESC\nLIMIT {limit}"
        ),
        other => return Err(format!("logfire_query: unsupported query_kind '{other}'")),
    };

    Ok(sql)
}

fn normalize_query_kind(query_kind: &str) -> &str {
    match query_kind {
        "workaround" => "alternate_success_paths",
        "governance_gap" => "intent_failure_cluster",
        other => other,
    }
}

fn sql_string(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn normalize_logfire_base_url(base: &str) -> String {
    let trimmed = base.trim().trim_end_matches('/');
    if trimmed.ends_with("/v1/query") {
        trimmed.trim_end_matches("/v1/query").to_string()
    } else {
        trimmed.to_string()
    }
}

fn truncate_tool_output(body: &str, max_chars: usize) -> String {
    if body.chars().count() <= max_chars {
        return body.to_string();
    }
    let truncated: String = body.chars().take(max_chars).collect();
    format!(
        "{truncated}\n\n[truncated {} chars; refine the query with a tighter filter or lower limit]",
        body.chars().count().saturating_sub(max_chars)
    )
}

fn summarize_logfire_response(body: &str, limit: usize) -> String {
    let Ok(parsed) = serde_json::from_str::<Value>(body) else {
        return body.to_string();
    };

    if let Some(rows) = parsed.get("rows").and_then(Value::as_array) {
        let compact_rows: Vec<Value> = rows
            .iter()
            .take(limit.min(8))
            .map(compact_logfire_row)
            .collect();
        return json!({
            "row_count": rows.len(),
            "rows": compact_rows,
            "truncated": rows.len() > compact_rows.len()
        })
        .to_string();
    }

    if let Some(columns) = parsed.get("columns").and_then(Value::as_array) {
        let rows = rows_from_columnar(columns, limit.min(8));
        let row_count = columnar_row_count(columns);
        return json!({
            "row_count": row_count,
            "rows": rows,
            "truncated": row_count > rows.len()
        })
        .to_string();
    }

    parsed.to_string()
}

fn columnar_row_count(columns: &[Value]) -> usize {
    columns
        .iter()
        .filter_map(|column| {
            column
                .get("values")
                .and_then(Value::as_array)
                .map(std::vec::Vec::len)
        })
        .max()
        .unwrap_or(0)
}

fn rows_from_columnar(columns: &[Value], row_limit: usize) -> Vec<Value> {
    let row_count = columnar_row_count(columns);
    let mut rows = Vec::new();
    for row_index in 0..row_count.min(row_limit) {
        let mut row = serde_json::Map::new();
        for column in columns {
            let Some(name) = column.get("name").and_then(Value::as_str) else {
                continue;
            };
            let Some(values) = column.get("values").and_then(Value::as_array) else {
                continue;
            };
            if let Some(value) = values.get(row_index)
                && !value.is_null()
            {
                row.insert(name.to_string(), value.clone());
            }
        }
        rows.push(compact_logfire_row(&Value::Object(row)));
    }
    rows
}

fn compact_logfire_row(row: &Value) -> Value {
    let Some(obj) = row.as_object() else {
        return row.clone();
    };

    let mut compact = serde_json::Map::new();
    for key in [
        "start_timestamp",
        "created_at",
        "last_seen",
        "message",
        "span_name",
        "activity",
        "action",
        "resource_type",
        "decision",
        "service_name",
        "deployment_environment",
        "event_count",
        "failed_event_count",
        "duration",
    ] {
        if let Some(value) = obj.get(key)
            && !value.is_null()
            && !value.as_str().is_some_and(str::is_empty)
        {
            compact.insert(key.to_string(), value.clone());
        }
    }

    if let Some(attributes) = obj.get("attributes").and_then(Value::as_object) {
        copy_attribute(attributes, &mut compact, "action", "action");
        copy_attribute(attributes, &mut compact, "resource_type", "resource_type");
        copy_attribute(attributes, &mut compact, "entity_type", "entity_type");
        copy_attribute(attributes, &mut compact, "decision", "decision");
        copy_attribute(attributes, &mut compact, "agent_id", "agent_id");
        copy_attribute(attributes, &mut compact, "tenant", "tenant");
        copy_attribute(attributes, &mut compact, "temper.from_status", "from_status");
        copy_attribute(attributes, &mut compact, "temper.to_status", "to_status");
    }

    Value::Object(compact)
}

fn copy_attribute(
    attributes: &serde_json::Map<String, Value>,
    compact: &mut serde_json::Map<String, Value>,
    source_key: &str,
    target_key: &str,
) {
    if compact.contains_key(target_key) {
        return;
    }
    let Some(value) = attributes.get(source_key) else {
        return;
    };
    if value.is_null() || value.as_str().is_some_and(str::is_empty) {
        return;
    }
    compact.insert(target_key.to_string(), value.clone());
}

// --- Local sandbox API (our custom HTTP server) ---

/// Read file via local sandbox API.
fn read_file_local(ctx: &Context, sandbox_url: &str, full_path: &str) -> Result<String, String> {
    let url = format!("{sandbox_url}/v1/fs/file?path={}", url_encode(full_path));
    let resp = ctx.http_get(&url)?;
    if resp.status == 200 {
        Ok(resp.body)
    } else {
        Err(format!("read failed (HTTP {}): {}", resp.status, resp.body))
    }
}

/// Write file via local sandbox API.
fn write_file_local(
    ctx: &Context,
    sandbox_url: &str,
    full_path: &str,
    content: &str,
) -> Result<String, String> {
    let url = format!("{sandbox_url}/v1/fs/file?path={}", url_encode(full_path));
    let headers = vec![("content-type".to_string(), "text/plain".to_string())];
    let resp = ctx.http_call("PUT", &url, &headers, content)?;
    if resp.status >= 200 && resp.status < 300 {
        Ok(format!("File written: {full_path}"))
    } else {
        Err(format!(
            "write failed (HTTP {}): {}",
            resp.status, resp.body
        ))
    }
}

/// Run bash command via local sandbox API.
fn run_bash_local(
    ctx: &Context,
    sandbox_url: &str,
    command: &str,
    workdir: &str,
) -> Result<String, String> {
    let url = format!("{sandbox_url}/v1/processes/run");
    let body = serde_json::to_string(&json!({
        "command": command,
        "workdir": workdir,
    }))
    .unwrap_or_default();

    let headers = vec![("content-type".to_string(), "application/json".to_string())];
    let resp = ctx.http_call("POST", &url, &headers, &body)?;

    if resp.status >= 200 && resp.status < 300 {
        if let Ok(parsed) = serde_json::from_str::<Value>(&resp.body) {
            let stdout = parsed.get("stdout").and_then(|v| v.as_str()).unwrap_or("");
            let stderr = parsed.get("stderr").and_then(|v| v.as_str()).unwrap_or("");
            let exit_code = parsed
                .get("exit_code")
                .and_then(|v| v.as_i64())
                .unwrap_or(-1);

            let mut output = String::new();
            if !stdout.is_empty() {
                output.push_str(stdout);
            }
            if !stderr.is_empty() {
                if !output.is_empty() {
                    output.push('\n');
                }
                output.push_str("STDERR: ");
                output.push_str(stderr);
            }
            if exit_code != 0 {
                output.push_str(&format!("\n(exit code: {exit_code})"));
            }
            Ok(output)
        } else {
            Ok(resp.body)
        }
    } else {
        Err(format!("bash failed (HTTP {}): {}", resp.status, resp.body))
    }
}

// --- E2B envd API (plain HTTP for files, port 49983) ---

/// Read file via E2B envd HTTP API: GET /files?path=...
fn read_file_e2b(ctx: &Context, sandbox_url: &str, full_path: &str) -> Result<String, String> {
    let url = format!("{sandbox_url}/files?path={}", url_encode(full_path));
    let resp = ctx.http_get(&url)?;
    if resp.status == 200 {
        Ok(resp.body)
    } else {
        Err(format!(
            "E2B read failed (HTTP {}): {}",
            resp.status, resp.body
        ))
    }
}

/// Write file via E2B envd HTTP API: POST /files?path=<full_path> with multipart file.
/// The E2B envd expects `path` as a query parameter (full file path) and the file
/// content as a multipart form-data upload with field name "file".
fn write_file_e2b(
    ctx: &Context,
    sandbox_url: &str,
    full_path: &str,
    content: &str,
) -> Result<String, String> {
    let url = format!("{sandbox_url}/files?path={}", url_encode(full_path));
    let boundary = "----TemperWasmBoundary7MA4YWxkTrZu0gW";
    let body = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{full_path}\"\r\nContent-Type: application/octet-stream\r\n\r\n{content}\r\n--{boundary}--\r\n"
    );

    let headers = vec![(
        "content-type".to_string(),
        format!("multipart/form-data; boundary={boundary}"),
    )];
    let resp = ctx.http_call("POST", &url, &headers, &body)?;
    if resp.status >= 200 && resp.status < 300 {
        Ok(format!("File written: {full_path}"))
    } else {
        Err(format!(
            "E2B write failed (HTTP {}): {}",
            resp.status, resp.body
        ))
    }
}

/// Run bash command via E2B envd Connect protocol: POST /process.Process/Start.
///
/// Uses the `host_connect_call` host function which handles Connect binary
/// frame parsing. The envd daemon returns server-streamed process output
/// frames, each containing stdout/stderr/exitCode fields.
fn run_bash_e2b(
    ctx: &Context,
    sandbox_url: &str,
    command: &str,
    workdir: &str,
) -> Result<String, String> {
    let url = format!("{sandbox_url}/process.Process/Start");
    let body = serde_json::to_string(&json!({
        "command": command,
        "envs": {},
        "cwd": workdir,
    }))
    .unwrap_or_default();

    let headers: Vec<(String, String)> = Vec::new();
    let frames = ctx.connect_call(&url, &headers, &body)?;

    if frames.is_empty() {
        return Ok(String::new());
    }

    // Parse each frame — E2B process output has stdout, stderr, exitCode fields
    let mut stdout = String::new();
    let mut stderr = String::new();
    let mut exit_code: i64 = 0;

    for frame_str in &frames {
        if let Ok(frame) = serde_json::from_str::<Value>(frame_str) {
            if let Some(event) = frame.get("event") {
                // Connect-streamed events may nest the data
                if let Some(s) = event.get("stdout").and_then(|v| v.as_str()) {
                    stdout.push_str(s);
                }
                if let Some(s) = event.get("stderr").and_then(|v| v.as_str()) {
                    stderr.push_str(s);
                }
                if let Some(c) = event.get("exitCode").and_then(|v| v.as_i64()) {
                    exit_code = c;
                }
            } else {
                // Direct fields
                if let Some(s) = frame.get("stdout").and_then(|v| v.as_str()) {
                    stdout.push_str(s);
                }
                if let Some(s) = frame.get("stderr").and_then(|v| v.as_str()) {
                    stderr.push_str(s);
                }
                if let Some(c) = frame.get("exitCode").and_then(|v| v.as_i64()) {
                    exit_code = c;
                }
            }
        }
    }

    let mut output = String::new();
    if !stdout.is_empty() {
        output.push_str(&stdout);
    }
    if !stderr.is_empty() {
        if !output.is_empty() {
            output.push('\n');
        }
        output.push_str("STDERR: ");
        output.push_str(&stderr);
    }
    if exit_code != 0 {
        output.push_str(&format!("\n(exit code: {exit_code})"));
    }
    Ok(output)
}

/// Read conversation from TemperFS File entity.
fn read_conversation_from_temperfs(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    file_id: &str,
) -> Result<Vec<Value>, String> {
    let url = format!("{temper_api_url}/tdata/Files('{file_id}')/$value");
    let headers = vec![
        ("x-tenant-id".to_string(), tenant.to_string()),
        ("x-temper-principal-kind".to_string(), "system".to_string()),
        ("accept".to_string(), "application/json".to_string()),
    ];

    let resp = ctx
        .http_call("GET", &url, &headers, "")
        .map_err(|e| format!("TemperFS conversation read failed: {e}"))?;

    if resp.status != 200 {
        return Err(format!(
            "TemperFS conversation read failed (HTTP {}): {}",
            resp.status,
            &resp.body[..resp.body.len().min(200)]
        ));
    }

    let parsed: Value = serde_json::from_str(&resp.body)
        .map_err(|e| format!("TemperFS conversation parse failed: {e}"))?;

    Ok(parsed
        .get("messages")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default())
}

/// Resolve a path relative to the working directory.
fn resolve_path(workdir: &str, path: &str) -> String {
    if path.starts_with('/') {
        path.to_string()
    } else {
        format!("{}/{}", workdir.trim_end_matches('/'), path)
    }
}

/// Minimal URL encoding for path parameters.
fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 2);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'/' => {
                out.push(b as char);
            }
            _ => {
                out.push_str(&format!("%{b:02X}"));
            }
        }
    }
    out
}

// --- Sandbox Fsync to TemperFS ---

/// File metadata from sandbox `find` + `stat`.
struct FileEntry {
    size_bytes: u64,
    mtime: u64,
}

/// Manifest entry stored in TemperFS.
struct ManifestEntry {
    file_id: String,
    size_bytes: u64,
    mtime: u64,
}

/// Enumerate all files in the sandbox workspace using `find` + `stat`.
/// Returns a map of path → FileEntry with size and mtime.
fn enumerate_sandbox_files(
    ctx: &Context,
    sandbox_url: &str,
    workdir: &str,
    exclude: &str,
    e2b: bool,
) -> Result<BTreeMap<String, FileEntry>, String> {
    // Build exclude flags from comma-separated patterns
    let mut exclude_flags = String::new();
    for pattern in exclude.split(',') {
        let pattern = pattern.trim();
        if !pattern.is_empty() {
            exclude_flags.push_str(&format!(" -not -path '*/{pattern}/*'"));
        }
    }

    // Use stat format appropriate for the OS
    let stat_fmt = if e2b {
        // Linux/GNU stat: %n=name %s=size %Y=mtime
        "-exec stat --format='%n %s %Y' {} +"
    } else {
        // macOS/BSD stat: %N=name %z=size %m=mtime
        "-exec stat -f '%N %z %m' {} +"
    };

    let command = format!("find {workdir} -type f -not -path '*/.*'{exclude_flags} {stat_fmt}");

    let output = if e2b {
        run_bash_e2b(ctx, sandbox_url, &command, workdir)?
    } else {
        run_bash_local(ctx, sandbox_url, &command, workdir)?
    };

    let mut files = BTreeMap::new();
    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("STDERR:") {
            continue;
        }
        // Parse: "path size mtime" — split from the right since path may contain spaces
        let parts: Vec<&str> = line.rsplitn(3, ' ').collect();
        if parts.len() == 3 {
            let mtime: u64 = parts[0].parse().unwrap_or(0);
            let size_bytes: u64 = parts[1].parse().unwrap_or(0);
            let path = parts[2].to_string();
            files.insert(path, FileEntry { size_bytes, mtime });
        }
    }

    Ok(files)
}

/// Read the file manifest from TemperFS.
fn read_manifest(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    manifest_file_id: &str,
) -> Result<BTreeMap<String, ManifestEntry>, String> {
    let url = format!("{temper_api_url}/tdata/Files('{manifest_file_id}')/$value");
    let headers = vec![
        ("x-tenant-id".to_string(), tenant.to_string()),
        ("x-temper-principal-kind".to_string(), "system".to_string()),
        ("accept".to_string(), "application/json".to_string()),
    ];

    let resp = ctx.http_call("GET", &url, &headers, "")?;
    if resp.status != 200 {
        return Ok(BTreeMap::new());
    }

    let parsed: Value = serde_json::from_str(&resp.body).unwrap_or(json!({}));
    let files_obj = parsed.get("files").and_then(|v| v.as_object());

    let mut manifest = BTreeMap::new();
    if let Some(files) = files_obj {
        for (path, entry) in files {
            let file_id = entry
                .get("file_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let size_bytes = entry
                .get("size_bytes")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let mtime = entry.get("mtime").and_then(|v| v.as_u64()).unwrap_or(0);
            if !file_id.is_empty() {
                manifest.insert(
                    path.clone(),
                    ManifestEntry {
                        file_id,
                        size_bytes,
                        mtime,
                    },
                );
            }
        }
    }

    Ok(manifest)
}

/// Simple hash function for deterministic File entity IDs.
/// Returns first 16 hex chars of a djb2 hash.
fn simple_hash(input: &str) -> String {
    let mut hash: u64 = 5381;
    for b in input.bytes() {
        hash = hash.wrapping_mul(33).wrapping_add(b as u64);
    }
    format!("{hash:016x}")
}

/// Sync sandbox files to TemperFS. Returns the number of files synced.
fn sync_files_to_temperfs(
    ctx: &Context,
    sandbox_url: &str,
    temper_api_url: &str,
    tenant: &str,
    workspace_id: &str,
    manifest_file_id: &str,
    workdir: &str,
    e2b: bool,
    max_file_bytes: u64,
    exclude: &str,
) -> Result<usize, String> {
    // 1. Enumerate current sandbox files with stat metadata
    let current_files = enumerate_sandbox_files(ctx, sandbox_url, workdir, exclude, e2b)?;
    ctx.log(
        "info",
        &format!(
            "tool_runner: fsync enumerated {} files",
            current_files.len()
        ),
    );

    // 2. Read previous manifest from TemperFS
    let old_manifest = read_manifest(ctx, temper_api_url, tenant, manifest_file_id)?;

    let headers = vec![
        ("content-type".to_string(), "application/json".to_string()),
        ("x-tenant-id".to_string(), tenant.to_string()),
        ("x-temper-principal-kind".to_string(), "system".to_string()),
    ];

    let file_url = format!("{temper_api_url}/tdata/Files");
    let mut new_manifest: BTreeMap<String, Value> = BTreeMap::new();
    let mut synced_count: usize = 0;

    // 3. Sync new/modified files
    for (path, entry) in &current_files {
        // Check if unchanged (size AND mtime match)
        if let Some(old_entry) = old_manifest.get(path) {
            if old_entry.size_bytes == entry.size_bytes && old_entry.mtime == entry.mtime {
                // Unchanged — carry forward manifest entry without reading file
                new_manifest.insert(
                    path.clone(),
                    json!({
                        "file_id": old_entry.file_id,
                        "size_bytes": old_entry.size_bytes,
                        "mtime": old_entry.mtime,
                    }),
                );
                continue;
            }
        }

        // File is new or modified — read from sandbox
        let content = if e2b {
            read_file_e2b(ctx, sandbox_url, path)
        } else {
            read_file_local(ctx, sandbox_url, path)
        };

        let content = match content {
            Ok(c) => c,
            Err(e) => {
                ctx.log(
                    "warn",
                    &format!("tool_runner: fsync skip {path}: read failed: {e}"),
                );
                continue;
            }
        };

        if content.len() as u64 > max_file_bytes {
            ctx.log(
                "warn",
                &format!(
                    "tool_runner: fsync skip {path}: {} bytes exceeds max {}",
                    content.len(),
                    max_file_bytes
                ),
            );
            continue;
        }

        // Deterministic File entity ID from workspace + path
        let file_entity_id = format!("wsf-{}", simple_hash(&format!("{workspace_id}:{path}")));

        // Create File entity (ignore 409 = already exists)
        let create_body = json!({
            "FileId": &file_entity_id,
            "workspace_id": workspace_id,
            "name": path.rsplit('/').next().unwrap_or("file"),
            "mime_type": "text/plain",
            "path": path,
        });
        let _ = ctx.http_call("POST", &file_url, &headers, &create_body.to_string());

        // Upload content (CAS dedup handles unchanged content)
        let value_url = format!("{temper_api_url}/tdata/Files('{file_entity_id}')/$value");
        match ctx.http_call("PUT", &value_url, &headers, &content) {
            Ok(resp) if resp.status >= 200 && resp.status < 300 => {
                synced_count += 1;
                new_manifest.insert(
                    path.clone(),
                    json!({
                        "file_id": file_entity_id,
                        "size_bytes": entry.size_bytes,
                        "mtime": entry.mtime,
                    }),
                );
            }
            Ok(resp) => {
                ctx.log(
                    "warn",
                    &format!(
                        "tool_runner: fsync upload failed for {path} (HTTP {})",
                        resp.status
                    ),
                );
            }
            Err(e) => {
                ctx.log(
                    "warn",
                    &format!("tool_runner: fsync upload failed for {path}: {e}"),
                );
            }
        }
    }

    // 4. Handle deletions — archive files that no longer exist in sandbox
    for (path, old_entry) in &old_manifest {
        if !current_files.contains_key(path) {
            let archive_url = format!(
                "{temper_api_url}/tdata/Files('{}')/TemperFS.File.Archive",
                old_entry.file_id
            );
            match ctx.http_call("POST", &archive_url, &headers, "{}") {
                Ok(_) => ctx.log(
                    "info",
                    &format!("tool_runner: fsync archived deleted file {path}"),
                ),
                Err(e) => ctx.log(
                    "warn",
                    &format!("tool_runner: fsync archive failed for {path}: {e}"),
                ),
            }
        }
    }

    // 5. Write updated manifest to TemperFS
    let manifest_body = json!({ "files": new_manifest }).to_string();
    let manifest_url = format!("{temper_api_url}/tdata/Files('{manifest_file_id}')/$value");
    ctx.http_call("PUT", &manifest_url, &headers, &manifest_body)
        .map_err(|e| format!("manifest write failed: {e}"))?;

    Ok(synced_count)
}

fn resolve_temper_api_url(ctx: &Context, fields: &Value) -> String {
    fields
        .get("temper_api_url")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .or_else(|| {
            ctx.config
                .get("temper_api_url")
                .filter(|s| !s.is_empty())
                .cloned()
        })
        .unwrap_or_else(|| "http://127.0.0.1:3000".to_string())
}
