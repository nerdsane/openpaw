//! Repo Sweep Lifecycle - queue graph sweeps, fan out findings, and assess them.
//!
//! Triggered by `RepoGraphSnapshot.StartScan` and `RepoGraphSnapshot.ScanComplete`.
//! StartScan creates a visible WorkCycle and local_codex WorkerRun. ScanComplete
//! turns agent-authored sweep output into QualityFinding and SecurityFinding
//! entities. The codebase health loop remains Temper-native: Codex performs the
//! repo graph and dependency investigation, then the worker self-reports through
//! entity actions rather than an external watcher.

use temper_wasm_sdk::prelude::*;

const WORK_CYCLES_PATH: &str = "/tdata/WorkCycles";
const WORKER_RUNS_PATH: &str = "/tdata/WorkerRuns";
const SESSIONS_PATH: &str = "/tdata/Sessions";
const QUALITY_FINDINGS_PATH: &str = "/tdata/QualityFindings";
const SECURITY_FINDINGS_PATH: &str = "/tdata/SecurityFindings";

const SESSION_CONFIGURE: &str = "TemperPaw.Configure";
const PATROL_CONFIGURE: &str = "TemperPaw.Patrol.Configure";
const PATROL_WRITE_PLAN: &str = "TemperPaw.Patrol.WritePlan";
const PATROL_START_WORK: &str = "TemperPaw.Patrol.StartWork";
const PATROL_ATTACH_WORKER_RUN: &str = "TemperPaw.Patrol.AttachWorkerRun";
const PATROL_ATTACH_ASSESSMENT_SESSION: &str = "TemperPaw.Patrol.AttachAssessmentSession";
const PATROL_ASSESSMENT_COMPLETE: &str = "TemperPaw.Patrol.AssessmentComplete";
const PATROL_OPEN_FINDING: &str = "TemperPaw.Patrol.OpenFinding";

#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        let base_url = resolve_api_url(&ctx);
        let headers = odata_headers(&ctx);
        let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));

        match ctx.trigger_action.as_str() {
            "StartScan" => handle_start_scan(&ctx, &base_url, &headers, &fields),
            "ScanComplete" => handle_scan_complete(&ctx, &base_url, &headers, &fields),
            other => Err(format!("repo_sweep_lifecycle: unsupported trigger {other}")),
        }?;

        set_success_result("", &json!({ "status": "repo_sweep_lifecycle_complete" }));
        Ok(())
    })();

    if let Err(error) = result {
        set_error_result(&error);
    }
    0
}

fn handle_start_scan(
    ctx: &Context,
    base_url: &str,
    headers: &[(String, String)],
    fields: &Value,
) -> Result<(), String> {
    let snapshot_id = entity_id(ctx);
    let commit_sha = string_param(ctx, fields, "commit_sha", "CommitSha");
    let commit_sha = if commit_sha.trim().is_empty() {
        "current-checkout".to_string()
    } else {
        commit_sha
    };
    let work_cycle_id = create_entity(ctx, base_url, headers, WORK_CYCLES_PATH)?;
    let worker_run_id = create_entity(ctx, base_url, headers, WORKER_RUNS_PATH)?;
    let branch_name = format!("codex/paw-repo-sweep-{}", short_id(&snapshot_id));
    let worktree_path = worktree_path(ctx, &branch_name);
    let task_summary = format!("repo graph and dependency sweep for {commit_sha}");
    let task_detail = worker_task(&snapshot_id, &work_cycle_id, &commit_sha);
    let plan_summary = repo_sweep_plan(&snapshot_id, &commit_sha);
    let allowed_worker_id = configured_local_worker_id(ctx);

    post_action(
        ctx,
        base_url,
        headers,
        "WorkCycles",
        &work_cycle_id,
        PATROL_CONFIGURE,
        &json!({
            "factory_case_id": "",
            "pm_issue_id": "",
            "task_summary": &task_summary,
            "task_detail": &task_detail,
            "risk_lane": "L1"
        }),
    )?;
    post_action(
        ctx,
        base_url,
        headers,
        "WorkCycles",
        &work_cycle_id,
        PATROL_WRITE_PLAN,
        &json!({ "plan_summary": &plan_summary }),
    )?;
    post_action(
        ctx,
        base_url,
        headers,
        "WorkCycles",
        &work_cycle_id,
        PATROL_START_WORK,
        &json!({}),
    )?;
    post_action(
        ctx,
        base_url,
        headers,
        "WorkerRuns",
        &worker_run_id,
        PATROL_CONFIGURE,
        &json!({
            "work_cycle_id": &work_cycle_id,
            "factory_case_id": "",
            "risk_lane": "L1",
            "task": &task_detail,
            "branch_name": &branch_name,
            "worktree_path": &worktree_path,
            "runner_kind": "local_codex",
            "allowed_worker_id": &allowed_worker_id,
            "provider_id": "local-codex",
            "required_capabilities": "local_codex,repo_write,evaluation"
        }),
    )?;
    post_action(
        ctx,
        base_url,
        headers,
        "WorkCycles",
        &work_cycle_id,
        PATROL_ATTACH_WORKER_RUN,
        &json!({ "implementer_worker_run_id": &worker_run_id }),
    )?;
    post_action(
        ctx,
        base_url,
        headers,
        "RepoGraphSnapshots",
        &snapshot_id,
        PATROL_ATTACH_WORKER_RUN,
        &json!({
            "work_cycle_id": &work_cycle_id,
            "worker_run_id": &worker_run_id
        }),
    )?;

    ctx.log(
        "info",
        &format!(
            "repo_sweep_lifecycle: queued repo sweep WorkerRun {worker_run_id} for RepoGraphSnapshot {snapshot_id}"
        ),
    );
    set_success_result("", &json!({ "worker_run_id": &worker_run_id }));
    Ok(())
}

fn handle_scan_complete(
    ctx: &Context,
    base_url: &str,
    headers: &[(String, String)],
    fields: &Value,
) -> Result<(), String> {
    let snapshot_id = entity_id(ctx);
    let graph_json = string_param(ctx, fields, "graph_json", "GraphJson");
    let summary_markdown = string_param(ctx, fields, "summary_markdown", "SummaryMarkdown");
    let generated_at = string_param(ctx, fields, "generated_at", "GeneratedAt");
    let graph = parse_graph_json(&graph_json)?;

    let quality_count = open_quality_findings(ctx, base_url, headers, &snapshot_id, &graph)?;
    let security_count = open_security_findings(ctx, base_url, headers, &snapshot_id, &graph)?;
    let assessment_session_id = if has_real_session_provider(ctx, "repo_assessment_provider") {
        let assessment_session_id = spawn_assessment_session(
            ctx,
            base_url,
            headers,
            AssessmentPrompt {
                snapshot_id: &snapshot_id,
                graph_json: &graph_json,
                summary_markdown: &summary_markdown,
                generated_at: &generated_at,
                quality_count,
                security_count,
            },
        )?;
        post_action(
            ctx,
            base_url,
            headers,
            "RepoGraphSnapshots",
            &snapshot_id,
            PATROL_ATTACH_ASSESSMENT_SESSION,
            &json!({
                "assessment_session_id": &assessment_session_id,
                "assessment_status": "running"
            }),
        )?;
        assessment_session_id
    } else {
        post_action(
            ctx,
            base_url,
            headers,
            "RepoGraphSnapshots",
            &snapshot_id,
            PATROL_ASSESSMENT_COMPLETE,
            &json!({
                "assessment_summary_markdown": summary_markdown,
                "assessment_status": "complete_from_repo_health_agent"
            }),
        )?;
        String::new()
    };

    ctx.log(
        "info",
        &format!(
            "repo_sweep_lifecycle: RepoGraphSnapshot {snapshot_id} opened {quality_count} quality and {security_count} security findings"
        ),
    );
    set_success_result(
        "",
        &json!({
            "quality_findings": quality_count,
            "security_findings": security_count,
            "assessment_session_id": assessment_session_id
        }),
    );
    Ok(())
}

struct AssessmentPrompt<'a> {
    snapshot_id: &'a str,
    graph_json: &'a str,
    summary_markdown: &'a str,
    generated_at: &'a str,
    quality_count: usize,
    security_count: usize,
}

fn spawn_assessment_session(
    ctx: &Context,
    base_url: &str,
    headers: &[(String, String)],
    prompt: AssessmentPrompt<'_>,
) -> Result<String, String> {
    let session_id = create_entity(ctx, base_url, headers, SESSIONS_PATH)?;
    let session_model = configured_session_value(ctx, "repo_assessment_model", "mock");
    let session_provider = configured_session_value(ctx, "repo_assessment_provider", "mock");
    let user_message = repo_assessment_session_prompt(&prompt);

    post_action(
        ctx,
        base_url,
        headers,
        "Sessions",
        &session_id,
        SESSION_CONFIGURE,
        &json!({
            "system_prompt": "You are a Patrol repo-health assessment agent. Review agent-authored graph evidence with security, readability, and architecture judgment, and close the loop by dispatching Temper actions.",
            "user_message": user_message,
            "model": session_model,
            "provider": session_provider,
            "temperature": "0.2",
            "max_turns": "12",
            "tools_enabled": "temper_get,temper_list,temper_action,temper_read",
            "temper_api_url": base_url,
            "soul_id": "SRE",
            "agent_id": "paw-patrol-repo-assessor",
            "session_mode": "patrol_repo_assessment"
        }),
    )?;

    Ok(session_id)
}

fn repo_assessment_session_prompt(input: &AssessmentPrompt<'_>) -> String {
    format!(
        "You are the intelligent assessment Session for a Patrol RepoGraphSnapshot.\n\nRepoGraphSnapshot: {}\nGenerated at: {}\nAgent-authored findings opened by repo_sweep_lifecycle:\n- QualityFinding count: {}\n- SecurityFinding count: {}\n\nSummary markdown from the repo-health worker:\n{}\n\nTruncated repo graph evidence JSON:\n{}\n\nYour job:\n1. Treat graph_json as structured evidence from a Codex repo-health patrol, not as final judgment.\n2. Profoundly scan the evidence with intelligence for giant modules, mixed concerns, duplicate logic, TODO/HACK/band-aids, hidden Rust orchestration, polling loops, Cedar/security drift, dependency risks, and missing proof/test coverage.\n3. Query Temper entities if you need more detail; cite entity IDs and affected paths.\n4. Produce a visual, human-readable assessment with Mermaid diagrams when useful.\n5. If the worker missed something, name it clearly in the assessment summary so Patrol can turn it into findings/work.\n6. Use `temper.action(\"RepoGraphSnapshots\", \"{}\", \"AssessmentComplete\", params)` with `assessment_status = \"complete\"` and `assessment_summary_markdown` containing your prioritized assessment. The OData action is `{}`.\n7. If blocked, dispatch the same action with `assessment_status = \"blocked\"` and explain the blocker.",
        input.snapshot_id,
        empty_fallback(input.generated_at, "unspecified"),
        input.quality_count,
        input.security_count,
        empty_fallback(input.summary_markdown, "no summary provided"),
        truncate(input.graph_json, 6000),
        input.snapshot_id,
        PATROL_ASSESSMENT_COMPLETE
    )
}

fn open_quality_findings(
    ctx: &Context,
    base_url: &str,
    headers: &[(String, String)],
    snapshot_id: &str,
    graph: &Value,
) -> Result<usize, String> {
    let findings = graph
        .get("quality_findings")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);

    for finding in findings {
        let finding_id = create_entity(ctx, base_url, headers, QUALITY_FINDINGS_PATH)?;
        post_action(
            ctx,
            base_url,
            headers,
            "QualityFindings",
            &finding_id,
            PATROL_OPEN_FINDING,
            &json!({
                "title": string_value(finding, "title", "Untitled quality finding"),
                "severity": string_value(finding, "severity", "medium"),
                "evidence": string_value(finding, "evidence", ""),
                "affected_paths": paths_value(finding),
                "fingerprint": string_value(finding, "fingerprint", ""),
                "repo_graph_snapshot_id": snapshot_id
            }),
        )?;
    }

    Ok(findings.len())
}

fn open_security_findings(
    ctx: &Context,
    base_url: &str,
    headers: &[(String, String)],
    snapshot_id: &str,
    graph: &Value,
) -> Result<usize, String> {
    let findings = graph
        .get("security_findings")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[]);

    for finding in findings {
        let finding_id = create_entity(ctx, base_url, headers, SECURITY_FINDINGS_PATH)?;
        post_action(
            ctx,
            base_url,
            headers,
            "SecurityFindings",
            &finding_id,
            PATROL_OPEN_FINDING,
            &json!({
                "title": string_value(finding, "title", "Untitled security finding"),
                "severity": string_value(finding, "severity", "medium"),
                "risk_lane": string_value(finding, "risk_lane", "L2"),
                "evidence": string_value(finding, "evidence", ""),
                "affected_paths": paths_value(finding),
                "fingerprint": string_value(finding, "fingerprint", ""),
                "repo_graph_snapshot_id": snapshot_id
            }),
        )?;
    }

    Ok(findings.len())
}

fn parse_graph_json(graph_json: &str) -> Result<Value, String> {
    if graph_json.trim().is_empty() {
        return Ok(json!({}));
    }
    serde_json::from_str(graph_json)
        .map_err(|err| format!("repo_sweep_lifecycle: graph_json parse failed: {err}"))
}

fn string_value(value: &Value, key: &str, fallback: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|text| !text.trim().is_empty())
        .unwrap_or(fallback)
        .to_string()
}

fn paths_value(value: &Value) -> String {
    value
        .get("affected_paths")
        .or_else(|| value.get("affectedPaths"))
        .cloned()
        .unwrap_or_else(|| json!([]))
        .to_string()
}

fn worker_task(snapshot_id: &str, work_cycle_id: &str, commit_sha: &str) -> String {
    format!(
        "You are the local Codex repo-health Patrol agent for TemperPaw paw-patrol.\n\nRepoGraphSnapshot: {snapshot_id}\nWorkCycle: {work_cycle_id}\nCommit: {commit_sha}\n\nRequired loop:\n1. Work in the assigned git worktree and branch; do not edit files during this patrol scan.\n2. Build the repo/dependency graph for TemperPaw and the deeply coupled Temper surface with agent judgment.\n3. Investigate giant modules, mixed concerns, duplicate logic, TODO/HACK band-aids, Cedar drift, dependency risks, hidden Rust orchestration, polling loops, missing proof coverage, missing tests, dashboard breakage, and agent/human readability.\n4. Return structured repo-health patrol JSON to paw-codex-worker. The worker validates it and dispatches RepoGraphSnapshot.ScanComplete through Temper.\n5. Produce a visual, human-readable summary with diagrams and links. The paw-codex-worker will report WorkerRun.ReportDone or WorkerRun.ReportFailed to Temper after the local Codex process exits."
    )
}

fn repo_sweep_plan(snapshot_id: &str, commit_sha: &str) -> String {
    format!(
        "# WorkCycle Plan\n\n## Context\nRun the recurring agent-led repo health patrol for TemperPaw.\n\nRepoGraphSnapshot: {snapshot_id}\nCommit: {commit_sha}\nRisk lane: L1\n\n## Codex Plan Mode\nThe repo sweep is itself read-only. Codex must still start from plan-mode discipline: inspect the assigned worktree, enumerate evidence surfaces, and avoid repository mutation while building the graph and findings.\n\n## Approach\n1. Build a factual repo/dependency graph for TemperPaw and the tightly coupled Temper surface.\n2. Inspect giant modules, duplicate logic, TODO/HACK band-aids, Cedar drift, dependency risk, Rust orchestration leaks, polling loops, missing proof/test coverage, dashboard breakage, and agent/human readability.\n3. Return structured repo-health JSON to the worker, not ad hoc prose.\n4. Let Patrol state transitions create QualityFinding/SecurityFinding entities from the structured evidence.\n\n## File Manifest\n- `crates/`, `os-apps/`, `dashboard/`, `scripts/`, `docs/`, and dependency manifests are read-only evidence sources.\n- `RepoGraphSnapshot` receives graph JSON and summary markdown through `ScanComplete`.\n- `QualityFindings` and `SecurityFindings` are created by the WASM lifecycle from scan output.\n\n## Verification Plan\nValidate the repo-health JSON shape, dispatch `RepoGraphSnapshot.ScanComplete`, query the snapshot and created findings, and produce a visual proof summary. No implementation diff should be required for this patrol WorkCycle.\n\n## Risks\n- Heuristic scans can over-report; Codex must use judgment and evidence.\n- Dependency/security signals may need human or specialist review before cleanup work starts.\n- The scan must not expose secrets or mutate source files.\n\n## Open Questions\nCodex Plan Mode must record any unavailable evidence surfaces and residual risk in the scan output."
    )
}

fn entity_id(ctx: &Context) -> String {
    ctx.entity_state
        .get("entity_id")
        .and_then(Value::as_str)
        .unwrap_or(&ctx.entity_id)
        .to_string()
}

fn string_param(ctx: &Context, fields: &Value, snake: &str, pascal: &str) -> String {
    ctx.trigger_params
        .get(snake)
        .and_then(Value::as_str)
        .or_else(|| ctx.trigger_params.get(pascal).and_then(Value::as_str))
        .map(str::to_string)
        .unwrap_or_else(|| string_field(fields, snake, pascal))
}

fn string_field(fields: &Value, snake: &str, pascal: &str) -> String {
    fields
        .get(snake)
        .and_then(Value::as_str)
        .or_else(|| fields.get(pascal).and_then(Value::as_str))
        .unwrap_or("")
        .to_string()
}

fn short_id(entity_id: &str) -> String {
    let tail: String = entity_id
        .chars()
        .rev()
        .take(8)
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    tail.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect()
}

fn resolve_api_url(ctx: &Context) -> String {
    ctx.config
        .get("temper_api_url")
        .filter(|value| !value.is_empty() && !value.contains("{secret:"))
        .cloned()
        .unwrap_or_else(|| "http://127.0.0.1:3000".to_string())
}

fn odata_headers(ctx: &Context) -> Vec<(String, String)> {
    vec![
        ("content-type".to_string(), "application/json".to_string()),
        ("x-tenant-id".to_string(), ctx.tenant.clone()),
        ("x-temper-principal-kind".to_string(), "agent".to_string()),
        ("x-temper-principal-id".to_string(), ctx.entity_id.clone()),
        ("x-temper-agent-type".to_string(), "system".to_string()),
    ]
}

fn configured_local_worker_id(ctx: &Context) -> String {
    ctx.config
        .get("local_codex_worker_id")
        .filter(|value| !value.trim().is_empty() && !value.contains("{secret:"))
        .cloned()
        .unwrap_or_else(|| "mac-mini-codex-prod".to_string())
}

fn configured_local_worktree_root(ctx: &Context) -> String {
    ctx.config
        .get("local_codex_worktree_root")
        .filter(|value| !value.trim().is_empty() && !value.contains("{secret:"))
        .cloned()
        .unwrap_or_else(|| "/Users/openclaw/Development/temperpaw-worktrees".to_string())
}

fn worktree_path(ctx: &Context, branch_name: &str) -> String {
    format!(
        "{}/{}",
        configured_local_worktree_root(ctx).trim_end_matches('/'),
        branch_name.replace('/', "-")
    )
}

fn configured_session_value(ctx: &Context, key: &str, fallback: &str) -> String {
    ctx.config
        .get(key)
        .filter(|value| !value.trim().is_empty() && !value.contains("{secret:"))
        .cloned()
        .unwrap_or_else(|| fallback.to_string())
}

fn has_real_session_provider(ctx: &Context, key: &str) -> bool {
    let provider = configured_session_value(ctx, key, "");
    !provider.trim().is_empty() && provider != "mock"
}

fn empty_fallback<'a>(value: &'a str, fallback: &'a str) -> &'a str {
    if value.trim().is_empty() {
        fallback
    } else {
        value
    }
}

fn create_entity(
    ctx: &Context,
    base_url: &str,
    headers: &[(String, String)],
    path: &str,
) -> Result<String, String> {
    let url = format!("{base_url}{path}");
    let entity_set = path.rsplit('/').next().unwrap_or(path);
    let resp = ctx.http_call("POST", &url, headers, "{}")?;
    let body = parse_json_response(resp, &format!("create {entity_set}"))?;
    entity_id_from_response(&body).ok_or_else(|| format!("create {entity_set}: missing entity_id"))
}

fn post_action(
    ctx: &Context,
    base_url: &str,
    headers: &[(String, String)],
    entity_set: &str,
    entity_id: &str,
    action_path: &str,
    body: &Value,
) -> Result<Value, String> {
    let url = format!("{base_url}/tdata/{entity_set}('{entity_id}')/{action_path}");
    let resp = ctx.http_call("POST", &url, headers, &body.to_string())?;
    parse_json_response(
        resp,
        &format!("{action_path} on {entity_set}('{entity_id}')"),
    )
}

fn parse_json_response(resp: HttpResponse, label: &str) -> Result<Value, String> {
    if resp.status < 200 || resp.status >= 300 {
        return Err(format!(
            "{label} failed with HTTP {}: {}",
            resp.status,
            truncate(&resp.body, 500)
        ));
    }
    if resp.body.trim().is_empty() {
        return Ok(json!({}));
    }
    serde_json::from_str(&resp.body).map_err(|err| format!("{label}: parse response: {err}"))
}

fn entity_id_from_response(value: &Value) -> Option<String> {
    value
        .get("entity_id")
        .or_else(|| value.get("id"))
        .or_else(|| value.get("Id"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn truncate(input: &str, max: usize) -> String {
    if input.len() <= max {
        input.to_string()
    } else {
        format!("{}[truncated]", input.chars().take(max).collect::<String>())
    }
}
