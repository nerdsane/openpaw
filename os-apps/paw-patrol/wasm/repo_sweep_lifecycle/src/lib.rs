//! Repo Sweep Lifecycle - queue graph sweeps and fan out findings.
//!
//! Triggered by `RepoGraphSnapshot.StartScan` and `RepoGraphSnapshot.ScanComplete`.
//! StartScan creates a visible WorkCycle and local_codex WorkerRun. ScanComplete
//! turns structured sweep output into QualityFinding and SecurityFinding
//! entities. The codebase health loop remains Temper-native: the worker does
//! the repo graph and dependency sweep, then self-reports through entity
//! actions rather than an external watcher.

use temper_wasm_sdk::prelude::*;

const WORK_CYCLES_PATH: &str = "/tdata/WorkCycles";
const WORKER_RUNS_PATH: &str = "/tdata/WorkerRuns";
const QUALITY_FINDINGS_PATH: &str = "/tdata/QualityFindings";
const SECURITY_FINDINGS_PATH: &str = "/tdata/SecurityFindings";

const PATROL_CONFIGURE: &str = "TemperPaw.Patrol.Configure";
const PATROL_WRITE_PLAN: &str = "TemperPaw.Patrol.WritePlan";
const PATROL_START_WORK: &str = "TemperPaw.Patrol.StartWork";
const PATROL_ATTACH_WORKER_RUN: &str = "TemperPaw.Patrol.AttachWorkerRun";
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
    let worktree_path = format!(
        "/Users/seshendranalla/Development/temperpaw-worktrees/{}",
        branch_name.replace('/', "-")
    );
    let task_summary = format!("repo graph and dependency sweep for {commit_sha}");
    let task_detail = worker_task(&snapshot_id, &work_cycle_id, &commit_sha);
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
        &json!({
            "plan_summary": "Run the recurring repo graph and dependency sweep: build the code/dependency graph, scan giant modules, duplicate logic, TODO/HACK band-aids, Cedar drift, dependency risks, hidden Rust orchestration, polling loops, and missing proof/test coverage. Report structured JSON to RepoGraphSnapshot.ScanComplete, then self-report WorkerRun.ReportDone with the visual evidence packet."
        }),
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
            "allowed_worker_id": &allowed_worker_id
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
    let graph = parse_graph_json(&graph_json)?;

    let quality_count = open_quality_findings(ctx, base_url, headers, &graph)?;
    let security_count = open_security_findings(ctx, base_url, headers, &graph)?;

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
            "security_findings": security_count
        }),
    );
    Ok(())
}

fn open_quality_findings(
    ctx: &Context,
    base_url: &str,
    headers: &[(String, String)],
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
                "fingerprint": string_value(finding, "fingerprint", "")
            }),
        )?;
    }

    Ok(findings.len())
}

fn open_security_findings(
    ctx: &Context,
    base_url: &str,
    headers: &[(String, String)],
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
                "fingerprint": string_value(finding, "fingerprint", "")
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
        "You are the local Codex repo-health worker for TemperPaw paw-patrol.\n\nRepoGraphSnapshot: {snapshot_id}\nWorkCycle: {work_cycle_id}\nCommit: {commit_sha}\n\nRequired loop:\n1. Work in the assigned git worktree and branch.\n2. Build the repo/dependency graph for TemperPaw and the deeply coupled Temper surface.\n3. Scan for giant modules, duplicate logic, TODO/HACK band-aids, Cedar drift, dependency risks, hidden Rust orchestration, polling loops, missing proof coverage, and missing tests.\n4. Produce structured graph_json with quality_findings and security_findings arrays; each finding should include fingerprint, title, severity, evidence, and affected_paths. Security findings also include risk_lane.\n5. Dispatch RepoGraphSnapshot.ScanComplete with graph_json, summary_markdown, generated_at, and finding_count.\n6. Produce a visual ProofPacket with diagrams and links, then self-report WorkerRun.ReportDone or WorkerRun.ReportFailed."
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
