//! Workspace Provisioner — WASM module for creating TemperFS conversation storage.
//!
//! Creates a TemperFS Workspace, conversation File, manifest File, and session
//! tree JSONL file. This is the fast, always-needed provisioning step that runs
//! before the agent starts thinking. Sandbox provisioning is lazy (ADR-0022).
//!
//! Build: `cargo build --target wasm32-unknown-unknown --release`

use temper_wasm_sdk::prelude::*;
use wasm_helpers::{
    resolve_temper_api_url, runtime_headers, runtime_headers_for_workspace,
    write_temperfs_value_with_retry,
};

/// Entry point.
#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        ctx.log("info", "workspace_provisioner: starting");

        let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));

        let user_message = fields
            .get("user_message")
            .and_then(|v| v.as_str())
            .unwrap_or("");

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

        let (workspace_id, conversation_file_id, file_manifest_id, session_file_id, session_leaf_id) =
            if !prior_conversation_file_id.is_empty() {
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
                let fs_result = create_conversation_storage_in_workspace(
                    &ctx,
                    &temper_api_url,
                    tenant,
                    &prior_workspace_id,
                    entity_id,
                    user_message,
                );
                match fs_result {
                    Ok((conv, manifest, sess_file, sess_leaf)) => (
                        prior_workspace_id,
                        conv,
                        manifest,
                        sess_file,
                        sess_leaf,
                    ),
                    Err(e) => {
                        ctx.log(
                            "warn",
                            &format!(
                                "workspace_provisioner: configured workspace bootstrap failed: {e}. Falling back to inline."
                            ),
                        );
                        (
                            prior_workspace_id,
                            String::new(),
                            String::new(),
                            String::new(),
                            String::new(),
                        )
                    }
                }
            } else {
                // Fresh session — create new conversation storage.
                let fs_result =
                    create_conversation_storage(&ctx, &temper_api_url, tenant, entity_id, user_message);
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
                        (
                            String::new(),
                            String::new(),
                            String::new(),
                            String::new(),
                            String::new(),
                        )
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

    let (file_id, manifest_id, session_file_id, session_leaf_id) =
        create_session_storage_files(ctx, temper_api_url, tenant, &workspace_id, agent_id, user_message)?;

    Ok((workspace_id, file_id, manifest_id, session_file_id, session_leaf_id))
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
    create_session_storage_files(ctx, temper_api_url, tenant, workspace_id, agent_id, user_message)
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

    let manifest_resp = ctx.http_call("POST", &file_url, &file_headers, &manifest_body.to_string())?;

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

    let (session_file_id, session_leaf_id) =
        create_session_tree(ctx, temper_api_url, tenant, workspace_id, agent_id, user_message);

    Ok((file_id, manifest_id, session_file_id, session_leaf_id))
}

/// Create a session tree JSONL file in TemperFS.
/// Returns (session_file_id, session_leaf_id). Non-fatal on failure.
fn create_session_tree(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    workspace_id: &str,
    agent_id: &str,
    user_message: &str,
) -> (String, String) {
    let headers = workspace_headers(ctx, tenant, workspace_id, Some("application/json"), None);

    let session_file_body = json!({
        "FileId": format!("session-{agent_id}"),
        "workspace_id": workspace_id,
        "name": "session.jsonl",
        "mime_type": "text/plain",
        "path": "/session.jsonl"
    });
    let session_file_resp = match ctx.http_call(
        "POST",
        &format!("{temper_api_url}/tdata/Files"),
        &headers,
        &serde_json::to_string(&session_file_body).unwrap_or_default(),
    ) {
        Ok(resp) => resp,
        Err(e) => {
            ctx.log("warn", &format!("Failed to create session file: {e}"));
            return (String::new(), String::new());
        }
    };

    let session_file_id = if session_file_resp.status >= 200 && session_file_resp.status < 300 {
        let parsed: Value = serde_json::from_str(&session_file_resp.body).unwrap_or(json!({}));
        parsed
            .get("entity_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    } else {
        ctx.log(
            "warn",
            &format!(
                "Failed to create session file (HTTP {})",
                session_file_resp.status
            ),
        );
        return (String::new(), String::new());
    };

    if session_file_id.is_empty() {
        return (String::new(), String::new());
    }

    // Create a TemperFS file for the first user message content.
    let content_file_headers =
        workspace_headers(ctx, tenant, workspace_id, Some("application/json"), None);
    let content_file_body = json!({
        "workspace_id": workspace_id,
        "name": format!("msg-u-{agent_id}-0.txt"),
        "mime_type": "text/plain",
        "path": format!("/msg-u-{agent_id}-0.txt")
    });
    let content_file_resp = ctx.http_call(
        "POST",
        &format!("{temper_api_url}/tdata/Files"),
        &content_file_headers,
        &serde_json::to_string(&content_file_body).unwrap_or_default(),
    );
    let content_file_id = match content_file_resp {
        Ok(resp) if resp.status >= 200 && resp.status < 300 => {
            let parsed: Value = serde_json::from_str(&resp.body).unwrap_or(json!({}));
            let file_id = parsed
                .get("entity_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if !file_id.is_empty() {
                let content_write_url =
                    format!("{temper_api_url}/tdata/Files('{file_id}')/$value");
                let content_write_headers =
                    workspace_headers(ctx, tenant, workspace_id, Some("text/plain"), None);
                match ctx.http_call("PUT", &content_write_url, &content_write_headers, user_message)
                {
                    Ok(write_resp) if write_resp.status >= 200 && write_resp.status < 300 => {
                        Some(file_id)
                    }
                    _ => None,
                }
            } else {
                None
            }
        }
        _ => None,
    };

    // Initialize session file with JSONL header + first user message.
    let header_id = format!("h-{agent_id}");
    let header_entry = json!({
        "id": header_id,
        "parentId": null,
        "type": "header",
        "version": 1,
        "tokens": 0
    });
    let header_line = serde_json::to_string(&header_entry).unwrap_or_default();

    let session_leaf_id = format!("u-{agent_id}-0");
    let user_entry = if let Some(content_file_id) = content_file_id {
        json!({
            "id": session_leaf_id,
            "parentId": header_id,
            "type": "message",
            "role": "user",
            "content_file_id": content_file_id,
            "tokens": user_message.len() / 4
        })
    } else {
        json!({
            "id": session_leaf_id,
            "parentId": header_id,
            "type": "message",
            "role": "user",
            "content": user_message,
            "tokens": user_message.len() / 4
        })
    };
    let user_line = serde_json::to_string(&user_entry).unwrap_or_default();
    let initial_jsonl = format!("{header_line}\n{user_line}");

    let write_url = format!("{temper_api_url}/tdata/Files('{session_file_id}')/$value");
    let write_headers = workspace_headers(ctx, tenant, workspace_id, Some("text/plain"), None);
    match write_temperfs_value_with_retry(
        ctx,
        &write_url,
        &write_headers,
        &initial_jsonl,
        "Failed to write session file",
    ) {
        Ok(()) => {
            ctx.log("info", "workspace_provisioner: session tree initialized");
        }
        Err(e) => {
            ctx.log("warn", &e);
        }
    }

    (session_file_id, session_leaf_id)
}
