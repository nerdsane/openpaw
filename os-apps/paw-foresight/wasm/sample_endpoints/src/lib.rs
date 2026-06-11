//! sample_endpoints — starts a corridor pass for a world (ADR-002).
//!
//! On World.SampleEndpoints: for each budget slot, create an Endpoint entity
//! carrying a deterministically assigned driver stance (modal first, then an
//! anti-modal spread), then spawn an endpoint-writer session against it. Each
//! writer produces a document bundle native to the world's target date and
//! self-reports Endpoint.SubmitForRepair.
//!
//! Hindcast worlds (hindcast_mode = "true") get web tools stripped at
//! Configure time: evidence comes only from the frozen corpus.
//!
//! Build: `cargo build --target wasm32-unknown-unknown --release`

use temper_wasm_sdk::prelude::*;

const WRITER_TOOLS: &str =
    "temper_get,temper_list,temper_create,temper_action,temper_read,temper_write";
const WEB_TOOLS: &str = ",temper_web_search,temper_web_fetch";

fn tools_enabled(hindcast: bool) -> String {
    if hindcast {
        WRITER_TOOLS.to_string()
    } else {
        format!("{WRITER_TOOLS}{WEB_TOOLS}")
    }
}

/// How many endpoints to sample this pass. Defaults to 3, hard-capped at 5 —
/// token spend grows linearly with writers, and each endpoint fans out into
/// repairer and adversary sessions besides.
fn endpoint_budget(raw: &str) -> usize {
    raw.trim().parse::<usize>().unwrap_or(3).min(5)
}

/// Deterministic driver stance for the i-th endpoint writer. Slot 0 is the
/// modal future; every later slot is anti-modal on one load-bearing
/// uncertainty so the pass spans the distribution instead of resampling
/// consensus. Pure: same i, same stance, on every rerun.
fn driver_stance(i: usize) -> String {
    match i {
        0 => "modal: take the consensus view on every major uncertainty".to_string(),
        1 => "anti-modal: take the 85th-percentile-surprise view on the domain's single most \
              load-bearing uncertainty, consensus elsewhere"
            .to_string(),
        2 => "anti-modal: take the 15th-percentile (disappointment) view on the domain's single \
              most load-bearing uncertainty, consensus elsewhere"
            .to_string(),
        _ => format!(
            "anti-modal: pick the {i}-th most load-bearing uncertainty and take a tail view on \
             it, consensus elsewhere"
        ),
    }
}

/// The endpoint writer's working contract. Single source of truth for the
/// action and parameter names it must use — tested below, asserted nowhere
/// else.
#[allow(clippy::too_many_arguments)]
fn endpoint_writer_prompt(
    world_id: &str,
    endpoint_id: &str,
    agent_id: &str,
    name: &str,
    domain: &str,
    target_date: &str,
    stance: &str,
    corpus_file_id: &str,
    driver_config_file_id: &str,
    hindcast: bool,
) -> String {
    let corpus_line = if corpus_file_id.is_empty() {
        "No corpus file was provided.".to_string()
    } else {
        format!(
            "Read the world corpus: temper.read(\"{corpus_file_id}\") — the domain documents \
             this world is grounded in."
        )
    };
    let driver_line = if driver_config_file_id.is_empty() {
        String::new()
    } else {
        format!(
            "Read the driver basis: temper.read(\"{driver_config_file_id}\") — the named \
             drivers your stance bends.\n"
        )
    };
    let research_line = if hindcast {
        "This is a HINDCAST world: you have NO web access by design. Use only the corpus and \
         the skeleton, and never reference anything dated after the world's vantage."
            .to_string()
    } else {
        "Use temper.web_search / temper.web_fetch to ground your future in real present-day \
         actors, products, and prices."
            .to_string()
    };
    format!(
        "You are the endpoint writer for world {world_id} (\"{name}\", domain: {domain}), \
         endpoint {endpoint_id}.\n\n\
         You are writing DOCUMENTS NATIVE TO {target_date}: artifacts that exist inside that \
         future, not predictions about it. Write under this driver stance: {stance}\n\n\
         First read the skeleton: temper.list(\"EventNodes\", \"world_id eq '{world_id}'\"). \
         Nodes with provenance \"determined\" are settled facts — your future may not \
         contradict any \"determined\" node.\n\
         {corpus_line}\n{driver_line}{research_line}\n\n\
         Then write a document bundle: 2-4 documents, every one dated AT {target_date}:\n\
         - a retrospective or postmortem looking back from {target_date},\n\
         - a news item,\n\
         - at least one in-world primary document (a filing, a review, a changelog).\n\
         Documents must contain specific dates, named actors, and numbers — vague futures \
         cannot be repaired.\n\n\
         Save the whole bundle as ONE markdown file with temper.write, then self-report:\n\
         temper.action(\"Endpoints\", \"{endpoint_id}\", \"SubmitForRepair\", \
         {{\"bundle_file_id\": \"<file-id-from-temper.write>\", \"summary\": \"<one line>\", \
         \"author_agent_id\": \"{agent_id}\"}})\n\
         Then call temper.done(\"complete\")."
    )
}

/// Workspace name for a world — the cross-module rendezvous key. Every
/// session-spawning module must resolve the same per-world workspace by
/// this exact name.
fn workspace_name(world_id: &str) -> String {
    format!("world-{world_id}")
}

/// Resolve (or create) the per-world PawFS workspace and return its id.
/// Sessions are Configured with this id so temper.write lands inside a
/// workspace PawFS Cedar accepts — without it, File create is denied
/// (resource.workspaceId must match the principal's workspace).
fn ensure_world_workspace(
    ctx: &Context,
    api: &str,
    headers: &[(String, String)],
    world_id: &str,
) -> Result<String, String> {
    let name = workspace_name(world_id);
    let list_resp = ctx.http_call(
        "GET",
        &format!("{api}/tdata/Workspaces?$filter=name eq '{name}'"),
        headers,
        "",
    )?;
    if list_resp.status < 200 || list_resp.status >= 300 {
        return Err(format!(
            "list Workspaces for {name} failed (HTTP {})",
            list_resp.status
        ));
    }
    let body: Value = serde_json::from_str(&list_resp.body).unwrap_or(json!({}));
    let rows = body
        .as_array()
        .cloned()
        .or_else(|| body.get("value").and_then(|v| v.as_array()).cloned())
        .unwrap_or_default();
    for row in &rows {
        let row_name = row
            .get("fields")
            .and_then(|f| f.get("name"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if row_name == name {
            if let Some(id) = row.get("entity_id").and_then(|v| v.as_str()) {
                return Ok(id.to_string());
            }
        }
    }
    let create_resp = ctx.http_call(
        "POST",
        &format!("{api}/tdata/Workspaces"),
        headers,
        &json!({ "name": name, "quota_limit": "104857600" }).to_string(),
    )?;
    if create_resp.status < 200 || create_resp.status >= 300 {
        return Err(format!(
            "create Workspace {name} failed (HTTP {})",
            create_resp.status
        ));
    }
    serde_json::from_str::<Value>(&create_resp.body)
        .ok()
        .and_then(|v| {
            v.get("entity_id")
                .and_then(|x| x.as_str())
                .map(str::to_string)
        })
        .ok_or_else(|| format!("Workspace {name} create returned no entity_id"))
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
    workspace_id: &str,
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
        "workspace_id": workspace_id,
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
        &format!("sample_endpoints: spawned {role} agent {agent_id} session {session_id}"),
    );
    Ok(agent_id)
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
        let model = get("agent_model");
        let provider = get("agent_provider");
        if model.trim().is_empty() || provider.trim().is_empty() {
            return Err("World.agent_model and World.agent_provider are required".to_string());
        }
        let hindcast = get("hindcast_mode") == "true";
        let tools = tools_enabled(hindcast);
        let budget = endpoint_budget(&get("endpoint_budget"));

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

        // One workspace per world, resolved once for the whole pass: every
        // writer session writes its bundle there. Without it, temper.write
        // fails Cedar — hard error.
        let workspace_id = ensure_world_workspace(&ctx, &api, &headers, &world_id)?;

        for i in 0..budget {
            let stance = driver_stance(i);

            // The Endpoint exists before its writer does: the writer
            // self-reports SubmitForRepair against a real entity id.
            let endpoint_body = json!({
                "world_id": world_id,
                "driver_config": json!({ "stance": stance }).to_string(),
            });
            let endpoint_resp = ctx.http_call(
                "POST",
                &format!("{api}/tdata/Endpoints"),
                &headers,
                &endpoint_body.to_string(),
            )?;
            if endpoint_resp.status < 200 || endpoint_resp.status >= 300 {
                return Err(format!(
                    "create Endpoint {i} failed (HTTP {})",
                    endpoint_resp.status
                ));
            }
            let endpoint_id = serde_json::from_str::<Value>(&endpoint_resp.body)
                .ok()
                .and_then(|v| {
                    v.get("entity_id")
                        .and_then(|x| x.as_str())
                        .map(str::to_string)
                })
                .ok_or("Endpoint create returned no entity_id")?;

            let writer_msg = endpoint_writer_prompt(
                &world_id,
                &endpoint_id,
                "{AGENT_ID}",
                &get("name"),
                &get("domain"),
                &get("target_date"),
                &stance,
                &get("corpus_file_id"),
                &get("driver_config_file_id"),
                hindcast,
            );
            spawn_session(
                &ctx,
                &api,
                &headers,
                &format!("EndpointWriter-{world_id}-{i}"),
                "endpoint-writer",
                &model,
                &provider,
                &tools,
                "50",
                &writer_msg,
                &workspace_id,
            )?;
        }

        set_success_result("EndpointsSampled", &json!({}));
        ctx.log(
            "info",
            &format!("sample_endpoints: spawned {budget} endpoint writers for world {world_id}"),
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

    // Prompt-contract tests: the generated prompts must reference the exact
    // entity sets, action names, and parameter names the specs declare.
    // The API silently drops unknown fields — drift here is a silent failure.

    fn writer_prompt(stance: &str, hindcast: bool) -> String {
        endpoint_writer_prompt(
            "w-1",
            "e-1",
            "a-1",
            "Test",
            "ai coding tools",
            "2026-12-11",
            stance,
            "file-9",
            "file-7",
            hindcast,
        )
    }

    #[test]
    fn stance_assignment_is_deterministic_modal_first_anti_modal_after() {
        assert_eq!(driver_stance(0), driver_stance(0));
        assert_eq!(driver_stance(4), driver_stance(4));
        assert!(driver_stance(0).starts_with("modal:"));
        for i in 1..6 {
            assert!(
                driver_stance(i).starts_with("anti-modal:"),
                "stance {i} must be anti-modal"
            );
        }
        assert!(driver_stance(1).contains("85th-percentile"));
        assert!(driver_stance(2).contains("15th-percentile"));
        assert!(driver_stance(3).contains('3'));
        assert!(driver_stance(4).contains('4'));
        assert_ne!(driver_stance(3), driver_stance(4));
    }

    #[test]
    fn budget_defaults_to_three_and_caps_at_five() {
        assert_eq!(endpoint_budget(""), 3);
        assert_eq!(endpoint_budget("junk"), 3);
        assert_eq!(endpoint_budget("-1"), 3);
        assert_eq!(endpoint_budget("1"), 1);
        assert_eq!(endpoint_budget("5"), 5);
        assert_eq!(endpoint_budget("10"), 5);
    }

    #[test]
    fn writer_prompt_carries_the_submit_for_repair_contract() {
        let p = writer_prompt(&driver_stance(0), false);
        for needle in [
            "temper.action(\"Endpoints\", \"e-1\", \"SubmitForRepair\"",
            "\"bundle_file_id\"",
            "\"summary\"",
            "\"author_agent_id\": \"a-1\"",
        ] {
            assert!(p.contains(needle), "writer prompt missing: {needle}");
        }
    }

    #[test]
    fn writer_prompt_is_native_to_the_target_date_under_its_stance() {
        let stance = driver_stance(1);
        let p = writer_prompt(&stance, false);
        assert!(p.contains("NATIVE TO 2026-12-11"));
        assert!(p.contains("dated AT 2026-12-11"));
        assert!(p.contains(&stance), "the assigned stance must appear");
    }

    #[test]
    fn writer_prompt_enforces_the_skeleton_constraint() {
        let p = writer_prompt(&driver_stance(0), false);
        assert!(p.contains("temper.list(\"EventNodes\", \"world_id eq 'w-1'\")"));
        assert!(p.contains("may not contradict any \"determined\" node"));
    }

    #[test]
    fn workspace_name_is_the_cross_module_rendezvous_key() {
        // All six session-spawning modules must derive the exact same
        // workspace name from a world id, or their sessions write into
        // different workspaces.
        assert_eq!(workspace_name("w-1"), "world-w-1");
        assert_eq!(workspace_name("0197abc"), "world-0197abc");
    }

    #[test]
    fn hindcast_mode_strips_web_tools_and_pins_the_vantage() {
        assert!(!tools_enabled(true).contains("web"));
        assert!(tools_enabled(false).contains("temper_web_search"));
        let p = writer_prompt(&driver_stance(0), true);
        assert!(p.contains("NO web access"));
        assert!(p.contains("never reference anything dated after the world's vantage"));
        assert!(!p.contains("temper.web_search /"));
        let open = writer_prompt(&driver_stance(0), false);
        assert!(open.contains("temper.web_search"));
    }
}
