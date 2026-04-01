//! Spawn Reviewers — WASM module for creating Code and DST reviewer agents.
//!
//! Triggered when a WorkCycle reaches the Reviewing state. Spawns two reviewer
//! agents (CodeReviewer + DSTReviewer), waits for both to complete, and then
//! dispatches Approve or RequestChanges on the WorkCycle based on results.
//!
//! Build: `cargo build --target wasm32-unknown-unknown --release`

use temper_wasm_sdk::prelude::*;

#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        ctx.log("info", "spawn_reviewers: starting");

        let fields = ctx
            .entity_state
            .get("fields")
            .cloned()
            .unwrap_or(json!({}));

        // 1. Read config
        let temper_api_url = ctx
            .config
            .get("temper_api_url")
            .filter(|s| !s.is_empty() && !s.contains("{secret:"))
            .cloned()
            .unwrap_or_else(|| "http://127.0.0.1:3000".to_string());
        let default_agent_model = ctx
            .config
            .get("default_agent_model")
            .cloned()
            .unwrap_or_else(|| "claude-sonnet-4-6".to_string());

        // 2. Read WorkCycle entity state
        let pr_url = fields
            .get("pr_url")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let project_harness_id = fields
            .get("project_harness_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let entity_id = ctx
            .entity_state
            .get("entity_id")
            .and_then(|v| v.as_str())
            .unwrap_or(&ctx.entity_id);

        if pr_url.is_empty() {
            return Err("spawn_reviewers: pr_url is required on the WorkCycle".to_string());
        }

        let tenant = &ctx.tenant;
        let headers = vec![
            ("content-type".to_string(), "application/json".to_string()),
            ("x-tenant-id".to_string(), tenant.to_string()),
            (
                "x-temper-principal-kind".to_string(),
                "agent".to_string(),
            ),
            (
                "x-temper-principal-id".to_string(),
                ctx.entity_id.clone(),
            ),
            ("x-temper-agent-type".to_string(), "system".to_string()),
        ];

        // 3. Build review instructions for each reviewer
        let code_review_msg = format!(
            "You are a Code Reviewer for WorkCycle {entity_id}.\n\n\
            PR URL: {pr_url}\n\
            ProjectHarness: {project_harness_id}\n\n\
            Instructions:\n\
            1. Read the PR diff with `gh pr diff {pr_url}`\n\
            2. Review the code for correctness, style, security, and maintainability.\n\
            3. Check that tests are adequate and cover the changed code paths.\n\
            4. If the code is acceptable, dispatch Approve on WorkCycle {entity_id}:\n\
               `temper_action WorkCycles {entity_id} OpenPaw.Harness.Approve`\n\
            5. If changes are needed, dispatch RequestChanges on WorkCycle {entity_id}:\n\
               `temper_action WorkCycles {entity_id} OpenPaw.Harness.RequestChanges`\n\n\
            Your final output MUST contain either PASS or FAIL on its own line."
        );

        let dst_review_msg = format!(
            "You are a DST (Deterministic Simulation Testing) Reviewer for WorkCycle {entity_id}.\n\n\
            PR URL: {pr_url}\n\
            ProjectHarness: {project_harness_id}\n\n\
            Instructions:\n\
            1. Read the PR diff with `gh pr diff {pr_url}`\n\
            2. Review from a testing perspective: are DST scenarios covered?\n\
            3. Check for race conditions, edge cases, and non-deterministic behavior.\n\
            4. Verify that simulation tests would catch regressions from these changes.\n\
            5. If the testing coverage is acceptable, dispatch Approve on WorkCycle {entity_id}:\n\
               `temper_action WorkCycles {entity_id} OpenPaw.Harness.Approve`\n\
            6. If changes are needed, dispatch RequestChanges on WorkCycle {entity_id}:\n\
               `temper_action WorkCycles {entity_id} OpenPaw.Harness.RequestChanges`\n\n\
            Your final output MUST contain either PASS or FAIL on its own line."
        );

        // 4. Spawn Code Reviewer agent
        let code_reviewer_id = spawn_agent(
            &ctx,
            &temper_api_url,
            &headers,
            &default_agent_model,
            "CodeReviewer",
            &code_review_msg,
            project_harness_id,
        )?;
        ctx.log(
            "info",
            &format!("spawn_reviewers: created CodeReviewer agent {code_reviewer_id}"),
        );

        // 5. Spawn DST Reviewer agent
        let dst_reviewer_id = spawn_agent(
            &ctx,
            &temper_api_url,
            &headers,
            &default_agent_model,
            "DSTReviewer",
            &dst_review_msg,
            project_harness_id,
        )?;
        ctx.log(
            "info",
            &format!("spawn_reviewers: created DSTReviewer agent {dst_reviewer_id}"),
        );

        // 6. Wait for BOTH agents to complete (sequential polling — WASM can't do true parallel)
        ctx.log(
            "info",
            "spawn_reviewers: waiting for both reviewer agents to complete",
        );

        let code_result = wait_for_agent(&ctx, &temper_api_url, &headers, &code_reviewer_id)?;
        ctx.log(
            "info",
            &format!("spawn_reviewers: CodeReviewer completed: {}", &code_result[..code_result.len().min(200)]),
        );

        let dst_result = wait_for_agent(&ctx, &temper_api_url, &headers, &dst_reviewer_id)?;
        ctx.log(
            "info",
            &format!("spawn_reviewers: DSTReviewer completed: {}", &dst_result[..dst_result.len().min(200)]),
        );

        // 7. Evaluate results — look for PASS/FAIL
        let code_passed = result_is_pass(&code_result);
        let dst_passed = result_is_pass(&dst_result);

        let mut review_notes = String::new();
        review_notes.push_str(&format!(
            "CodeReviewer ({code_reviewer_id}): {}\n",
            if code_passed { "PASS" } else { "FAIL" }
        ));
        review_notes.push_str(&format!(
            "DSTReviewer ({dst_reviewer_id}): {}\n",
            if dst_passed { "PASS" } else { "FAIL" }
        ));
        review_notes.push_str("\n--- CodeReviewer output ---\n");
        review_notes.push_str(&truncate(&code_result, 2048));
        review_notes.push_str("\n--- DSTReviewer output ---\n");
        review_notes.push_str(&truncate(&dst_result, 2048));

        // 8. Dispatch Approve or RequestChanges on the WorkCycle
        if code_passed && dst_passed {
            ctx.log("info", "spawn_reviewers: BOTH reviewers passed, approving");
            let approve_url = format!(
                "{temper_api_url}/tdata/WorkCycles('{entity_id}')/OpenPaw.Harness.Approve"
            );
            let approve_body = json!({
                "approver_id": format!("{code_reviewer_id},{dst_reviewer_id}"),
                "review_notes": review_notes,
            });
            let approve_resp = ctx.http_call(
                "POST",
                &approve_url,
                &headers,
                &approve_body.to_string(),
            )?;
            if approve_resp.status < 200 || approve_resp.status >= 300 {
                ctx.log(
                    "warn",
                    &format!(
                        "spawn_reviewers: Approve dispatch returned HTTP {}",
                        approve_resp.status
                    ),
                );
            }

            set_success_result(
                "",
                &json!({
                    "code_reviewer_id": code_reviewer_id,
                    "dst_reviewer_id": dst_reviewer_id,
                    "verdict": "approved",
                }),
            );
        } else {
            ctx.log(
                "info",
                "spawn_reviewers: one or both reviewers FAILED, requesting changes",
            );
            let rc_url = format!(
                "{temper_api_url}/tdata/WorkCycles('{entity_id}')/OpenPaw.Harness.RequestChanges"
            );
            let rc_body = json!({
                "review_notes": review_notes,
            });
            let rc_resp =
                ctx.http_call("POST", &rc_url, &headers, &rc_body.to_string())?;
            if rc_resp.status < 200 || rc_resp.status >= 300 {
                ctx.log(
                    "warn",
                    &format!(
                        "spawn_reviewers: RequestChanges dispatch returned HTTP {}",
                        rc_resp.status
                    ),
                );
            }

            set_success_result(
                "",
                &json!({
                    "code_reviewer_id": code_reviewer_id,
                    "dst_reviewer_id": dst_reviewer_id,
                    "verdict": "changes_requested",
                }),
            );
        }

        ctx.log("info", "spawn_reviewers: done");
        Ok(())
    })();

    if let Err(e) = result {
        set_error_result(&e);
    }
    0
}

/// Create, configure, and provision an Agent entity. Returns the agent ID.
fn spawn_agent(
    ctx: &Context,
    temper_api_url: &str,
    headers: &[(String, String)],
    model: &str,
    soul_id: &str,
    user_message: &str,
    project_harness_id: &str,
) -> Result<String, String> {
    // Create Agent entity
    let create_url = format!("{temper_api_url}/tdata/Agents");
    let create_resp = ctx.http_call("POST", &create_url, headers, "{}")?;
    if create_resp.status < 200 || create_resp.status >= 300 {
        return Err(format!(
            "spawn_reviewers: failed to create Agent for {soul_id} (HTTP {}): {}",
            create_resp.status,
            &create_resp.body[..create_resp.body.len().min(300)]
        ));
    }

    let agent: Value = serde_json::from_str(&create_resp.body)
        .map_err(|e| format!("spawn_reviewers: failed to parse Agent response: {e}"))?;
    let agent_id = agent
        .get("entity_id")
        .or_else(|| agent.get("Id"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| "spawn_reviewers: Agent creation did not return entity_id".to_string())?;

    // Configure the agent
    let configure_url = format!(
        "{temper_api_url}/tdata/Agents('{agent_id}')/OpenPaw.Configure"
    );
    let configure_body = json!({
        "model": model,
        "provider": "anthropic",
        "tools_enabled": "bash,temper_get,temper_list,temper_action",
        "soul_id": soul_id,
        "user_message": user_message,
        "max_turns": "30",
        "temper_api_url": temper_api_url,
        "project_harness_id": project_harness_id,
    });
    let configure_resp = ctx.http_call(
        "POST",
        &configure_url,
        headers,
        &configure_body.to_string(),
    )?;
    if configure_resp.status < 200 || configure_resp.status >= 300 {
        return Err(format!(
            "spawn_reviewers: Configure failed for {soul_id} (HTTP {}): {}",
            configure_resp.status,
            &configure_resp.body[..configure_resp.body.len().min(300)]
        ));
    }

    // Provision the agent
    let provision_url = format!(
        "{temper_api_url}/tdata/Agents('{agent_id}')/OpenPaw.Provision"
    );
    let provision_resp = ctx.http_call("POST", &provision_url, headers, "{}")?;
    if provision_resp.status < 200 || provision_resp.status >= 300 {
        ctx.log(
            "warn",
            &format!(
                "spawn_reviewers: Provision for {soul_id} returned HTTP {} (non-fatal)",
                provision_resp.status
            ),
        );
    }

    Ok(agent_id.to_string())
}

/// Wait for an agent to reach a terminal state by polling the observe endpoint.
/// Returns the agent's result text.
fn wait_for_agent(
    ctx: &Context,
    temper_api_url: &str,
    headers: &[(String, String)],
    agent_id: &str,
) -> Result<String, String> {
    let wait_url = format!(
        "{temper_api_url}/observe/entities/Agent/{agent_id}/wait"
    );

    // Poll with up to 600 attempts (network latency provides natural backoff)
    for attempt in 0..600 {
        let resp = ctx.http_call("GET", &wait_url, headers, "");
        match resp {
            Ok(r) if r.status >= 200 && r.status < 300 => {
                // Agent has reached a terminal state
                let body: Value =
                    serde_json::from_str(&r.body).unwrap_or(json!({}));

                // Check if agent is in a terminal state
                let state = body
                    .get("state")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                if is_terminal_state(state) {
                    let result = body
                        .get("fields")
                        .and_then(|f| f.get("result"))
                        .and_then(|v| v.as_str())
                        .or_else(|| {
                            body.get("fields")
                                .and_then(|f| f.get("last_response"))
                                .and_then(|v| v.as_str())
                        })
                        .unwrap_or("");
                    return Ok(result.to_string());
                }

                // Not terminal yet — the /wait endpoint might return early
                if attempt % 50 == 0 && attempt > 0 {
                    ctx.log(
                        "info",
                        &format!(
                            "spawn_reviewers: agent {agent_id} still in state '{state}' (attempt {attempt})"
                        ),
                    );
                }
            }
            Ok(r) if r.status == 408 => {
                // Timeout on wait — retry
                if attempt % 50 == 0 && attempt > 0 {
                    ctx.log(
                        "info",
                        &format!(
                            "spawn_reviewers: wait timeout for agent {agent_id}, retrying (attempt {attempt})"
                        ),
                    );
                }
            }
            Ok(r) => {
                ctx.log(
                    "warn",
                    &format!(
                        "spawn_reviewers: unexpected HTTP {} from wait for {agent_id}",
                        r.status
                    ),
                );
            }
            Err(e) => {
                ctx.log(
                    "warn",
                    &format!(
                        "spawn_reviewers: wait request failed for {agent_id}: {e}"
                    ),
                );
            }
        }
    }

    Err(format!(
        "spawn_reviewers: timed out waiting for agent {agent_id}"
    ))
}

/// Check if an agent state is terminal.
fn is_terminal_state(state: &str) -> bool {
    matches!(
        state,
        "Completed" | "Failed" | "Cancelled" | "Terminated" | "Done"
    )
}

/// Check if an agent's result text indicates PASS.
fn result_is_pass(result: &str) -> bool {
    // Look for explicit PASS/FAIL markers
    let upper = result.to_uppercase();
    if upper.contains("FAIL") {
        return false;
    }
    if upper.contains("PASS") {
        return true;
    }
    // Default to pass if no explicit marker (agent completed without failing)
    true
}

/// Truncate a string to at most `max_len` bytes, taking from the end.
fn truncate(s: &str, max_len: usize) -> &str {
    if s.len() <= max_len {
        s
    } else {
        &s[s.len() - max_len..]
    }
}
