//! Workspace Provisioner — WASM module for creating TemperFS conversation storage.
//!
//! Creates the hot session log needed before the agent starts thinking.
//!
//! Fresh sessions default to SessionEntry hot state and avoid bootstrapping
//! empty TemperFS files. PawFS workspace files are opt-in for legacy flows and
//! governed artifacts; sandbox provisioning remains lazy (ADR-0022).
//!
//! Build: `cargo build --target wasm32-unknown-unknown --release`

use temper_wasm_sdk::prelude::*;
use wasm_helpers::{
    create_initial_session_entries, create_session_entry, resolve_temper_api_url, runtime_headers,
    runtime_headers_for_workspace, session_entries_ref, write_temperfs_value_with_retry,
};

fn elapsed_ms_since(started_at: i64) -> i64 {
    Context::get_time_millis().saturating_sub(started_at)
}

fn emit_phase_duration(ctx: &Context, phase: &str, started_at: i64, result: &str) {
    let elapsed_ms = elapsed_ms_since(started_at);
    let _ = ctx.emit_metric(
        "temper_session_phase_duration_ms",
        elapsed_ms as f64,
        &json!({
            "phase": phase,
            "result": result,
        }),
        Some("histogram"),
    );
    ctx.log(
        "info",
        &format!("session_phase phase={phase} result={result} elapsed_ms={elapsed_ms}"),
    );
}

fn emit_phase_step_duration(ctx: &Context, phase: &str, step: &str, started_at: i64, result: &str) {
    let elapsed_ms = elapsed_ms_since(started_at);
    let _ = ctx.emit_metric(
        "temper_session_phase_step_duration_ms",
        elapsed_ms as f64,
        &json!({
            "phase": phase,
            "step": step,
            "result": result,
        }),
        Some("histogram"),
    );
    ctx.log(
        "info",
        &format!("session_phase phase={phase} step={step} result={result} elapsed_ms={elapsed_ms}"),
    );
}

/// Entry point.
#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let started_at = Context::get_time_millis();
        let ctx = Context::from_host()?;
        ctx.log("info", "workspace_provisioner: starting");

        let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));

        // Read user_message via the ceiling-aware SDK helper so the read works
        // whether Temper stored the value inline or as a blob ref (>128KB).
        // See temper ADR-0045 and ADR-0046.
        let user_message_owned = ctx.read_field_string("user_message").unwrap_or_default();
        let user_message: &str = &user_message_owned;

        if user_message.is_empty() {
            return Err("agent not configured — user_message is empty".to_string());
        }

        let temper_api_url = resolve_temper_api_url(&ctx, &fields);
        let entity_id = ctx
            .entity_state
            .get("entity_id")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let tenant = &ctx.tenant;

        // Check for continuation context passed via Configure (resume fields).
        let prior_workspace_id = fields
            .get("workspace_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let prior_conversation_file_id = fields
            .get("conversation_file_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let prior_file_manifest_id = fields
            .get("file_manifest_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let prior_session_file_id = fields
            .get("session_file_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let prior_session_leaf_id = fields
            .get("session_leaf_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let (
            workspace_id,
            conversation_file_id,
            file_manifest_id,
            session_file_id,
            session_leaf_id,
        ) = if !prior_conversation_file_id.is_empty() || !prior_session_file_id.is_empty() {
            // Continuation session — reuse prior workspace and conversation storage.
            ctx.log(
                    "info",
                    &format!(
                        "workspace_provisioner: continuation detected — reusing workspace={prior_workspace_id} conv={prior_conversation_file_id} session={prior_session_file_id}"
                    ),
                );
            (
                prior_workspace_id,
                prior_conversation_file_id,
                prior_file_manifest_id,
                prior_session_file_id,
                prior_session_leaf_id,
            )
        } else if !prior_workspace_id.is_empty() {
            // Fresh session with an explicitly configured workspace.
            ctx.log(
                    "info",
                    &format!(
                        "workspace_provisioner: using configured workspace {prior_workspace_id} for fresh session storage"
                    ),
                );
            let bootstrap_started_at = Context::get_time_millis();
            let fs_result = if legacy_session_files_enabled(&ctx, &fields) {
                create_conversation_storage_in_workspace(
                    &ctx,
                    &temper_api_url,
                    tenant,
                    &prior_workspace_id,
                    entity_id,
                    user_message,
                )
            } else {
                Ok(create_hot_session_storage_in_workspace(
                    &ctx,
                    entity_id,
                    user_message,
                    &prior_workspace_id,
                ))
            };
            emit_phase_step_duration(
                &ctx,
                "workspace_provisioner",
                "bootstrap_existing_workspace",
                bootstrap_started_at,
                if fs_result.is_ok() { "ok" } else { "error" },
            );
            match fs_result {
                Ok((conv, manifest, sess_file, sess_leaf)) => {
                    (prior_workspace_id, conv, manifest, sess_file, sess_leaf)
                }
                Err(e) => {
                    ctx.log(
                            "warn",
                            &format!(
                                "workspace_provisioner: configured workspace bootstrap failed: {e}. Falling back to inline."
                            ),
                        );
                    create_hot_session_storage(&ctx, entity_id, user_message, &prior_workspace_id)
                }
            }
        } else {
            // Fresh session — create new conversation storage.
            let bootstrap_started_at = Context::get_time_millis();
            let fs_result = if legacy_session_files_enabled(&ctx, &fields) {
                create_conversation_storage(&ctx, &temper_api_url, tenant, entity_id, user_message)
            } else {
                Ok(create_hot_session_storage(&ctx, entity_id, user_message, ""))
            };
            emit_phase_step_duration(
                &ctx,
                "workspace_provisioner",
                "bootstrap_new_workspace",
                bootstrap_started_at,
                if fs_result.is_ok() { "ok" } else { "error" },
            );
            match fs_result {
                Ok((ws, conv, manifest, sess_file, sess_leaf)) => {
                    (ws, conv, manifest, sess_file, sess_leaf)
                }
                Err(e) => {
                    ctx.log(
                            "warn",
                            &format!(
                                "workspace_provisioner: TemperFS bootstrap failed at {temper_api_url}/tdata (tenant={tenant}, agent={entity_id}): {e}. Falling back to inline."
                            ),
                        );
                    create_hot_session_storage(&ctx, entity_id, user_message, "")
                }
            }
        };

        set_success_result(
            "WorkspaceReady",
            &json!({
                "workspace_id": workspace_id,
                "conversation_file_id": conversation_file_id,
                "file_manifest_id": file_manifest_id,
                "session_file_id": session_file_id,
                "session_leaf_id": session_leaf_id,
            }),
        );
        emit_phase_duration(&ctx, "workspace_provisioner", started_at, "workspace_ready");

        Ok(())
    })();

    if let Err(e) = result {
        set_error_result(&e);
    }
    0
}

fn agent_headers(
    ctx: &Context,
    tenant: &str,
    content_type: Option<&str>,
    accept: Option<&str>,
) -> Vec<(String, String)> {
    let fields = ctx
        .entity_state
        .get("fields")
        .cloned()
        .unwrap_or_else(|| json!({}));
    runtime_headers(ctx, tenant, &fields, content_type, accept)
}

fn workspace_headers(
    ctx: &Context,
    tenant: &str,
    workspace_id: &str,
    content_type: Option<&str>,
    accept: Option<&str>,
) -> Vec<(String, String)> {
    runtime_headers_for_workspace(ctx, tenant, &json!({}), workspace_id, content_type, accept)
}

fn bool_field_or_config(ctx: &Context, fields: &Value, key: &str, default_value: bool) -> bool {
    fields
        .get(key)
        .and_then(|value| value.as_str())
        .or_else(|| ctx.config.get(key).map(String::as_str))
        .map(|value| matches!(value.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes"))
        .unwrap_or(default_value)
}

fn legacy_session_files_enabled(ctx: &Context, fields: &Value) -> bool {
    bool_field_or_config(ctx, fields, "bootstrap_temperfs_session_files", false)
}

fn create_hot_session_storage(
    ctx: &Context,
    session_id: &str,
    user_message: &str,
    workspace_id: &str,
) -> (String, String, String, String, String) {
    let session_ref = session_entries_ref(session_id);
    let session_leaf_id = format!("u-{session_id}-0");
    let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));
    let temper_api_url = resolve_temper_api_url(ctx, &fields);
    let tenant = &ctx.tenant;

    if let Err(e) = create_initial_session_entries(
        ctx,
        &temper_api_url,
        tenant,
        &fields,
        session_id,
        user_message,
    ) {
        ctx.log(
            "warn",
            &format!("workspace_provisioner: hot session initial entries failed: {e}"),
        );
        return (
            workspace_id.to_string(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
        );
    }

    ctx.log(
        "info",
        "workspace_provisioner: hot session entries initialized without PawFS bootstrap",
    );
    (
        workspace_id.to_string(),
        String::new(),
        String::new(),
        session_ref,
        session_leaf_id,
    )
}

fn create_hot_session_storage_in_workspace(
    ctx: &Context,
    session_id: &str,
    user_message: &str,
    workspace_id: &str,
) -> (String, String, String, String) {
    let (_, conversation_file_id, file_manifest_id, session_file_id, session_leaf_id) =
        create_hot_session_storage(ctx, session_id, user_message, workspace_id);
    (
        conversation_file_id,
        file_manifest_id,
        session_file_id,
        session_leaf_id,
    )
}

/// Create a TemperFS Workspace, conversation File, manifest File, and session file.
/// Returns (workspace_entity_id, conversation_file_id, manifest_file_id, session_file_id, session_leaf_id).
fn create_conversation_storage(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    agent_id: &str,
    user_message: &str,
) -> Result<(String, String, String, String, String), String> {
    let headers = agent_headers(ctx, tenant, Some("application/json"), None);

    // 1. Create Workspace
    let ws_body = json!({
        "WorkspaceId": format!("agent-{agent_id}"),
        "name": format!("Agent {agent_id} Workspace"),
        "owner_id": agent_id,
        "quota_bytes": "104857600"
    });

    let ws_url = format!("{temper_api_url}/tdata/Workspaces");
    let ws_resp = ctx.http_call("POST", &ws_url, &headers, &ws_body.to_string())?;

    if ws_resp.status < 200 || ws_resp.status >= 300 {
        return Err(format!(
            "Workspace creation failed (HTTP {}): {}",
            ws_resp.status,
            &ws_resp.body[..ws_resp.body.len().min(300)]
        ));
    }

    let ws_parsed: Value = serde_json::from_str(&ws_resp.body)
        .map_err(|e| format!("parse workspace response: {e}"))?;
    let workspace_id = ws_parsed
        .get("entity_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    ctx.log(
        "info",
        &format!("workspace_provisioner: created workspace {workspace_id}"),
    );

    let (file_id, manifest_id, session_file_id, session_leaf_id) = create_session_storage_files(
        ctx,
        temper_api_url,
        tenant,
        &workspace_id,
        agent_id,
        user_message,
    )?;

    Ok((
        workspace_id,
        file_id,
        manifest_id,
        session_file_id,
        session_leaf_id,
    ))
}

/// Create conversation + manifest + session files inside an existing workspace.
fn create_conversation_storage_in_workspace(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    workspace_id: &str,
    agent_id: &str,
    user_message: &str,
) -> Result<(String, String, String, String), String> {
    if workspace_id.is_empty() {
        return Err("configured workspace_id is empty".to_string());
    }
    create_session_storage_files(
        ctx,
        temper_api_url,
        tenant,
        workspace_id,
        agent_id,
        user_message,
    )
}

/// Create conversation + manifest + session files inside the provided workspace.
/// Returns (conversation_file_id, manifest_file_id, session_file_id, session_leaf_id).
fn create_session_storage_files(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    workspace_id: &str,
    agent_id: &str,
    user_message: &str,
) -> Result<(String, String, String, String), String> {
    let file_url = format!("{temper_api_url}/tdata/Files");
    let file_headers = workspace_headers(ctx, tenant, workspace_id, Some("application/json"), None);

    // 2. Create File for conversation
    let file_body = json!({
        "FileId": format!("conv-{agent_id}"),
        "workspace_id": workspace_id,
        "name": "conversation.json",
        "mime_type": "application/json",
        "path": "/conversation.json"
    });

    let file_resp = ctx.http_call("POST", &file_url, &file_headers, &file_body.to_string())?;

    if file_resp.status < 200 || file_resp.status >= 300 {
        return Err(format!(
            "File creation failed (HTTP {}): {}",
            file_resp.status,
            &file_resp.body[..file_resp.body.len().min(300)]
        ));
    }

    let file_parsed: Value =
        serde_json::from_str(&file_resp.body).map_err(|e| format!("parse file response: {e}"))?;
    let file_id = file_parsed
        .get("entity_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    ctx.log(
        "info",
        &format!("workspace_provisioner: created conversation file {file_id}"),
    );

    // 3. Write initial empty conversation
    let init_conv = json!({"messages": []}).to_string();
    let value_url = format!("{temper_api_url}/tdata/Files('{file_id}')/$value");
    let value_headers =
        workspace_headers(ctx, tenant, workspace_id, Some("application/json"), None);
    if let Err(err) = write_temperfs_value_with_retry(
        ctx,
        &value_url,
        &value_headers,
        &init_conv,
        "workspace_provisioner: initial $value write failed",
    ) {
        ctx.log("warn", &err);
    }

    // 4. Create manifest File for sandbox fsync
    let manifest_body = json!({
        "FileId": format!("manifest-{agent_id}"),
        "workspace_id": workspace_id,
        "name": "file_manifest.json",
        "mime_type": "application/json",
        "path": "/file_manifest.json"
    });

    let manifest_resp =
        ctx.http_call("POST", &file_url, &file_headers, &manifest_body.to_string())?;

    if manifest_resp.status < 200 || manifest_resp.status >= 300 {
        return Err(format!(
            "Manifest File creation failed (HTTP {}): {}",
            manifest_resp.status,
            &manifest_resp.body[..manifest_resp.body.len().min(300)]
        ));
    }

    let manifest_parsed: Value = serde_json::from_str(&manifest_resp.body)
        .map_err(|e| format!("parse manifest response: {e}"))?;
    let manifest_id = manifest_parsed
        .get("entity_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    ctx.log(
        "info",
        &format!("workspace_provisioner: created manifest file {manifest_id}"),
    );

    // 5. Write initial empty manifest
    let init_manifest = json!({"files": {}, "synced_at_turn": 0}).to_string();
    let manifest_value_url = format!("{temper_api_url}/tdata/Files('{manifest_id}')/$value");
    if let Err(err) = write_temperfs_value_with_retry(
        ctx,
        &manifest_value_url,
        &value_headers,
        &init_manifest,
        "workspace_provisioner: initial manifest $value write failed",
    ) {
        ctx.log("warn", &err);
    }

    let (session_file_id, session_leaf_id) = create_session_tree(
        ctx,
        temper_api_url,
        tenant,
        workspace_id,
        agent_id,
        user_message,
    );

    Ok((file_id, manifest_id, session_file_id, session_leaf_id))
}

/// Create a Temper-native session tree using one SessionEntry entity per turn.
/// Returns (session_file_id, session_leaf_id). Non-fatal on failure.
fn create_session_tree(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    _workspace_id: &str,
    agent_id: &str,
    user_message: &str,
) -> (String, String) {
    let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));
    let session_ref = session_entries_ref(agent_id);
    let header_id = format!("h-{agent_id}");
    let session_leaf_id = format!("u-{agent_id}-0");

    let header_result = create_session_entry(
        ctx,
        temper_api_url,
        tenant,
        &fields,
        agent_id,
        &header_id,
        None,
        0,
        "header",
        None,
        None,
        None,
        None,
        Some(&json!({ "version": 1 })),
        0,
    );
    if let Err(e) = header_result {
        ctx.log(
            "warn",
            &format!("Failed to create session header entry: {e}"),
        );
        return (String::new(), String::new());
    }

    let user_result = create_session_entry(
        ctx,
        temper_api_url,
        tenant,
        &fields,
        agent_id,
        &session_leaf_id,
        Some(&header_id),
        1,
        "message",
        Some("user"),
        Some(&json!(user_message)),
        None,
        None,
        None,
        user_message.len() / 4,
    );
    match user_result {
        Ok(_) => ctx.log("info", "workspace_provisioner: session entries initialized"),
        Err(e) => {
            ctx.log(
                "warn",
                &format!("Failed to create initial user SessionEntry: {e}"),
            );
            return (String::new(), String::new());
        }
    }

    (session_ref, session_leaf_id)
}
