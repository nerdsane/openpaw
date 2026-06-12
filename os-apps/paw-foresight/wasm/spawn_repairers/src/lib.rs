//! spawn_repairers — attaches a repair path to a submitted endpoint (ADR-002).
//!
//! On Endpoint.SubmitForRepair: create one Path entity, create a fresh
//! repairer agent, bind it to the path, and spawn its session. The repairer
//! works BACKWARD from the endpoint's documents to the skeleton, proposing
//! the intermediate EventNodes the future requires and flagging every place
//! it had to bend the world. It self-reports Path.RepairComplete.
//!
//! Hindcast worlds (hindcast_mode = "true") get web tools stripped at
//! Configure time: evidence comes only from the frozen corpus.
//!
//! Build: `cargo build --target wasm32-unknown-unknown --release`

use temper_wasm_sdk::prelude::*;

const REPAIRER_TOOLS: &str =
    "temper_get,temper_list,temper_create,temper_action,temper_read,temper_write";
const WEB_TOOLS: &str = ",temper_web_search,temper_web_fetch";

fn tools_enabled(hindcast: bool) -> String {
    if hindcast {
        REPAIRER_TOOLS.to_string()
    } else {
        format!("{REPAIRER_TOOLS}{WEB_TOOLS}")
    }
}

/// Read one property from a single-entity OData response. Properties arrive
/// either snake_case inside a "fields" object or PascalCase at the top
/// level, depending on the endpoint shape — accept both.
fn entity_field(entity: &Value, snake: &str, pascal: &str) -> String {
    entity
        .get("fields")
        .and_then(|f| f.get(snake))
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .or_else(|| entity.get(pascal).and_then(|v| v.as_str()))
        .unwrap_or("")
        .to_string()
}

/// The repairer's working contract. Single source of truth for the action
/// and parameter names it must use — tested below, asserted nowhere else.
fn repairer_prompt(
    path_id: &str,
    endpoint_id: &str,
    world_id: &str,
    agent_id: &str,
    bundle_file_id: &str,
    summary: &str,
    hindcast: bool,
) -> String {
    let bundle_line = if bundle_file_id.is_empty() {
        "The endpoint reported no bundle file; work from the summary and the skeleton."
            .to_string()
    } else {
        format!(
            "Read the endpoint bundle first: temper.read(\"{bundle_file_id}\") — the documents \
             native to the target date that you must connect to the present."
        )
    };
    let summary_line = if summary.is_empty() {
        String::new()
    } else {
        format!("Writer's summary: {summary}\n")
    };
    let research_line = if hindcast {
        "This is a HINDCAST world: you have NO web access by design. Judge lags and incentives \
         from the corpus and the skeleton, and never reference anything dated after the \
         world's vantage."
            .to_string()
    } else {
        "Use temper.web_search / temper.web_fetch to check historical durations and actor \
         incentives against reality."
            .to_string()
    };
    format!(
        "You are the Repairer for path {path_id}, endpoint {endpoint_id}, world {world_id}.\n\n\
         {bundle_line}\n\
         {summary_line}Then read the skeleton: temper.list(\"EventNodes\", \"world_id eq \
         '{world_id}'\"). Nodes with provenance \"determined\" are settled facts.\n\
         {research_line}\n\n\
         Work BACKWARD from the documents: for this future to exist, what must have happened, \
         by when, done by whom? Derive the chain of intermediate events from the endpoint back \
         to the skeleton.\n\n\
         For each required intermediate event, propose an EventNode:\n\
         temper.create(\"EventNodes\", {{\"world_id\": \"{world_id}\", \"statement\": \"...\", \
         \"layer\": \"mid|fast\", \"probability\": \"<honest 0-1>\", \"provenance\": \
         \"authored\", \"source_refs\": \"[]\", \"resolve_by\": \"YYYY-MM-DD\", \
         \"author_agent_id\": \"{agent_id}\"}})\n\n\
         Flag every place you bend the world, honestly. Kinds:\n\
         - \"contradiction\": the repair conflicts with a determined node\n\
         - \"incentive\": an actor must act against its interests\n\
         - \"lag\": a process compressed below its historical duration\n\
         - \"miracle\": an unexplained discontinuity\n\
         Severity: \"low\" | \"medium\" | \"high\". You flag costs; you NEVER compute scores — \
         costing is deterministic and runs elsewhere.\n\n\
         Write a repair log with temper.write (markdown: the backward chain with your \
         reasoning), then self-report:\n\
         temper.action(\"Paths\", \"{path_id}\", \"RepairComplete\", {{\"repair_log_file_id\": \
         \"<file-id-from-temper.write>\", \"required_node_ids\": \"[\\\"<event-node-id>\\\", \
         ...]\", \"cost_flags\": \"[{{\\\"kind\\\": \\\"...\\\", \\\"severity\\\": \\\"...\\\", \
         \\\"note\\\": \\\"...\\\"}}]\"}})\n\
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
    // Workspace rows are not readable by agent principals (paw-fs Cedar has
    // no read/list permit on Workspace), so idempotent lookup is impossible:
    // create one per spawn batch. Correctness needs only that each session's
    // Configure workspace matches the files it writes; file READS are
    // unrestricted, so cross-session reads work across workspaces.
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

fn create_agent(
    ctx: &Context,
    api: &str,
    headers: &[(String, String)],
    name: &str,
    role: &str,
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
    serde_json::from_str::<Value>(&agent_resp.body)
        .ok()
        .and_then(|v| {
            v.get("entity_id")
                .and_then(|x| x.as_str())
                .map(str::to_string)
        })
        .ok_or_else(|| "Agent create returned no entity_id".to_string())
}

#[allow(clippy::too_many_arguments)]
fn start_session(
    ctx: &Context,
    api: &str,
    headers: &[(String, String)],
    agent_id: &str,
    role: &str,
    model: &str,
    provider: &str,
    tools: &str,
    max_turns: &str,
    user_message: &str,
    workspace_id: &str,
) -> Result<String, String> {
    let session_resp = ctx.http_call(
        "POST",
        &format!("{api}/tdata/Sessions"),
        headers,
        &json!({ "agent_id": agent_id }).to_string(),
    )?;
    if session_resp.status < 200 || session_resp.status >= 300 {
        return Err(format!(
            "create Session for agent {agent_id} failed (HTTP {})",
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
    let message = user_message.replace("{AGENT_ID}", agent_id);
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
            "Configure for agent {agent_id} failed (HTTP {}): {}",
            configure_resp.status,
            &configure_resp.body[..configure_resp.body.len().min(200)]
        ));
    }
    ctx.log(
        "info",
        &format!("spawn_repairers: spawned {role} agent {agent_id} session {session_id}"),
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

        let endpoint_id = ctx.entity_id.clone();
        let world_id = get("world_id");
        if world_id.trim().is_empty() {
            return Err("Endpoint.world_id is required".to_string());
        }
        let bundle_file_id = get("bundle_file_id");
        let summary = get("summary");
        let author_agent_id = get("author_agent_id");

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
            ("x-temper-principal-id".to_string(), endpoint_id.clone()),
            ("x-temper-agent-type".to_string(), "system".to_string()),
        ];

        // The world owns the model/provider/hindcast configuration.
        let world_resp = ctx.http_call(
            "GET",
            &format!("{api}/tdata/Worlds('{world_id}')"),
            &headers,
            "",
        )?;
        if world_resp.status < 200 || world_resp.status >= 300 {
            return Err(format!(
                "fetch World {world_id} failed (HTTP {})",
                world_resp.status
            ));
        }
        let world: Value = serde_json::from_str(&world_resp.body).unwrap_or(json!({}));
        let model = entity_field(&world, "agent_model", "AgentModel");
        let provider = entity_field(&world, "agent_provider", "AgentProvider");
        if model.trim().is_empty() || provider.trim().is_empty() {
            return Err("World.agent_model and World.agent_provider are required".to_string());
        }
        let hindcast = entity_field(&world, "hindcast_mode", "HindcastMode") == "true";

        // One workspace per world: the repairer writes its repair log there.
        // Without it, temper.write fails Cedar — hard error.
        let workspace_id = ensure_world_workspace(&ctx, &api, &headers, &world_id)?;

        // 1. The Path exists before its repairer does: the repairer
        // self-reports RepairComplete against a real entity id.
        let path_body = json!({
            "world_id": world_id,
            "endpoint_id": endpoint_id,
            "repairer_agent_id": "",
        });
        let path_resp = ctx.http_call(
            "POST",
            &format!("{api}/tdata/Paths"),
            &headers,
            &path_body.to_string(),
        )?;
        if path_resp.status < 200 || path_resp.status >= 300 {
            return Err(format!("create Path failed (HTTP {})", path_resp.status));
        }
        let path_id = serde_json::from_str::<Value>(&path_resp.body)
            .ok()
            .and_then(|v| {
                v.get("entity_id")
                    .and_then(|x| x.as_str())
                    .map(str::to_string)
            })
            .ok_or("Path create returned no entity_id")?;

        // 2. Create the repairer agent. It is freshly created here, so by
        // construction it can never equal the endpoint's author_agent_id —
        // this is the ADR repairer != author mechanism. The PATCH below only
        // binds the assigned repairer so Cedar can check it on RepairComplete.
        let repairer_agent_id = create_agent(
            &ctx,
            &api,
            &headers,
            &format!("Repairer-{path_id}"),
            "repairer",
        )?;
        ctx.log(
            "info",
            &format!(
                "spawn_repairers: repairer {repairer_agent_id} != endpoint author \
                 {author_agent_id} by construction"
            ),
        );
        let patch_body = json!({ "repairer_agent_id": repairer_agent_id });
        match ctx.http_call(
            "PATCH",
            &format!("{api}/tdata/Paths('{path_id}')"),
            &headers,
            &patch_body.to_string(),
        ) {
            Ok(r) if r.status < 400 => {}
            Ok(r) => ctx.log(
                "warn",
                &format!(
                    "spawn_repairers: PATCH Paths('{path_id}') repairer_agent_id failed \
                     (HTTP {}); Cedar's assigned-repairer check won't bind",
                    r.status
                ),
            ),
            Err(e) => ctx.log(
                "warn",
                &format!(
                    "spawn_repairers: PATCH Paths('{path_id}') repairer_agent_id failed ({e}); \
                     Cedar's assigned-repairer check won't bind"
                ),
            ),
        }

        // 3. Spawn the repairer session.
        let repairer_msg = repairer_prompt(
            &path_id,
            &endpoint_id,
            &world_id,
            "{AGENT_ID}",
            &bundle_file_id,
            &summary,
            hindcast,
        );
        start_session(
            &ctx,
            &api,
            &headers,
            &repairer_agent_id,
            "repairer",
            &model,
            &provider,
            &tools_enabled(hindcast),
            "50",
            &repairer_msg,
            &workspace_id,
        )?;

        // 4. Record the attached path on the endpoint.
        set_success_result(
            "PathsAttached",
            &json!({ "path_ids": format!("[\"{path_id}\"]") }),
        );
        ctx.log(
            "info",
            &format!(
                "spawn_repairers: path {path_id} attached to endpoint {endpoint_id} \
                 (repairer will report RepairComplete)"
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

    // Prompt-contract tests: the generated prompts must reference the exact
    // entity sets, action names, and parameter names the specs declare.
    // The API silently drops unknown fields — drift here is a silent failure.

    fn prompt(hindcast: bool) -> String {
        repairer_prompt("p-1", "e-1", "w-1", "a-1", "file-9", "a one-liner", hindcast)
    }

    #[test]
    fn repairer_prompt_carries_the_repair_complete_contract() {
        let p = prompt(false);
        for needle in [
            "temper.action(\"Paths\", \"p-1\", \"RepairComplete\"",
            "\"repair_log_file_id\"",
            "\"required_node_ids\"",
            "\"cost_flags\"",
        ] {
            assert!(p.contains(needle), "repairer prompt missing: {needle}");
        }
    }

    #[test]
    fn repairer_works_backward_and_flags_all_four_kinds_without_scoring() {
        let p = prompt(false);
        for needle in [
            "Work BACKWARD",
            "\"contradiction\"",
            "\"incentive\"",
            "\"lag\"",
            "\"miracle\"",
            "NEVER compute scores",
        ] {
            assert!(p.contains(needle), "repairer prompt missing: {needle}");
        }
    }

    #[test]
    fn repairer_proposes_authored_event_nodes() {
        let p = prompt(false);
        for needle in [
            "temper.create(\"EventNodes\"",
            "\"world_id\": \"w-1\"",
            "\"provenance\": \"authored\"",
            "\"author_agent_id\": \"a-1\"",
            "temper.read(\"file-9\")",
            "temper.list(\"EventNodes\", \"world_id eq 'w-1'\")",
        ] {
            assert!(p.contains(needle), "repairer prompt missing: {needle}");
        }
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
        let p = prompt(true);
        assert!(p.contains("NO web access"));
        assert!(p.contains("never reference anything dated after the world's vantage"));
        assert!(!p.contains("temper.web_search /"));
        let open = prompt(false);
        assert!(open.contains("temper.web_search"));
    }

    #[test]
    fn entity_field_reads_snake_case_fields_and_pascal_case_top_level() {
        let snake = json!({ "fields": { "agent_model": "m1" } });
        assert_eq!(entity_field(&snake, "agent_model", "AgentModel"), "m1");

        let pascal = json!({ "AgentModel": "m2" });
        assert_eq!(entity_field(&pascal, "agent_model", "AgentModel"), "m2");

        // fields wins when both shapes are present and non-empty.
        let both = json!({ "AgentModel": "m2", "fields": { "agent_model": "m1" } });
        assert_eq!(entity_field(&both, "agent_model", "AgentModel"), "m1");

        // An empty snake_case value falls through to PascalCase.
        let empty_snake = json!({ "AgentModel": "m2", "fields": { "agent_model": "" } });
        assert_eq!(entity_field(&empty_snake, "agent_model", "AgentModel"), "m2");

        let neither = json!({});
        assert_eq!(entity_field(&neither, "agent_model", "AgentModel"), "");
    }
}
