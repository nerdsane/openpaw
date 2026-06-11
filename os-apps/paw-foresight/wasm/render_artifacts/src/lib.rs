//! render_artifacts — turn the canonical path into the world's first
//! published texts (ADR-002).
//!
//! On World.Render: read the canonical path's repair log and its endpoint's
//! document bundle, then spawn ONE author session that drafts two artifacts —
//! a decision brief and the single best in-world document — and submits each
//! through the consistency gate (SubmitForCheck). The author never publishes:
//! the gate owns that transition. No follow-up action is dispatched.
//!
//! Build: `cargo build --target wasm32-unknown-unknown --release`

use temper_wasm_sdk::prelude::*;

const AUTHOR_TOOLS: &str =
    "temper_get,temper_list,temper_create,temper_action,temper_read,temper_write";

/// The author's working contract. Single source of truth for the entity
/// sets, action, and parameter names it must use — tested below.
fn author_prompt(
    world_id: &str,
    path_id: &str,
    repair_log_file_id: &str,
    bundle_file_id: &str,
    target_date: &str,
) -> String {
    let repair_line = if repair_log_file_id.is_empty() {
        "No repair log file is available; rely on the EventNodes.".to_string()
    } else {
        format!("Read the canonical path's repair log: temper.read(\"{repair_log_file_id}\")")
    };
    let bundle_line = if bundle_file_id.is_empty() {
        "No endpoint bundle file is available; derive the document from the repair log and \
         the EventNodes."
            .to_string()
    } else {
        format!("Read the endpoint's document bundle: temper.read(\"{bundle_file_id}\")")
    };
    format!(
        "You are writing the first published texts of world {world_id}.\n\n\
         {repair_line}\n\
         {bundle_line}\n\
         Read the world's EventNodes: temper.list(\"EventNodes\") filtered to world_id \
         {world_id} — they are the only facts you may stand on.\n\n\
         Produce TWO artifacts:\n\
         (a) kind \"brief\" — a decision brief that opens with the single most consequential \
         claim and its date, then the load-bearing assumptions, each citing the EventNode ids \
         it rests on.\n\
         (b) kind \"document\" — the single best in-world document from the bundle, polished, \
         dated at the target date ({target_date}).\n\n\
         For each artifact, in order:\n\
         1. temper.create(\"Artifacts\", {{\"world_id\": \"{world_id}\", \"path_id\": \
         \"{path_id}\", \"kind\": \"brief|document\", \"title\": \"...\", \
         \"author_agent_id\": \"{{AGENT_ID}}\"}}) — capture the returned artifact id.\n\
         2. temper.write the full content (markdown) — capture the returned file id.\n\
         3. temper.action(\"Artifacts\", \"<artifact-id>\", \"SubmitForCheck\", \
         {{\"content_file_id\": \"<file-id>\", \"cited_node_ids\": \"[\\\"...\\\"]\"}})\n\n\
         Every factual sentence should trace to a cited node — the consistency gate will \
         check every citation. Do not publish; submission is where your authority ends.\n\n\
         Then call temper.done(\"complete\")."
    )
}

#[allow(clippy::too_many_arguments)]
fn spawn_session(
    ctx: &Context,
    api: &str,
    headers: &[(String, String)],
    name: &str,
    role: &str,
    model: &str,
    provider: &str,
    tools: &str,
    max_turns: &str,
    user_message: &str,
) -> Result<String, String> {
    let agent_body = json!({ "Name": name, "Role": role });
    let agent_resp = ctx.http_call(
        "POST",
        &format!("{api}/tdata/Agents"),
        headers,
        &agent_body.to_string(),
    )?;
    if agent_resp.status < 200 || agent_resp.status >= 300 {
        return Err(format!(
            "create Agent {name} failed (HTTP {})",
            agent_resp.status
        ));
    }
    let agent_id = serde_json::from_str::<Value>(&agent_resp.body)
        .ok()
        .and_then(|v| {
            v.get("entity_id")
                .and_then(|x| x.as_str())
                .map(str::to_string)
        })
        .ok_or("Agent create returned no entity_id")?;

    let session_resp = ctx.http_call(
        "POST",
        &format!("{api}/tdata/Sessions"),
        headers,
        &json!({ "agent_id": agent_id }).to_string(),
    )?;
    if session_resp.status < 200 || session_resp.status >= 300 {
        return Err(format!(
            "create Session for {name} failed (HTTP {})",
            session_resp.status
        ));
    }
    let session_id = serde_json::from_str::<Value>(&session_resp.body)
        .ok()
        .and_then(|v| {
            v.get("entity_id")
                .and_then(|x| x.as_str())
                .map(str::to_string)
        })
        .ok_or("Session create returned no entity_id")?;

    // The prompt needs the real agent id for author fields; substitute now.
    let message = user_message.replace("{AGENT_ID}", &agent_id);
    let configure_body = json!({
        "model": model,
        "provider": provider,
        "agent_name": role,
        "tools_enabled": tools,
        "max_turns": max_turns,
        "user_message": message,
        "sandbox_url": "none",
        "temper_api_url": api
    });
    let configure_resp = ctx.http_call(
        "POST",
        &format!("{api}/tdata/Sessions('{session_id}')/TemperPaw.Configure"),
        headers,
        &configure_body.to_string(),
    )?;
    if configure_resp.status < 200 || configure_resp.status >= 300 {
        return Err(format!(
            "Configure for {name} failed (HTTP {}): {}",
            configure_resp.status,
            &configure_resp.body[..configure_resp.body.len().min(200)]
        ));
    }
    ctx.log(
        "info",
        &format!("render_artifacts: spawned {role} agent {agent_id} session {session_id}"),
    );
    Ok(session_id)
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
        let path_id = get("canonical_path_id");
        if path_id.is_empty() {
            ctx.log(
                "warn",
                &format!(
                    "render_artifacts: world {world_id} has no canonical path yet; nothing to render"
                ),
            );
            // A successful run with nothing to dispatch must still set a
            // result: the host treats an empty result as failure.
            set_success_result("", &json!({}));
            return Ok(());
        }
        let model = get("agent_model");
        let provider = get("agent_provider");
        if model.trim().is_empty() || provider.trim().is_empty() {
            return Err("World.agent_model and World.agent_provider are required".to_string());
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
            ("x-temper-principal-id".to_string(), world_id.clone()),
            ("x-temper-agent-type".to_string(), "system".to_string()),
        ];

        // 1. The canonical path: repair log + endpoint.
        let path_resp = ctx.http_call(
            "GET",
            &format!("{api}/tdata/Paths('{path_id}')"),
            &headers,
            "",
        )?;
        if path_resp.status < 200 || path_resp.status >= 300 {
            return Err(format!(
                "failed to read canonical path {path_id} (HTTP {})",
                path_resp.status
            ));
        }
        let path: Value = serde_json::from_str(&path_resp.body).unwrap_or(json!({}));
        let path_str = |k: &str| {
            path.get(k)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string()
        };
        let repair_log_file_id = path_str("RepairLogFileId");
        let endpoint_id = path_str("EndpointId");

        // 2. The endpoint's bundle. Missing bundle degrades the prompt, not
        //    the render: the repair log and nodes still carry the world.
        let mut bundle_file_id = String::new();
        if endpoint_id.is_empty() {
            ctx.log(
                "warn",
                &format!("render_artifacts: canonical path {path_id} has no endpoint_id"),
            );
        } else {
            let endpoint_resp = ctx.http_call(
                "GET",
                &format!("{api}/tdata/Endpoints('{endpoint_id}')"),
                &headers,
                "",
            )?;
            if endpoint_resp.status >= 200 && endpoint_resp.status < 300 {
                bundle_file_id = serde_json::from_str::<Value>(&endpoint_resp.body)
                    .ok()
                    .and_then(|v| {
                        v.get("BundleFileId")
                            .and_then(|x| x.as_str())
                            .map(str::to_string)
                    })
                    .unwrap_or_default();
            } else {
                ctx.log(
                    "warn",
                    &format!(
                        "render_artifacts: could not read endpoint {endpoint_id} (HTTP {})",
                        endpoint_resp.status
                    ),
                );
            }
        }

        // 3. One author session drafts both artifacts.
        let prompt = author_prompt(
            &world_id,
            &path_id,
            &repair_log_file_id,
            &bundle_file_id,
            &get("target_date"),
        );
        spawn_session(
            &ctx,
            &api,
            &headers,
            &format!("RendererAuthor-{world_id}"),
            "renderer-author",
            &model,
            &provider,
            AUTHOR_TOOLS,
            "50",
            &prompt,
        )?;

        ctx.log(
            "info",
            &format!(
                "render_artifacts: author spawned for world {world_id} (canonical path {path_id}); artifacts will arrive via SubmitForCheck"
            ),
        );
        // A successful run with nothing to dispatch must still set a
        // result: the host treats an empty result as failure.
        set_success_result("", &json!({}));
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

    // Prompt-contract tests: the generated prompt must reference the exact
    // entity sets, action names, and parameter names the specs declare.
    // The API silently drops unknown fields — drift here is a silent failure.

    #[test]
    fn author_prompt_carries_the_artifact_create_contract() {
        let p = author_prompt("w-1", "p-9", "file-log", "file-bundle", "2027-06-30");
        for needle in [
            "temper.create(\"Artifacts\"",
            "\"world_id\": \"w-1\"",
            "\"path_id\": \"p-9\"",
            "\"kind\"",
            "\"title\"",
            "\"author_agent_id\": \"{AGENT_ID}\"",
        ] {
            assert!(p.contains(needle), "author prompt missing: {needle}");
        }
    }

    #[test]
    fn author_prompt_carries_the_submit_for_check_contract() {
        let p = author_prompt("w-1", "p-9", "file-log", "file-bundle", "2027-06-30");
        for needle in [
            "temper.action(\"Artifacts\", \"<artifact-id>\", \"SubmitForCheck\"",
            "content_file_id",
            "cited_node_ids",
            "temper.write",
            "temper.done",
        ] {
            assert!(p.contains(needle), "author prompt missing: {needle}");
        }
        // The author submits to the gate; it never publishes.
        assert!(!p.contains("\"Publish\""));
        assert!(!p.contains("PassCheck"));
    }

    #[test]
    fn author_prompt_demands_both_kinds() {
        let p = author_prompt("w-1", "p-9", "file-log", "file-bundle", "2027-06-30");
        assert!(p.contains("kind \"brief\""), "must demand the brief");
        assert!(p.contains("kind \"document\""), "must demand the document");
        assert!(p.contains("TWO artifacts"));
        assert!(p.contains("temper.read(\"file-log\")"));
        assert!(p.contains("temper.read(\"file-bundle\")"));
        assert!(p.contains("2027-06-30"), "document is dated at the target date");
    }

    #[test]
    fn author_prompt_degrades_honestly_without_files() {
        let p = author_prompt("w-1", "p-9", "", "", "2027-06-30");
        assert!(!p.contains("temper.read(\"\")"));
        assert!(p.contains("No repair log file is available"));
        assert!(p.contains("No endpoint bundle file is available"));
    }
}
