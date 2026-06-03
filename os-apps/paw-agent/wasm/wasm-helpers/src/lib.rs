//! Shared helper functions for Agent WASM modules.
//!
//! Provides common TemperFS I/O, field extraction, URL resolution,
//! and sandbox provider abstraction to eliminate duplication across
//! WASM integration modules.

pub mod sandbox;

use std::collections::BTreeMap;

use temper_wasm_sdk::prelude::*;

pub const SESSION_ENTRIES_REF_PREFIX: &str = "session-entries:";
const TEMPERFS_READ_ATTEMPTS: usize = 10;
const TEMPERFS_WRITE_ATTEMPTS: usize = 5;
const TEMPERFS_BATCH_READ_ATTEMPTS: usize = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchTextFileReadItem {
    pub file_id: String,
    pub found: bool,
    pub content_hash: String,
    pub mime_type: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchTextFileVersionReadItem {
    pub file_version_id: String,
    pub found: bool,
    pub content_hash: String,
    pub mime_type: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreatedContentFileRef {
    pub file_id: String,
    pub file_version_id: String,
    pub content_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreatedSessionEntry {
    pub entity_id: String,
    pub entry_id: String,
}

struct SessionEntryCreateSpec<'a> {
    session_id: &'a str,
    entry_id: &'a str,
    parent_entry_id: Option<&'a str>,
    sequence: i64,
    entry_type: &'a str,
    role: Option<&'a str>,
    content: Option<&'a Value>,
    content_file_id: Option<&'a str>,
    content_file_version_id: Option<&'a str>,
    extra_json: Option<&'a Value>,
    tokens: usize,
}

/// Current wall-clock time as a millis-since-epoch string.
///
/// Used as the value for OData fields shaped `last_*_at` (e.g.
/// `last_heartbeat_at`, `last_progress_at`, `last_message_at`). Historically
/// these fields were populated with sentinel words ("alive", "resumed",
/// "created") because no chrono dep existed in the WASM crates; this helper
/// gives ops a real, sortable, machine-parseable value without pulling in
/// chrono. Consumers that want an ISO timestamp can divide by 1000 and
/// format with any standard library — the string is the canonical i64
/// decimal representation of the host's wall clock.
pub fn timestamp_millis_string() -> String {
    Context::get_time_millis().to_string()
}

pub fn session_entries_ref(session_id: &str) -> String {
    format!("{SESSION_ENTRIES_REF_PREFIX}{session_id}")
}

pub fn session_id_from_entries_ref(reference: &str) -> Option<&str> {
    reference
        .strip_prefix(SESSION_ENTRIES_REF_PREFIX)
        .filter(|session_id| !session_id.is_empty())
}

pub fn is_session_entries_ref(reference: &str) -> bool {
    session_id_from_entries_ref(reference).is_some()
}

pub fn next_session_entry_id(prefix: &str, parent_entry_id: &str) -> (String, i64) {
    let next_sequence = parent_session_entry_sequence(parent_entry_id).unwrap_or(0) + 1;
    (format!("{prefix}-{next_sequence}"), next_sequence)
}

fn parent_session_entry_sequence(parent_entry_id: &str) -> Option<i64> {
    let suffix = parent_entry_id
        .rsplit('-')
        .next()
        .and_then(|value| value.parse::<i64>().ok())?;
    if parent_entry_id.starts_with("u-ss-") {
        suffix.checked_mul(2)?.checked_add(1)
    } else {
        Some(suffix)
    }
}

fn read_temperfs_value_with_retry(
    ctx: &Context,
    url: &str,
    headers: &[(String, String)],
    label: &str,
) -> Result<String, String> {
    let mut last_status = 0;
    let mut last_body = String::new();

    for attempt in 0..TEMPERFS_READ_ATTEMPTS {
        let resp = ctx.http_call("GET", url, headers, "")?;
        if resp.status == 200 {
            return Ok(resp.body);
        }
        if resp.status == 404 {
            return Ok(String::new());
        }

        last_status = resp.status;
        last_body = resp.body;

        if (500..600).contains(&resp.status) && attempt + 1 < TEMPERFS_READ_ATTEMPTS {
            ctx.log(
                "warn",
                &format!(
                    "{label}: transient read failure (HTTP {}), retry {}/{}",
                    resp.status,
                    attempt + 2,
                    TEMPERFS_READ_ATTEMPTS
                ),
            );
            continue;
        }
        break;
    }

    Err(format!(
        "{label} (HTTP {}): {}",
        last_status,
        &last_body[..last_body.len().min(200)]
    ))
}

fn is_retriable_write_failure(status: u16, body: &str) -> bool {
    (500..600).contains(&status) || body.contains("BlobUploadFailed") || body.contains("HTTP -1")
}

pub fn write_temperfs_value_with_retry(
    ctx: &Context,
    url: &str,
    headers: &[(String, String)],
    body: &str,
    label: &str,
) -> Result<(), String> {
    let mut last_status = 0;
    let mut last_body = String::new();

    for attempt in 0..TEMPERFS_WRITE_ATTEMPTS {
        let resp = ctx.http_call("PUT", url, headers, body)?;
        if (200..300).contains(&resp.status) {
            return Ok(());
        }

        last_status = resp.status;
        last_body = resp.body;

        if is_retriable_write_failure(resp.status, &last_body)
            && attempt + 1 < TEMPERFS_WRITE_ATTEMPTS
        {
            ctx.log(
                "warn",
                &format!(
                    "{label}: transient write failure (HTTP {}), retry {}/{}",
                    resp.status,
                    attempt + 2,
                    TEMPERFS_WRITE_ATTEMPTS
                ),
            );
            continue;
        }
        break;
    }

    Err(format!(
        "{label} (HTTP {}): {}",
        last_status,
        &last_body[..last_body.len().min(200)]
    ))
}

/// Resolve the Temper API URL from entity fields or context config,
/// falling back to localhost.
pub fn resolve_temper_api_url(ctx: &Context, fields: &Value) -> String {
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

/// Read session JSONL from TemperFS by file ID.
pub fn read_session_from_temperfs(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    fields: &Value,
    file_id: &str,
) -> Result<String, String> {
    if let Some(session_id) = session_id_from_entries_ref(file_id) {
        return read_session_from_entries(ctx, temper_api_url, tenant, fields, session_id);
    }

    let url = format!("{temper_api_url}/tdata/Files('{file_id}')/$value");
    let headers = runtime_headers(ctx, tenant, fields, None, None);
    read_temperfs_value_with_retry(ctx, &url, &headers, "TemperFS session read failed")
}

/// Write session JSONL to TemperFS by file ID.
pub fn write_session_to_temperfs(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    fields: &Value,
    file_id: &str,
    jsonl: &str,
) -> Result<(), String> {
    if let Some(session_id) = session_id_from_entries_ref(file_id) {
        return sync_session_entries_from_jsonl(
            ctx,
            temper_api_url,
            tenant,
            fields,
            session_id,
            jsonl,
        );
    }

    let url = format!("{temper_api_url}/tdata/Files('{file_id}')/$value");
    let headers = runtime_headers(ctx, tenant, fields, Some("text/plain"), None);
    write_temperfs_value_with_retry(ctx, &url, &headers, jsonl, "TemperFS session write failed")
}

pub fn create_session_entry(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    fields: &Value,
    session_id: &str,
    entry_id: &str,
    parent_entry_id: Option<&str>,
    sequence: i64,
    entry_type: &str,
    role: Option<&str>,
    content: Option<&Value>,
    content_file_id: Option<&str>,
    content_file_version_id: Option<&str>,
    extra_json: Option<&Value>,
    tokens: usize,
) -> Result<CreatedSessionEntry, String> {
    let spec = SessionEntryCreateSpec {
        session_id,
        entry_id,
        parent_entry_id,
        sequence,
        entry_type,
        role,
        content,
        content_file_id,
        content_file_version_id,
        extra_json,
        tokens,
    };
    let body = session_entry_create_body(&spec)?;
    let url = format!("{temper_api_url}/tdata/SessionEntries");
    let headers = runtime_headers(
        ctx,
        tenant,
        fields,
        Some("application/json"),
        Some("application/json"),
    );
    let resp = ctx.http_call("POST", &url, &headers, &body.to_string())?;
    if resp.status < 200 || resp.status >= 300 {
        return Err(format!(
            "SessionEntry creation failed (HTTP {}): {}",
            resp.status,
            &resp.body[..resp.body.len().min(300)]
        ));
    }
    let created = parse_created_session_entry_ack(&resp.body, session_id, entry_id)?;
    maybe_verify_session_entries(
        ctx,
        temper_api_url,
        tenant,
        fields,
        session_id,
        &[entry_id],
        "create_session_entry",
    )?;

    Ok(created)
}

pub fn create_initial_session_entries(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    fields: &Value,
    session_id: &str,
    user_message: &str,
) -> Result<(CreatedSessionEntry, CreatedSessionEntry), String> {
    let header_id = format!("h-{session_id}");
    let user_entry_id = format!("u-{session_id}-0");
    let header_extra = json!({ "version": 1 });
    let user_content = json!(user_message);
    let specs = [
        SessionEntryCreateSpec {
            session_id,
            entry_id: &header_id,
            parent_entry_id: None,
            sequence: 0,
            entry_type: "header",
            role: None,
            content: None,
            content_file_id: None,
            content_file_version_id: None,
            extra_json: Some(&header_extra),
            tokens: 0,
        },
        SessionEntryCreateSpec {
            session_id,
            entry_id: &user_entry_id,
            parent_entry_id: Some(&header_id),
            sequence: 1,
            entry_type: "message",
            role: Some("user"),
            content: Some(&user_content),
            content_file_id: None,
            content_file_version_id: None,
            extra_json: None,
            tokens: user_message.len() / 4,
        },
    ];
    let created =
        create_session_entry_batch(ctx, temper_api_url, tenant, fields, &specs, "initial")?;

    maybe_verify_session_entries(
        ctx,
        temper_api_url,
        tenant,
        fields,
        session_id,
        &[&header_id, &user_entry_id],
        "create_initial_session_entries",
    )?;

    if created.len() != 2 {
        return Err(format!(
            "initial SessionEntry batch returned {} created entries for 2 specs",
            created.len()
        ));
    }
    Ok((created[0].clone(), created[1].clone()))
}

pub fn materialize_initial_session_entries_with_assistant(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    fields: &Value,
    session_id: &str,
    user_message: &str,
    assistant_content: &Value,
    assistant_tokens: usize,
) -> Result<CreatedSessionEntry, String> {
    let user_entry_id = format!("u-{session_id}-0");
    let (assistant_entry_id, assistant_sequence) = next_session_entry_id("a", &user_entry_id);
    let user_content = json!(user_message);
    let specs = [
        SessionEntryCreateSpec {
            session_id,
            entry_id: &user_entry_id,
            parent_entry_id: None,
            sequence: 1,
            entry_type: "message",
            role: Some("user"),
            content: Some(&user_content),
            content_file_id: None,
            content_file_version_id: None,
            extra_json: None,
            tokens: user_message.len() / 4,
        },
        SessionEntryCreateSpec {
            session_id,
            entry_id: &assistant_entry_id,
            parent_entry_id: Some(&user_entry_id),
            sequence: assistant_sequence,
            entry_type: "message",
            role: Some("assistant"),
            content: Some(assistant_content),
            content_file_id: None,
            content_file_version_id: None,
            extra_json: None,
            tokens: assistant_tokens,
        },
    ];
    let created = create_session_entry_batch(
        ctx,
        temper_api_url,
        tenant,
        fields,
        &specs,
        "materialize initial SessionEntry",
    )?;
    maybe_verify_session_entries(
        ctx,
        temper_api_url,
        tenant,
        fields,
        session_id,
        &[&user_entry_id, &assistant_entry_id],
        "materialize_initial_session_entries_with_assistant",
    )?;

    created
        .into_iter()
        .find(|entry| entry.entry_id == assistant_entry_id)
        .ok_or_else(|| {
            format!(
                "materialize initial SessionEntry batch did not return assistant entry {assistant_entry_id}"
            )
        })
}

fn create_session_entry_batch(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    fields: &Value,
    specs: &[SessionEntryCreateSpec<'_>],
    label: &str,
) -> Result<Vec<CreatedSessionEntry>, String> {
    let create_headers = runtime_headers(
        ctx,
        tenant,
        fields,
        Some("application/json"),
        Some("application/json"),
    );
    let create_url = format!("{temper_api_url}/tdata/SessionEntries");
    let create_requests = specs
        .iter()
        .map(|spec| {
            let body = session_entry_create_body(spec)?;
            Ok(HttpRequest {
                method: "POST".to_string(),
                url: create_url.clone(),
                headers: create_headers.clone(),
                body: body.to_string(),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let create_responses = ctx.http_call_batch(&create_requests)?;
    if create_responses.len() != specs.len() {
        return Err(format!(
            "{label} batch returned {} responses for {} requests",
            create_responses.len(),
            specs.len()
        ));
    }

    let mut created = Vec::with_capacity(specs.len());
    for (spec, resp) in specs.iter().zip(create_responses.iter()) {
        if resp.status < 200 || resp.status >= 300 {
            return Err(format!(
                "{label} {} creation failed (HTTP {}): {}",
                spec.entry_id,
                resp.status,
                &resp.body[..resp.body.len().min(300)]
            ));
        }
        created.push(parse_created_session_entry_ack(
            &resp.body,
            spec.session_id,
            spec.entry_id,
        )?);
    }
    Ok(created)
}

fn maybe_verify_session_entries(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    fields: &Value,
    session_id: &str,
    entry_ids: &[&str],
    label: &str,
) -> Result<(), String> {
    let config_value = ctx
        .config
        .get("session_entry_create_verify_readback")
        .map(String::as_str);
    if session_entry_create_verify_readback_enabled(fields, config_value) {
        return verify_session_entries(
            ctx,
            temper_api_url,
            tenant,
            fields,
            session_id,
            entry_ids,
            label,
        );
    }

    ctx.log(
        "info",
        &format!(
            "{label}: SessionEntry create response ack verified; strict read-back skipped (SessionId={session_id}, entries={})",
            entry_ids.join(",")
        ),
    );
    Ok(())
}

fn verify_session_entries(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    fields: &Value,
    session_id: &str,
    entry_ids: &[&str],
    label: &str,
) -> Result<(), String> {
    const VERIFY_ATTEMPTS: u32 = 4;
    let verify_headers = runtime_headers(ctx, tenant, fields, None, Some("application/json"));
    let single_entry_id = entry_ids
        .iter()
        .copied()
        .next()
        .filter(|_| entry_ids.len() == 1);
    let verify_url = single_entry_id
        .map(|entry_id| session_entry_verify_url(temper_api_url, session_id, entry_id))
        .unwrap_or_else(|| session_entries_verify_url(temper_api_url, session_id));
    let mut last_status = 0_i64;
    let mut last_err = String::new();
    for attempt in 0..VERIFY_ATTEMPTS {
        match ctx.http_call("GET", &verify_url, &verify_headers, "") {
            Ok(resp) if resp.status == 200 => {
                last_status = resp.status as i64;
                let visible = if single_entry_id.is_some() {
                    session_entry_verify_response_visible(&resp.body)
                } else {
                    match session_entry_verify_missing_ids(&resp.body, entry_ids) {
                        Ok(missing) if missing.is_empty() => true,
                        Ok(missing) => {
                            last_err = format!("missing entries: {}", missing.join(","));
                            false
                        }
                        Err(err) => {
                            last_err = err;
                            false
                        }
                    }
                };
                if visible {
                    if attempt > 0 {
                        ctx.log(
                            "info",
                            &format!(
                                "{label}: read-back visible on attempt {} (SessionId={session_id})",
                                attempt + 1
                            ),
                        );
                    }
                    return Ok(());
                }
            }
            Ok(resp) => {
                last_status = resp.status as i64;
            }
            Err(err) => {
                last_status = -1;
                last_err = err.to_string();
            }
        }
        if attempt + 1 < VERIFY_ATTEMPTS {
            let target_delay_ms = 50_i64 << attempt;
            let until = Context::get_time_millis() + target_delay_ms;
            while Context::get_time_millis() < until {}
        }
    }

    ctx.log(
        "error",
        &format!(
            "{label}: WRITE LOST — batch POST 2xx for SessionId={session_id} but read-back missed after {VERIFY_ATTEMPTS} attempts (last_status={last_status} last_err={last_err:?})"
        ),
    );
    Err(format!(
        "{label} entries acknowledged but read-back missed after {VERIFY_ATTEMPTS} attempts: SessionId={session_id}"
    ))
}

fn session_entry_create_body(spec: &SessionEntryCreateSpec<'_>) -> Result<Value, String> {
    let content_json = spec
        .content
        .map(serde_json::to_string)
        .transpose()
        .map_err(|err| format!("serialize SessionEntry content: {err}"))?
        .unwrap_or_default();
    let extra_json = spec
        .extra_json
        .map(serde_json::to_string)
        .transpose()
        .map_err(|err| format!("serialize SessionEntry extra_json: {err}"))?
        .unwrap_or_else(|| "{}".to_string());

    Ok(json!({
        "SessionId": spec.session_id,
        "EntryId": spec.entry_id,
        "ParentEntryId": spec.parent_entry_id.unwrap_or(""),
        "Sequence": spec.sequence,
        "EntryType": spec.entry_type,
        "Role": spec.role.unwrap_or(""),
        "Content": content_json,
        "ContentFileId": spec.content_file_id.unwrap_or(""),
        "ContentFileVersionId": spec.content_file_version_id.unwrap_or(""),
        "ExtraJson": extra_json,
        "Tokens": spec.tokens as i64,
    }))
}

fn parse_created_session_entry_ack(
    body: &str,
    expected_session_id: &str,
    expected_entry_id: &str,
) -> Result<CreatedSessionEntry, String> {
    let parsed: Value = serde_json::from_str(body)
        .map_err(|err| format!("parse SessionEntry creation response: {err}"))?;
    let entity_id = entity_field_str(&parsed, &["entity_id", "Id"])
        .unwrap_or("")
        .to_string();
    if entity_id.is_empty() {
        return Err("SessionEntry creation response missing entity_id/Id".to_string());
    }

    let actual_session_id = entity_field_str(&parsed, &["SessionId", "session_id"]).unwrap_or("");
    let actual_entry_id = entity_field_str(&parsed, &["EntryId", "entry_id"]).unwrap_or("");
    if actual_session_id != expected_session_id || actual_entry_id != expected_entry_id {
        return Err(format!(
            "SessionEntry creation response ack mismatch: expected SessionId={expected_session_id} EntryId={expected_entry_id}, got SessionId={actual_session_id:?} EntryId={actual_entry_id:?}"
        ));
    }

    Ok(CreatedSessionEntry {
        entity_id,
        entry_id: expected_entry_id.to_string(),
    })
}

fn session_entry_create_verify_readback_enabled(
    fields: &Value,
    config_value: Option<&str>,
) -> bool {
    fields
        .get("session_entry_create_verify_readback")
        .and_then(boolish_json)
        .or_else(|| config_value.and_then(boolish_str))
        .unwrap_or(false)
}

fn boolish_json(value: &Value) -> Option<bool> {
    value
        .as_bool()
        .or_else(|| value.as_str().and_then(boolish_str))
}

fn boolish_str(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

fn session_entry_verify_url(temper_api_url: &str, session_id: &str, entry_id: &str) -> String {
    format!(
        "{temper_api_url}/tdata/SessionEntries?$filter=SessionId%20eq%20%27{}%27%20and%20EntryId%20eq%20%27{}%27&$top=1",
        session_id.replace('\'', "''"),
        entry_id.replace('\'', "''"),
    )
}

fn session_entries_verify_url(temper_api_url: &str, session_id: &str) -> String {
    format!(
        "{temper_api_url}/tdata/SessionEntries?$filter=SessionId%20eq%20%27{}%27&$top=10000",
        session_id.replace('\'', "''"),
    )
}

fn session_entry_verify_response_visible(body: &str) -> bool {
    let parsed: Value = serde_json::from_str(body).unwrap_or_else(|_| json!({"value": []}));
    parsed
        .get("value")
        .and_then(|value| value.as_array())
        .map(|items| !items.is_empty())
        .unwrap_or(false)
}

fn session_entry_verify_missing_ids(body: &str, entry_ids: &[&str]) -> Result<Vec<String>, String> {
    let parsed: Value = serde_json::from_str(body)
        .map_err(|err| format!("parse SessionEntry verify response: {err}"))?;
    let items = parsed
        .get("value")
        .and_then(Value::as_array)
        .ok_or("SessionEntry verify response missing value array")?;
    let visible_ids = items
        .iter()
        .filter_map(|entry| entity_field_str(entry, &["EntryId", "entry_id"]).map(str::to_string))
        .collect::<std::collections::BTreeSet<_>>();
    Ok(entry_ids
        .iter()
        .filter(|entry_id| !visible_ids.contains(**entry_id))
        .map(|entry_id| (*entry_id).to_string())
        .collect())
}

pub fn append_session_entry_inline(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    fields: &Value,
    session_ref: &str,
    parent_entry_id: &str,
    entry_prefix: &str,
    role: &str,
    content: &Value,
    tokens: usize,
) -> Result<CreatedSessionEntry, String> {
    let session_id = session_id_from_entries_ref(session_ref)
        .ok_or("append_session_entry_inline requires session-entries:<session_id> ref")?;
    if parent_entry_id.is_empty() {
        return Err("append_session_entry_inline requires parent_entry_id".to_string());
    }
    let (entry_id, sequence) = next_session_entry_id(entry_prefix, parent_entry_id);
    create_session_entry(
        ctx,
        temper_api_url,
        tenant,
        fields,
        session_id,
        &entry_id,
        Some(parent_entry_id),
        sequence,
        "message",
        Some(role),
        Some(content),
        None,
        None,
        None,
        tokens,
    )
}

pub fn read_session_from_entries(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    fields: &Value,
    session_id: &str,
) -> Result<String, String> {
    let entries = list_session_entries(ctx, temper_api_url, tenant, fields, session_id)?;
    let user_message = entity_field_str(fields, &["UserMessage", "user_message"]).unwrap_or("");
    Ok(session_entries_jsonl_from_entities_with_synthetic_root(
        &entries,
        session_id,
        user_message,
    ))
}

pub fn sync_session_entries_from_jsonl(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    fields: &Value,
    session_id: &str,
    jsonl: &str,
) -> Result<(), String> {
    let existing = list_session_entries(ctx, temper_api_url, tenant, fields, session_id)?;
    let existing_ids: std::collections::BTreeSet<String> = existing
        .iter()
        .filter_map(|entry| entity_field_str(entry, &["EntryId", "entry_id"]).map(str::to_string))
        .collect();

    for (sequence, line) in jsonl.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let parsed: Value = serde_json::from_str(line)
            .map_err(|err| format!("parse SessionTree JSONL line for SessionEntry sync: {err}"))?;
        let entry_id = parsed
            .get("id")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .ok_or("SessionTree JSONL line missing id")?;
        if existing_ids.contains(entry_id) {
            continue;
        }

        create_session_entry_from_jsonl_value(
            ctx,
            temper_api_url,
            tenant,
            fields,
            session_id,
            &parsed,
            sequence as i64,
        )?;
    }

    Ok(())
}

pub fn session_entries_jsonl_from_entities(entries: &[Value]) -> String {
    session_entries_jsonl_from_entities_with_synthetic_root(entries, "", "")
}

fn session_entries_jsonl_from_entities_with_synthetic_root(
    entries: &[Value],
    session_id: &str,
    user_message: &str,
) -> String {
    let mut rows: Vec<(i64, String, String)> = entries
        .iter()
        .filter_map(|entry| {
            let sequence = entity_field_i64(entry, &["Sequence", "sequence"]).unwrap_or(0);
            let entry_id = entity_field_str(entry, &["EntryId", "entry_id"])
                .unwrap_or("")
                .to_string();
            if entry_id.is_empty() {
                return None;
            }
            session_entry_entity_to_jsonl(entry).map(|line| (sequence, entry_id, line))
        })
        .collect();
    synthesize_missing_initial_session_root(&mut rows, session_id, user_message);
    rows.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
    rows.into_iter()
        .map(|(_, _, line)| line)
        .collect::<Vec<_>>()
        .join("\n")
}

fn synthesize_missing_initial_session_root(
    rows: &mut Vec<(i64, String, String)>,
    session_id: &str,
    user_message: &str,
) {
    if rows.is_empty() || session_id.is_empty() || user_message.is_empty() {
        return;
    }

    let header_id = format!("h-{session_id}");
    let user_id = format!("u-{session_id}-0");
    let mut has_header = false;
    let mut has_user = false;
    let mut references_user = false;

    for (_, entry_id, line) in rows.iter() {
        if entry_id == &header_id {
            has_header = true;
        }
        if entry_id == &user_id {
            has_user = true;
        }
        if jsonl_parent_id(line).as_deref() == Some(user_id.as_str()) {
            references_user = true;
        }
    }

    if has_header && has_user {
        return;
    }
    if has_user || !references_user {
        return;
    }

    if !has_header {
        let header_line = json!({
            "id": header_id,
            "parentId": Value::Null,
            "type": "header",
            "version": 1,
            "tokens": 0
        })
        .to_string();
        rows.push((-2, header_id.clone(), header_line));
    }

    let user_line = json!({
        "id": user_id,
        "parentId": header_id,
        "type": "message",
        "role": "user",
        "content": user_message,
        "tokens": user_message.len() / 4
    })
    .to_string();
    rows.push((-1, user_id, user_line));
}

fn jsonl_parent_id(line: &str) -> Option<String> {
    serde_json::from_str::<Value>(line)
        .ok()?
        .get("parentId")?
        .as_str()
        .map(str::to_string)
}

fn create_session_entry_from_jsonl_value(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    fields: &Value,
    session_id: &str,
    entry: &Value,
    sequence: i64,
) -> Result<CreatedSessionEntry, String> {
    let entry_id = entry
        .get("id")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or("SessionTree JSONL entry missing id")?;
    let parent_entry_id = entry.get("parentId").and_then(Value::as_str);
    let entry_type = entry
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("message");
    let role = entry.get("role").and_then(Value::as_str);
    let tokens = entry.get("tokens").and_then(Value::as_u64).unwrap_or(0) as usize;
    let content = entry.get("content");
    let content_file_id = entry.get("content_file_id").and_then(Value::as_str);
    let content_file_version_id = entry.get("content_file_version_id").and_then(Value::as_str);
    let extra_json = session_entry_extra_json(entry);

    create_session_entry(
        ctx,
        temper_api_url,
        tenant,
        fields,
        session_id,
        entry_id,
        parent_entry_id,
        sequence,
        entry_type,
        role,
        content,
        content_file_id,
        content_file_version_id,
        Some(&extra_json),
        tokens,
    )
}

fn list_session_entries(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    fields: &Value,
    session_id: &str,
) -> Result<Vec<Value>, String> {
    let escaped = session_id.replace('\'', "''");
    let url = format!(
        "{temper_api_url}/tdata/SessionEntries?$filter=SessionId eq '{escaped}'&$top=10000"
    );
    let headers = runtime_headers(ctx, tenant, fields, None, Some("application/json"));
    let resp = ctx.http_call("GET", &url, &headers, "")?;
    if resp.status != 200 {
        return Err(format!(
            "SessionEntry list failed (HTTP {}): {}",
            resp.status,
            &resp.body[..resp.body.len().min(300)]
        ));
    }
    let parsed: Value = serde_json::from_str(&resp.body)
        .map_err(|err| format!("parse SessionEntry list response: {err}"))?;
    Ok(parsed
        .get("value")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default())
}

fn session_entry_entity_to_jsonl(entry: &Value) -> Option<String> {
    let entry_id = entity_field_str(entry, &["EntryId", "entry_id"])?;
    let parent_entry_id =
        entity_field_str(entry, &["ParentEntryId", "parent_entry_id"]).unwrap_or("");
    let entry_type = entity_field_str(entry, &["EntryType", "entry_type"]).unwrap_or("message");
    let role = entity_field_str(entry, &["Role", "role"]).unwrap_or("");
    let tokens = entity_field_i64(entry, &["Tokens", "tokens"]).unwrap_or(0);
    let content = entity_field_str(entry, &["Content", "content"]).unwrap_or("");
    let content_file_id =
        entity_field_str(entry, &["ContentFileId", "content_file_id"]).unwrap_or("");
    let content_file_version_id =
        entity_field_str(entry, &["ContentFileVersionId", "content_file_version_id"]).unwrap_or("");
    let extra_json = entity_field_str(entry, &["ExtraJson", "extra_json"]).unwrap_or("{}");

    let mut line = json!({
        "id": entry_id,
        "parentId": if parent_entry_id.is_empty() { Value::Null } else { json!(parent_entry_id) },
        "type": entry_type,
        "tokens": tokens,
    });

    if let Ok(extra) = serde_json::from_str::<Value>(extra_json) {
        if let (Some(target), Some(extra_obj)) = (line.as_object_mut(), extra.as_object()) {
            for (key, value) in extra_obj {
                if !is_session_entry_canonical_key(key) {
                    target.insert(key.clone(), value.clone());
                }
            }
        }
    }

    if !role.is_empty() {
        line["role"] = json!(role);
    }
    if !content.is_empty() {
        line["content"] = serde_json::from_str::<Value>(content)
            .unwrap_or_else(|_| Value::String(content.to_string()));
    }
    if !content_file_id.is_empty() {
        line["content_file_id"] = json!(content_file_id);
    }
    if !content_file_version_id.is_empty() {
        line["content_file_version_id"] = json!(content_file_version_id);
    }

    serde_json::to_string(&line).ok()
}

fn session_entry_extra_json(entry: &Value) -> Value {
    let mut extra = serde_json::Map::new();
    let Some(obj) = entry.as_object() else {
        return json!({});
    };
    for (key, value) in obj {
        if !is_session_entry_canonical_key(key) {
            extra.insert(key.clone(), value.clone());
        }
    }
    Value::Object(extra)
}

fn is_session_entry_canonical_key(key: &str) -> bool {
    matches!(
        key,
        "id" | "parentId"
            | "type"
            | "role"
            | "content"
            | "tokens"
            | "content_file_id"
            | "content_file_version_id"
    )
}

/// Read raw file content from TemperFS by file ID.
pub fn read_content_file(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    fields: &Value,
    file_id: &str,
) -> Result<String, String> {
    let url = format!("{temper_api_url}/tdata/Files('{file_id}')/$value");
    let headers = runtime_headers(ctx, tenant, fields, None, None);
    read_temperfs_value_with_retry(ctx, &url, &headers, "TemperFS content file read failed")
}

pub fn read_content_file_version(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    fields: &Value,
    file_version_id: &str,
) -> Result<String, String> {
    let results = read_text_file_versions_batch(
        ctx,
        temper_api_url,
        tenant,
        fields,
        &[file_version_id.to_string()],
    )?;
    Ok(results
        .get(file_version_id)
        .filter(|item| item.found)
        .map(|item| item.text.clone())
        .unwrap_or_default())
}

pub fn read_text_files_batch(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    fields: &Value,
    file_ids: &[String],
) -> Result<BTreeMap<String, BatchTextFileReadItem>, String> {
    if file_ids.is_empty() {
        return Ok(BTreeMap::new());
    }

    let url = format!("{temper_api_url}/api/files/read-text-batch");
    let headers = runtime_headers(
        ctx,
        tenant,
        fields,
        Some("application/json"),
        Some("application/json"),
    );
    let body = json!({ "file_ids": file_ids }).to_string();

    let mut last_status = 0;
    let mut last_body = String::new();

    for attempt in 0..TEMPERFS_BATCH_READ_ATTEMPTS {
        let resp = ctx.http_call("POST", &url, &headers, &body)?;
        if resp.status == 200 {
            return parse_batch_text_file_read_response(&resp.body);
        }

        last_status = resp.status;
        last_body = resp.body;

        if (500..600).contains(&last_status) && attempt + 1 < TEMPERFS_BATCH_READ_ATTEMPTS {
            ctx.log(
                "warn",
                &format!(
                    "TemperFS batch read transient failure (HTTP {}), retry {}/{}",
                    last_status,
                    attempt + 2,
                    TEMPERFS_BATCH_READ_ATTEMPTS
                ),
            );
            continue;
        }
        break;
    }

    Err(format!(
        "TemperFS batch read failed (HTTP {}): {}",
        last_status,
        &last_body[..last_body.len().min(200)]
    ))
}

pub fn read_text_file_versions_batch(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    fields: &Value,
    file_version_ids: &[String],
) -> Result<BTreeMap<String, BatchTextFileVersionReadItem>, String> {
    if file_version_ids.is_empty() {
        return Ok(BTreeMap::new());
    }

    let url = format!("{temper_api_url}/api/files/read-version-text-batch");
    let headers = runtime_headers(
        ctx,
        tenant,
        fields,
        Some("application/json"),
        Some("application/json"),
    );
    let body = json!({ "file_version_ids": file_version_ids }).to_string();

    let mut last_status = 0;
    let mut last_body = String::new();

    for attempt in 0..TEMPERFS_BATCH_READ_ATTEMPTS {
        let resp = ctx.http_call("POST", &url, &headers, &body)?;
        if resp.status == 200 {
            return parse_batch_text_file_version_read_response(&resp.body);
        }

        last_status = resp.status;
        last_body = resp.body;

        if (500..600).contains(&last_status) && attempt + 1 < TEMPERFS_BATCH_READ_ATTEMPTS {
            ctx.log(
                "warn",
                &format!(
                    "TemperFS batch version read transient failure (HTTP {}), retry {}/{}",
                    last_status,
                    attempt + 2,
                    TEMPERFS_BATCH_READ_ATTEMPTS
                ),
            );
            continue;
        }
        break;
    }

    Err(format!(
        "TemperFS batch version read failed (HTTP {}): {}",
        last_status,
        &last_body[..last_body.len().min(200)]
    ))
}

pub fn parse_batch_text_file_read_response(
    body: &str,
) -> Result<BTreeMap<String, BatchTextFileReadItem>, String> {
    let parsed: Value =
        serde_json::from_str(body).map_err(|e| format!("parse batch file read response: {e}"))?;
    let files = parsed
        .get("files")
        .and_then(Value::as_array)
        .ok_or("batch file read response missing files array")?;

    let mut by_id = BTreeMap::new();
    for file in files {
        let file_id = file
            .get("file_id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        if file_id.is_empty() {
            continue;
        }

        by_id.insert(
            file_id.clone(),
            BatchTextFileReadItem {
                file_id,
                found: file.get("found").and_then(Value::as_bool).unwrap_or(false),
                content_hash: file
                    .get("content_hash")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                mime_type: file
                    .get("mime_type")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                text: file
                    .get("text")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
            },
        );
    }

    Ok(by_id)
}

pub fn parse_batch_text_file_version_read_response(
    body: &str,
) -> Result<BTreeMap<String, BatchTextFileVersionReadItem>, String> {
    let parsed: Value = serde_json::from_str(body)
        .map_err(|e| format!("parse batch file version read response: {e}"))?;
    let files = parsed
        .get("files")
        .and_then(Value::as_array)
        .ok_or("batch file version read response missing files array")?;

    let mut by_id = BTreeMap::new();
    for file in files {
        let file_version_id = file
            .get("file_version_id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        if file_version_id.is_empty() {
            continue;
        }

        by_id.insert(
            file_version_id.clone(),
            BatchTextFileVersionReadItem {
                file_version_id,
                found: file.get("found").and_then(Value::as_bool).unwrap_or(false),
                content_hash: file
                    .get("content_hash")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                mime_type: file
                    .get("mime_type")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
                text: file
                    .get("text")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string(),
            },
        );
    }

    Ok(by_id)
}

/// Create a TemperFS file and write content into it.
pub fn create_content_file(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    workspace_id: &str,
    file_name: &str,
    content: &str,
) -> Result<String, String> {
    create_content_file_ref(
        ctx,
        temper_api_url,
        tenant,
        workspace_id,
        file_name,
        content,
    )
    .map(|created| created.file_id)
}

/// Create a TemperFS file, write content into it, and resolve the immutable
/// file version produced by that write.
pub fn create_content_file_ref(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    workspace_id: &str,
    file_name: &str,
    content: &str,
) -> Result<CreatedContentFileRef, String> {
    let headers = runtime_headers_with_workspace(
        ctx,
        tenant,
        &serde_json::json!({}),
        Some(workspace_id),
        None,
        Some("application/json"),
        None,
    );

    let file_body = serde_json::json!({
        "workspace_id": workspace_id,
        "name": file_name,
        "mime_type": "text/plain",
        "path": format!("/{file_name}")
    });
    let file_url = format!("{temper_api_url}/tdata/Files");
    let file_resp = ctx.http_call("POST", &file_url, &headers, &file_body.to_string())?;

    if file_resp.status < 200 || file_resp.status >= 300 {
        return Err(format!(
            "content file creation failed (HTTP {}): {}",
            file_resp.status,
            &file_resp.body[..file_resp.body.len().min(300)]
        ));
    }

    let file_parsed: Value = serde_json::from_str(&file_resp.body)
        .map_err(|e| format!("parse content file response: {e}"))?;
    let file_id = file_parsed
        .get("entity_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    if file_id.is_empty() {
        return Err("content file created but entity_id missing".to_string());
    }

    let value_url = format!("{temper_api_url}/tdata/Files('{file_id}')/$value");
    let value_headers = runtime_headers_with_workspace(
        ctx,
        tenant,
        &serde_json::json!({}),
        Some(workspace_id),
        None,
        Some("text/plain"),
        None,
    );
    write_temperfs_value_with_retry(
        ctx,
        &value_url,
        &value_headers,
        content,
        "content file write failed",
    )?;

    let file_state = read_content_file_head(ctx, temper_api_url, tenant, workspace_id, &file_id)?;
    let file_version_id = entity_field_str(&file_state, &["LastVersionId", "last_version_id"])
        .unwrap_or("")
        .to_string();
    let content_hash = entity_field_str(&file_state, &["ContentHash", "content_hash"])
        .unwrap_or("")
        .to_string();

    Ok(CreatedContentFileRef {
        file_id,
        file_version_id,
        content_hash,
    })
}

/// Build standard OData headers for tenant-scoped requests.
pub fn odata_headers(ctx: &Context, tenant: &str, fields: &Value) -> Vec<(String, String)> {
    runtime_headers(
        ctx,
        tenant,
        fields,
        Some("application/json"),
        Some("application/json"),
    )
}

/// Build tenant-scoped runtime headers with an entity-derived principal.
pub fn runtime_headers(
    ctx: &Context,
    tenant: &str,
    fields: &Value,
    content_type: Option<&str>,
    accept: Option<&str>,
) -> Vec<(String, String)> {
    runtime_headers_with_workspace(ctx, tenant, fields, None, None, content_type, accept)
}

/// Build runtime headers while overriding the logical agent type.
pub fn runtime_headers_as(
    ctx: &Context,
    tenant: &str,
    fields: &Value,
    agent_type: &str,
    content_type: Option<&str>,
    accept: Option<&str>,
) -> Vec<(String, String)> {
    runtime_headers_with_workspace(
        ctx,
        tenant,
        fields,
        None,
        Some(agent_type),
        content_type,
        accept,
    )
}

/// Build runtime headers while explicitly supplying the workspace context.
pub fn runtime_headers_for_workspace(
    ctx: &Context,
    tenant: &str,
    fields: &Value,
    workspace_id: &str,
    content_type: Option<&str>,
    accept: Option<&str>,
) -> Vec<(String, String)> {
    runtime_headers_with_workspace(
        ctx,
        tenant,
        fields,
        Some(workspace_id),
        None,
        content_type,
        accept,
    )
}

fn runtime_headers_with_workspace(
    ctx: &Context,
    tenant: &str,
    fields: &Value,
    workspace_id_override: Option<&str>,
    agent_type_override: Option<&str>,
    content_type: Option<&str>,
    accept: Option<&str>,
) -> Vec<(String, String)> {
    let mut headers = Vec::new();
    headers.push(("x-tenant-id".to_string(), tenant.to_string()));
    headers.push(("x-temper-principal-kind".to_string(), "agent".to_string()));
    headers.push(("x-temper-principal-id".to_string(), ctx.entity_id.clone()));
    headers.push((
        "x-temper-agent-type".to_string(),
        agent_type_override
            .unwrap_or_else(|| default_agent_type(ctx))
            .to_string(),
    ));

    if let Some(content_type) = content_type {
        headers.push(("content-type".to_string(), content_type.to_string()));
    }
    if let Some(accept) = accept {
        headers.push(("accept".to_string(), accept.to_string()));
    }

    if let Some(soul_id) =
        entity_field_str(fields, &["soul_id", "SoulId"]).filter(|v| !v.is_empty())
    {
        headers.push(("x-temper-attr-soul_id".to_string(), soul_id.to_string()));
    }

    let workspace_id = workspace_id_override
        .filter(|value| !value.is_empty())
        .or_else(|| {
            entity_field_str(fields, &["workspace_id", "WorkspaceId"])
                .filter(|value| !value.is_empty())
        });
    if let Some(workspace_id) = workspace_id {
        headers.push((
            "x-temper-attr-workspaceid".to_string(),
            workspace_id.to_string(),
        ));
    }

    if let Some(key) = ctx.config.get("temper_api_key").filter(|k| !k.is_empty()) {
        headers.push(("authorization".to_string(), format!("Bearer {key}")));
    }

    headers
}

fn read_content_file_head(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    workspace_id: &str,
    file_id: &str,
) -> Result<Value, String> {
    let url = format!("{temper_api_url}/tdata/Files('{file_id}')");
    let headers = runtime_headers_with_workspace(
        ctx,
        tenant,
        &serde_json::json!({}),
        Some(workspace_id),
        None,
        Some("application/json"),
        Some("application/json"),
    );
    let resp = ctx.http_call("GET", &url, &headers, "")?;
    if resp.status != 200 {
        return Err(format!(
            "content file head read failed (HTTP {}): {}",
            resp.status,
            &resp.body[..resp.body.len().min(200)]
        ));
    }
    serde_json::from_str(&resp.body).map_err(|e| format!("parse content file head response: {e}"))
}

/// Derive agent type from entity type for WASM modules that construct their own headers.
/// Session integrations act on behalf of an agent; all others are platform-internal ("system")
/// dispatch callbacks. For modules covered by host-injected auth (ADR-0043), this is unused —
/// the host derives agent_type from WasmInvocationContext.entity_type directly.
fn default_agent_type(ctx: &Context) -> &'static str {
    if ctx.entity_type.eq_ignore_ascii_case("Session")
        || ctx.entity_type.eq_ignore_ascii_case("Sessions")
    {
        "agent"
    } else {
        "system"
    }
}

/// Look up a string field directly on a JSON value, trying multiple key names.
pub fn direct_field_str<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
}

/// Look up a string field on a JSON value, falling back to nested `fields` object.
pub fn entity_field_str<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a str> {
    direct_field_str(value, keys).or_else(|| {
        value
            .get("fields")
            .and_then(|fields| direct_field_str(fields, keys))
    })
}

/// Look up an integer-ish field on a JSON value, accepting JSON numbers or strings.
pub fn entity_field_i64(value: &Value, keys: &[&str]) -> Option<i64> {
    keys.iter().find_map(|key| {
        value
            .get(*key)
            .or_else(|| value.get("fields").and_then(|fields| fields.get(*key)))
            .and_then(|raw| {
                raw.as_i64()
                    .or_else(|| raw.as_u64().and_then(|num| i64::try_from(num).ok()))
                    .or_else(|| raw.as_str().and_then(|text| text.parse::<i64>().ok()))
            })
    })
}

/// List entities returned by an OData collection URL.
pub fn list_entities(ctx: &Context, url: &str, tenant: &str) -> Result<Vec<Value>, String> {
    let fields = ctx
        .entity_state
        .get("fields")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let headers = runtime_headers(ctx, tenant, &fields, None, Some("application/json"));
    let resp = ctx.http_call("GET", url, &headers, "")?;
    if resp.status != 200 {
        return Err(format!("GET {url} failed (HTTP {})", resp.status));
    }
    let parsed: Value = serde_json::from_str(&resp.body).unwrap_or_else(|_| json!({"value": []}));
    Ok(parsed
        .get("value")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default())
}

/// Find the best available ChannelSession for a bound agent.
pub fn find_channel_session_by_agent(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    agent_id: &str,
) -> Result<Option<Value>, String> {
    let escaped = agent_id.replace('\'', "''");
    let active_filter =
        format!("$filter=Status eq 'Active' and agent_entity_id eq '{escaped}'&$top=1");
    let active_url = format!("{temper_api_url}/tdata/ChannelSessions?{active_filter}");
    if let Some(session) = list_entities(ctx, &active_url, tenant)?.into_iter().next() {
        return Ok(Some(session));
    }

    let any_filter = format!("$filter=agent_entity_id eq '{escaped}'&$top=1");
    let any_url = format!("{temper_api_url}/tdata/ChannelSessions?{any_filter}");
    Ok(list_entities(ctx, &any_url, tenant)?.into_iter().next())
}

/// Find the connected Channel entity for a platform-specific channel ID.
pub fn find_connected_channel_by_external_id(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    channel_id: &str,
) -> Result<Option<Value>, String> {
    let escaped = channel_id.replace('\'', "''");
    let filter = format!("$filter=Status eq 'Connected' and channel_id eq '{escaped}'&$top=1");
    let url = format!("{temper_api_url}/tdata/Channels?{filter}");
    Ok(list_entities(ctx, &url, tenant)?.into_iter().next())
}

/// Parse a basic ISO 8601 timestamp (YYYY-MM-DDTHH:MM:SSZ) to Unix epoch seconds.
/// Returns None if the format is unrecognized.
pub fn parse_iso8601_to_epoch_secs(s: &str) -> Option<u64> {
    // Supported formats: "2026-03-24T12:30:00Z", "2026-03-24T12:30:00.000Z"
    let s = s.trim();
    if s.len() < 19 {
        return None;
    }

    let year: u64 = s.get(0..4)?.parse().ok()?;
    let month: u64 = s.get(5..7)?.parse().ok()?;
    let day: u64 = s.get(8..10)?.parse().ok()?;
    let hour: u64 = s.get(11..13)?.parse().ok()?;
    let minute: u64 = s.get(14..16)?.parse().ok()?;
    let second: u64 = s.get(17..19)?.parse().ok()?;

    if s.as_bytes().get(4) != Some(&b'-')
        || s.as_bytes().get(7) != Some(&b'-')
        || s.as_bytes().get(10) != Some(&b'T')
    {
        return None;
    }

    // Days in each month (non-leap)
    let days_in_month = [0, 31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let is_leap = (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0);

    // Days from epoch (1970-01-01) to start of `year`
    let mut days: u64 = 0;
    for y in 1970..year {
        let leap = (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0);
        days += if leap { 366 } else { 365 };
    }

    // Days from start of year to start of month
    for m in 1..month {
        days += days_in_month[m as usize];
        if m == 2 && is_leap {
            days += 1;
        }
    }

    // Days within month (1-indexed)
    days += day - 1;

    Some(days * 86400 + hour * 3600 + minute * 60 + second)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_batch_text_file_read_response_deserializes_found_and_missing_items() {
        let by_id = parse_batch_text_file_read_response(
            r#"{
                "files": [
                    {
                        "file_id": "file-a",
                        "found": true,
                        "content_hash": "sha256:file-a",
                        "mime_type": "application/json",
                        "text": "{\"ok\":true}"
                    },
                    {
                        "file_id": "file-missing",
                        "found": false,
                        "content_hash": "",
                        "mime_type": "",
                        "text": ""
                    }
                ]
            }"#,
        )
        .expect("parse batch response");

        assert!(by_id["file-a"].found);
        assert_eq!(by_id["file-a"].text, "{\"ok\":true}");
        assert_eq!(by_id["file-a"].content_hash, "sha256:file-a");
        assert!(!by_id["file-missing"].found);
        assert_eq!(by_id["file-missing"].text, "");
    }

    #[test]
    fn test_parse_iso8601() {
        // 2026-03-24T12:00:00Z
        let secs = parse_iso8601_to_epoch_secs("2026-03-24T12:00:00Z");
        assert!(secs.is_some());
        let s = secs.unwrap();
        // Rough sanity: should be > 2025-01-01 (~1735689600) and < 2027-01-01
        assert!(s > 1_735_000_000);
        assert!(s < 1_800_000_000);
    }

    #[test]
    fn test_parse_iso8601_with_millis() {
        let secs = parse_iso8601_to_epoch_secs("2026-03-24T12:00:00.123Z");
        assert!(secs.is_some());
    }

    #[test]
    fn test_parse_iso8601_invalid() {
        assert!(parse_iso8601_to_epoch_secs("").is_none());
        assert!(parse_iso8601_to_epoch_secs("not-a-date").is_none());
        assert!(parse_iso8601_to_epoch_secs("2026").is_none());
    }

    #[test]
    fn test_epoch_zero() {
        let secs = parse_iso8601_to_epoch_secs("1970-01-01T00:00:00Z");
        assert_eq!(secs, Some(0));
    }

    #[test]
    fn test_direct_field_str() {
        let val = serde_json::json!({"Name": "test", "id": "123"});
        assert_eq!(direct_field_str(&val, &["Name"]), Some("test"));
        assert_eq!(direct_field_str(&val, &["missing", "id"]), Some("123"));
        assert_eq!(direct_field_str(&val, &["missing"]), None);
    }

    #[test]
    fn test_entity_field_str() {
        let val = serde_json::json!({"fields": {"Status": "Active"}});
        assert_eq!(entity_field_str(&val, &["Status"]), Some("Active"));
    }

    #[test]
    fn test_session_entries_ref_round_trips_session_id() {
        let reference = session_entries_ref("ses-123");
        assert_eq!(reference, "session-entries:ses-123");
        assert_eq!(session_id_from_entries_ref(&reference), Some("ses-123"));
        assert!(is_session_entries_ref(&reference));
        assert_eq!(session_id_from_entries_ref("fl-123"), None);
    }

    #[test]
    fn next_session_entry_id_advances_numeric_suffix() {
        assert_eq!(next_session_entry_id("a", "u-1"), ("a-2".to_string(), 2));
        assert_eq!(next_session_entry_id("t", "a-17"), ("t-18".to_string(), 18));
    }

    #[test]
    fn next_session_entry_id_maps_initial_session_user_turn_to_logical_sequence() {
        assert_eq!(
            next_session_entry_id("a", "u-ss-019dd16f-0da6-7863-932c-f5a477da4f00-0"),
            ("a-2".to_string(), 2)
        );
    }

    #[test]
    fn next_session_entry_id_defaults_to_first_child_when_parent_suffix_is_not_numeric() {
        assert_eq!(
            next_session_entry_id("a", "legacy-leaf"),
            ("a-1".to_string(), 1)
        );
    }

    #[test]
    fn test_session_entries_jsonl_from_entities_sorts_and_reconstructs() {
        let entities = vec![
            json!({
                "EntryId": "u-1",
                "ParentEntryId": "h-1",
                "Sequence": 1,
                "EntryType": "message",
                "Role": "user",
                "Content": "\"hello\"",
                "Tokens": 2,
                "ExtraJson": "{}"
            }),
            json!({
                "EntryId": "h-1",
                "ParentEntryId": "",
                "Sequence": 0,
                "EntryType": "header",
                "Role": "",
                "Content": "",
                "Tokens": 0,
                "ExtraJson": "{\"version\":1}"
            }),
        ];

        let jsonl = session_entries_jsonl_from_entities(&entities);
        let lines: Vec<Value> = jsonl
            .lines()
            .map(|line| serde_json::from_str(line).expect("valid json line"))
            .collect();

        assert_eq!(lines[0]["id"], "h-1");
        assert_eq!(lines[0]["version"], 1);
        assert_eq!(lines[1]["id"], "u-1");
        assert_eq!(lines[1]["parentId"], "h-1");
        assert_eq!(lines[1]["content"], "hello");
    }

    #[test]
    fn session_entries_jsonl_supports_headerless_root_user() {
        let entities = vec![
            json!({
                "EntryId": "a-2",
                "ParentEntryId": "u-ss-1-0",
                "Sequence": 2,
                "EntryType": "message",
                "Role": "assistant",
                "Content": "[{\"type\":\"text\",\"text\":\"hi\"}]",
                "Tokens": 3,
                "ExtraJson": "{}"
            }),
            json!({
                "EntryId": "u-ss-1-0",
                "ParentEntryId": "",
                "Sequence": 1,
                "EntryType": "message",
                "Role": "user",
                "Content": "\"hello\"",
                "Tokens": 2,
                "ExtraJson": "{}"
            }),
        ];

        let jsonl = session_entries_jsonl_from_entities(&entities);
        let lines: Vec<Value> = jsonl
            .lines()
            .map(|line| serde_json::from_str(line).expect("valid json line"))
            .collect();

        assert_eq!(lines[0]["id"], "u-ss-1-0");
        assert_eq!(lines[0]["parentId"], Value::Null);
        assert_eq!(lines[1]["id"], "a-2");
        assert_eq!(lines[1]["parentId"], "u-ss-1-0");
    }

    #[test]
    fn session_entries_jsonl_repairs_missing_virtual_initial_root() {
        let entities = vec![
            json!({
                "EntryId": "a-1",
                "ParentEntryId": "u-ss-123-0",
                "Sequence": 1,
                "EntryType": "message",
                "Role": "assistant",
                "Content": "[{\"type\":\"tool_use\",\"id\":\"call-1\"}]",
                "Tokens": 3,
                "ExtraJson": "{}"
            }),
            json!({
                "EntryId": "t-2",
                "ParentEntryId": "a-1",
                "Sequence": 2,
                "EntryType": "message",
                "Role": "user",
                "Content": "[{\"type\":\"tool_result\",\"tool_use_id\":\"call-1\",\"content\":\"ok\"}]",
                "Tokens": 5,
                "ExtraJson": "{}"
            }),
        ];

        let jsonl =
            session_entries_jsonl_from_entities_with_synthetic_root(&entities, "ss-123", "prompt");
        let lines: Vec<Value> = jsonl
            .lines()
            .map(|line| serde_json::from_str(line).expect("valid json line"))
            .collect();

        assert_eq!(lines[0]["id"], "h-ss-123");
        assert_eq!(lines[1]["id"], "u-ss-123-0");
        assert_eq!(lines[1]["content"], "prompt");
        assert_eq!(lines[2]["id"], "a-1");
        assert_eq!(lines[2]["parentId"], "u-ss-123-0");
        assert_eq!(lines[3]["id"], "t-2");
        assert_eq!(lines[3]["parentId"], "a-1");
    }

    #[test]
    fn session_entry_create_body_shapes_header_and_user_entries() {
        let header_extra = json!({"version": 1});
        let header = session_entry_create_body(&SessionEntryCreateSpec {
            session_id: "ss-1",
            entry_id: "h-ss-1",
            parent_entry_id: None,
            sequence: 0,
            entry_type: "header",
            role: None,
            content: None,
            content_file_id: None,
            content_file_version_id: None,
            extra_json: Some(&header_extra),
            tokens: 0,
        })
        .expect("header body");
        let user_content = json!("hello");
        let user = session_entry_create_body(&SessionEntryCreateSpec {
            session_id: "ss-1",
            entry_id: "u-ss-1-0",
            parent_entry_id: Some("h-ss-1"),
            sequence: 1,
            entry_type: "message",
            role: Some("user"),
            content: Some(&user_content),
            content_file_id: None,
            content_file_version_id: None,
            extra_json: None,
            tokens: 1,
        })
        .expect("user body");

        assert_eq!(header["SessionId"], "ss-1");
        assert_eq!(header["EntryId"], "h-ss-1");
        assert_eq!(header["ParentEntryId"], "");
        assert_eq!(header["ExtraJson"], "{\"version\":1}");
        assert_eq!(user["EntryId"], "u-ss-1-0");
        assert_eq!(user["ParentEntryId"], "h-ss-1");
        assert_eq!(user["Role"], "user");
        assert_eq!(user["Content"], "\"hello\"");
    }

    #[test]
    fn session_entry_create_ack_accepts_matching_odata_state_response() {
        let created = parse_created_session_entry_ack(
            r#"{
                "entity_id": "se-123",
                "fields": {
                    "SessionId": "ss-1",
                    "EntryId": "a-2"
                }
            }"#,
            "ss-1",
            "a-2",
        )
        .expect("acknowledged create");

        assert_eq!(created.entity_id, "se-123");
        assert_eq!(created.entry_id, "a-2");
    }

    #[test]
    fn session_entry_create_ack_rejects_wrong_session_or_entry() {
        assert!(
            parse_created_session_entry_ack(
                r#"{"fields":{"SessionId":"ss-other","EntryId":"a-2"}}"#,
                "ss-1",
                "a-2",
            )
            .is_err()
        );
        assert!(
            parse_created_session_entry_ack(
                r#"{"fields":{"SessionId":"ss-1","EntryId":"a-3"}}"#,
                "ss-1",
                "a-2",
            )
            .is_err()
        );
    }

    #[test]
    fn session_entry_strict_readback_is_opt_in_by_field_or_config() {
        assert!(!session_entry_create_verify_readback_enabled(
            &json!({}),
            None,
        ));
        assert!(session_entry_create_verify_readback_enabled(
            &json!({"session_entry_create_verify_readback": "true"}),
            None,
        ));
        assert!(session_entry_create_verify_readback_enabled(
            &json!({}),
            Some("true"),
        ));
        assert!(!session_entry_create_verify_readback_enabled(
            &json!({"session_entry_create_verify_readback": "false"}),
            Some("true"),
        ));
    }

    #[test]
    fn session_entry_verify_response_requires_visible_row() {
        assert!(session_entry_verify_response_visible(
            r#"{"value":[{"EntryId":"u-1"}]}"#
        ));
        assert!(!session_entry_verify_response_visible(r#"{"value":[]}"#));
        assert!(!session_entry_verify_response_visible("not-json"));
    }

    #[test]
    fn session_entry_verify_missing_ids_accepts_all_expected_ids_in_one_response() {
        let missing = session_entry_verify_missing_ids(
            r#"{
                "value": [
                    {"fields": {"EntryId": "u-ss-1-0"}},
                    {"entry_id": "a-2"}
                ]
            }"#,
            &["u-ss-1-0", "a-2"],
        )
        .expect("parse verify response");

        assert!(missing.is_empty());
    }

    #[test]
    fn session_entry_verify_missing_ids_fails_closed_on_missing_or_invalid_response() {
        assert_eq!(
            session_entry_verify_missing_ids(
                r#"{"value":[{"EntryId":"h-ss-1"}]}"#,
                &["h-ss-1", "u-ss-1-0",]
            )
            .expect("parse verify response"),
            vec!["u-ss-1-0".to_string()]
        );
        assert!(session_entry_verify_missing_ids("not-json", &["h-ss-1"]).is_err());
        assert!(session_entry_verify_missing_ids(r#"{"items":[]}"#, &["h-ss-1"]).is_err());
    }

    #[test]
    fn test_session_entry_extra_json_omits_canonical_fields() {
        let entry = json!({
            "id": "c-1",
            "parentId": "a-1",
            "type": "compaction",
            "tokens": 10,
            "summary": "short",
            "first_kept": "u-1"
        });
        let extra = session_entry_extra_json(&entry);
        assert!(extra.get("id").is_none());
        assert_eq!(extra["summary"], "short");
        assert_eq!(extra["first_kept"], "u-1");
    }
}

/// Send a typing indicator to Discord for the given agent.
/// Best-effort: silently ignores all errors.
///
/// Looks up ChannelSession → Channel → webhook_url, then POSTs to /typing.
pub fn send_typing_indicator(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    agent_entity_id: &str,
) {
    let _ = send_typing_indicator_inner(ctx, temper_api_url, tenant, agent_entity_id);
}

fn send_typing_indicator_inner(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    agent_entity_id: &str,
) -> Result<(), String> {
    if agent_entity_id.is_empty() {
        return Ok(());
    }

    let fields = ctx
        .entity_state
        .get("fields")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let headers = runtime_headers(ctx, tenant, &fields, None, Some("application/json"));
    let parent_session_id =
        entity_field_str(&ctx.entity_state, &["parent_session_id", "ParentSessionId"])
            .unwrap_or("");

    // Find ChannelSession for this agent
    let session = find_channel_session(ctx, temper_api_url, &headers, agent_entity_id)
        .or_else(|| {
            if parent_session_id.is_empty() || parent_session_id == agent_entity_id {
                None
            } else {
                find_channel_session(ctx, temper_api_url, &headers, parent_session_id)
            }
        })
        .ok_or("no session")?;

    let channel_id = entity_field_str(&session, &["ChannelId", "channel_id"]).unwrap_or("");
    let thread_id = entity_field_str(&session, &["ThreadId", "thread_id"]).unwrap_or("");
    if channel_id.is_empty() || thread_id.is_empty() {
        return Ok(());
    }

    // Find Channel to get webhook_url
    let escaped_ch = channel_id.replace('\'', "''");
    let channel_url = format!(
        "{temper_api_url}/tdata/Channels?$filter=Status eq 'Connected' and channel_id eq '{escaped_ch}'&$top=1"
    );
    let ch_resp = ctx.http_call("GET", &channel_url, &headers, "")?;
    if ch_resp.status != 200 {
        return Ok(());
    }
    let channels: Value =
        serde_json::from_str(&ch_resp.body).unwrap_or_else(|_| json!({"value": []}));
    let channel = channels
        .get("value")
        .and_then(Value::as_array)
        .and_then(|arr| arr.first())
        .ok_or("no channel")?;

    let webhook_url = entity_field_str(channel, &["webhook_url", "WebhookUrl"]).unwrap_or("");
    if webhook_url.is_empty() {
        return Ok(());
    }

    // POST to /typing endpoint
    let typing_url = format!(
        "{}/typing",
        webhook_url.trim_end_matches('/').trim_end_matches("/reply")
    );
    let body = json!({"thread_id": thread_id});
    let wh_headers = vec![("content-type".to_string(), "application/json".to_string())];
    let _ = ctx.http_call("POST", &typing_url, &wh_headers, &body.to_string());
    Ok(())
}

fn find_channel_session(
    ctx: &Context,
    temper_api_url: &str,
    headers: &[(String, String)],
    agent_entity_id: &str,
) -> Option<Value> {
    let escaped = agent_entity_id.replace('\'', "''");
    let active_url = format!(
        "{temper_api_url}/tdata/ChannelSessions?$filter=Status eq 'Active' and agent_entity_id eq '{escaped}'&$top=1"
    );
    let active_resp = ctx.http_call("GET", &active_url, headers, "").ok()?;
    if active_resp.status == 200 {
        let sessions: Value =
            serde_json::from_str(&active_resp.body).unwrap_or_else(|_| json!({"value": []}));
        if let Some(session) = sessions
            .get("value")
            .and_then(Value::as_array)
            .and_then(|arr| arr.first())
        {
            return Some(session.clone());
        }
    }

    let fallback_url = format!(
        "{temper_api_url}/tdata/ChannelSessions?$filter=agent_entity_id eq '{escaped}'&$top=1"
    );
    let fallback_resp = ctx.http_call("GET", &fallback_url, headers, "").ok()?;
    if fallback_resp.status != 200 {
        return None;
    }
    let sessions: Value =
        serde_json::from_str(&fallback_resp.body).unwrap_or_else(|_| json!({"value": []}));
    sessions
        .get("value")
        .and_then(Value::as_array)
        .and_then(|arr| arr.first())
        .cloned()
}
