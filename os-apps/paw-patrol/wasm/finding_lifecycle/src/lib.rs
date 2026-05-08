//! Finding Lifecycle - turn accepted findings into cleanup WorkCycles.
//!
//! Triggered by `QualityFinding.Accept`, `SecurityFinding.Accept`, and
//! `ObservabilityFinding.Accept`. A finding is not just a passive note:
//! accepting it creates a paw-pm Issue, creates a cleanup WorkCycle for
//! accepted finding evidence, and queues local Codex work unless the finding is
//! L3 and must pause for human start approval.
//! In short: cleanup WorkCycle for accepted finding.

use temper_wasm_sdk::prelude::*;

const PATROL_PROJECT_ID: &str = "temperpaw-dark-factory";
const ISSUES_PATH: &str = "/tdata/Issues";
const WORK_CYCLES_PATH: &str = "/tdata/WorkCycles";
const WORKER_RUNS_PATH: &str = "/tdata/WorkerRuns";

const PM_SET_DESCRIPTION: &str = "TemperPaw.PM.SetDescription";
const PM_SET_PRIORITY: &str = "TemperPaw.PM.SetPriority";
const PM_MOVE_TO_TRIAGE: &str = "TemperPaw.PM.MoveToTriage";

const PATROL_LINK_PM_ISSUE: &str = "TemperPaw.Patrol.LinkPmIssue";
const PATROL_START_WORK: &str = "TemperPaw.Patrol.StartWork";
const PATROL_CONFIGURE: &str = "TemperPaw.Patrol.Configure";
const PATROL_LINK_SOURCE: &str = "TemperPaw.Patrol.LinkSource";
const PATROL_WRITE_PLAN: &str = "TemperPaw.Patrol.WritePlan";
const PATROL_REQUEST_HUMAN_START_APPROVAL: &str = "TemperPaw.Patrol.RequestHumanStartApproval";
const PATROL_ATTACH_WORKER_RUN: &str = "TemperPaw.Patrol.AttachWorkerRun";

#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        let base_url = resolve_api_url(&ctx);
        let headers = odata_headers(&ctx);
        let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));

        match ctx.trigger_action.as_str() {
            "Accept" => handle_accept(&ctx, &base_url, &headers, &fields),
            other => Err(format!("finding_lifecycle: unsupported trigger {other}")),
        }?;

        set_success_result("", &json!({ "status": "finding_lifecycle_complete" }));
        Ok(())
    })();

    if let Err(error) = result {
        set_error_result(&error);
    }
    0
}

fn handle_accept(
    ctx: &Context,
    base_url: &str,
    headers: &[(String, String)],
    fields: &Value,
) -> Result<(), String> {
    let finding_id = entity_id(ctx);
    let finding_set = finding_entity_set(ctx)?;
    let finding_kind = finding_kind(ctx);
    let title = string_field(fields, "title", "Title");
    let severity = string_field(fields, "severity", "Severity");
    let evidence = evidence_text(fields);
    let affected_paths = string_field(fields, "affected_paths", "AffectedPaths");
    let risk_lane = finding_risk_lane(ctx, fields, &severity);
    let task_summary = format!(
        "{} cleanup: {}",
        finding_kind,
        empty_fallback(&title, "untitled finding")
    );
    let task_detail = worker_task(
        finding_kind,
        &finding_id,
        &task_summary,
        &risk_lane,
        &severity,
        &evidence,
        &affected_paths,
    );
    let branch_name = format!(
        "codex/paw-finding-{}-{}",
        finding_kind.to_ascii_lowercase(),
        short_id(&finding_id)
    );
    let worktree_path = worktree_path(ctx, &branch_name);
    let allowed_worker_id = configured_local_worker_id(ctx);
    let start_approval_required = requires_human_start_approval(&risk_lane);
    let worker_run_id = if start_approval_required {
        "queued after human start approval".to_string()
    } else {
        create_entity(ctx, base_url, headers, WORKER_RUNS_PATH)?
    };
    let issue_id = create_entity(ctx, base_url, headers, ISSUES_PATH)?;
    let work_cycle_id = create_entity(ctx, base_url, headers, WORK_CYCLES_PATH)?;

    post_action(
        ctx,
        base_url,
        headers,
        "Issues",
        &issue_id,
        PM_SET_DESCRIPTION,
        &json!({
            "description": issue_description(
                finding_kind,
                &finding_id,
                &work_cycle_id,
                &worker_run_id,
                &risk_lane,
                &severity,
                &evidence,
                &affected_paths
            )
        }),
    )?;
    post_action(
        ctx,
        base_url,
        headers,
        "Issues",
        &issue_id,
        PM_SET_PRIORITY,
        &json!({ "level": priority_for_lane(&risk_lane) }),
    )?;
    post_action(
        ctx,
        base_url,
        headers,
        "Issues",
        &issue_id,
        PM_MOVE_TO_TRIAGE,
        &json!({ "project_id": PATROL_PROJECT_ID }),
    )?;

    post_action(
        ctx,
        base_url,
        headers,
        "WorkCycles",
        &work_cycle_id,
        PATROL_CONFIGURE,
        &json!({
            "factory_case_id": "",
            "pm_issue_id": &issue_id,
            "task_summary": &task_summary,
            "task_detail": &task_detail,
            "risk_lane": &risk_lane
        }),
    )?;
    post_action(
        ctx,
        base_url,
        headers,
        "WorkCycles",
        &work_cycle_id,
        PATROL_LINK_SOURCE,
        &json!({
            "source_entity_type": finding_kind,
            "source_entity_id": &finding_id
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
            "plan_summary": "Fix or intentionally ratchet the accepted finding in a worktree with red-green TDD, focused tests, live/E2E checks when relevant, and a visual ProofPacket before resolution."
        }),
    )?;

    if requires_human_start_approval(&risk_lane) {
        post_action(
            ctx,
            base_url,
            headers,
            "WorkCycles",
            &work_cycle_id,
            PATROL_REQUEST_HUMAN_START_APPROVAL,
            &json!({
                "approval_summary": format!(
                    "{} {} is L3 and requires human start approval before cleanup work is queued.",
                    finding_kind, finding_id
                )
            }),
        )?;
    } else {
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
                "risk_lane": &risk_lane,
                "task": &task_detail,
                "branch_name": &branch_name,
                "worktree_path": &worktree_path,
                "runner_kind": "local_codex",
                "allowed_worker_id": &allowed_worker_id,
                "provider_id": "local-codex",
                "required_capabilities": finding_required_capabilities(finding_kind)
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
    }

    post_action(
        ctx,
        base_url,
        headers,
        finding_set,
        &finding_id,
        PATROL_LINK_PM_ISSUE,
        &json!({ "pm_issue_id": &issue_id }),
    )?;
    post_action(
        ctx,
        base_url,
        headers,
        finding_set,
        &finding_id,
        PATROL_START_WORK,
        &json!({ "work_cycle_id": &work_cycle_id }),
    )?;

    ctx.log(
        "info",
        &format!(
            "finding_lifecycle: accepted {finding_kind} {finding_id} into WorkCycle {work_cycle_id} and WorkerRun {worker_run_id}"
        ),
    );
    Ok(())
}

fn finding_entity_set(ctx: &Context) -> Result<&'static str, String> {
    if is_entity_type(ctx, "QualityFinding") {
        Ok("QualityFindings")
    } else if is_entity_type(ctx, "SecurityFinding") {
        Ok("SecurityFindings")
    } else if is_entity_type(ctx, "ObservabilityFinding") {
        Ok("ObservabilityFindings")
    } else {
        Err(format!(
            "finding_lifecycle: unsupported entity type {}",
            ctx.entity_type
        ))
    }
}

fn finding_kind(ctx: &Context) -> &'static str {
    if is_entity_type(ctx, "SecurityFinding") {
        "SecurityFinding"
    } else if is_entity_type(ctx, "ObservabilityFinding") {
        "ObservabilityFinding"
    } else {
        "QualityFinding"
    }
}

fn finding_risk_lane(ctx: &Context, fields: &Value, severity: &str) -> String {
    if is_entity_type(ctx, "SecurityFinding") || is_entity_type(ctx, "ObservabilityFinding") {
        let risk_lane = string_field(fields, "risk_lane", "RiskLane");
        return empty_fallback(&risk_lane, "L2").to_string();
    }
    match severity.trim().to_ascii_lowercase().as_str() {
        "critical" | "high" => "L2".to_string(),
        _ => "L1".to_string(),
    }
}

fn finding_required_capabilities(finding_kind: &str) -> &'static str {
    if finding_kind == "ObservabilityFinding" {
        "local_codex,repo_write,datadog_query"
    } else {
        "local_codex,repo_write"
    }
}

fn evidence_text(fields: &Value) -> String {
    let evidence = string_field(fields, "evidence", "Evidence");
    if !evidence.trim().is_empty() {
        return evidence;
    }
    string_field(fields, "evidence_json", "EvidenceJson")
}

fn requires_human_start_approval(lane: &str) -> bool {
    lane.eq_ignore_ascii_case("L3")
}

fn priority_for_lane(lane: &str) -> &str {
    match lane {
        "L3" => "1",
        "L2" => "2",
        "L1" => "3",
        _ => "4",
    }
}

fn issue_description(
    finding_kind: &str,
    finding_id: &str,
    work_cycle_id: &str,
    worker_run_id: &str,
    risk_lane: &str,
    severity: &str,
    evidence: &str,
    affected_paths: &str,
) -> String {
    format!(
        "Patrol accepted {finding_kind} {finding_id} as actionable cleanup.\n\nRisk lane: {risk_lane}\nSeverity: {severity}\nWorkCycle: {work_cycle_id}\nWorkerRun: {worker_run_id}\nAffected paths: {affected_paths}\n\nEvidence:\n{evidence}\n"
    )
}

fn worker_task(
    finding_kind: &str,
    finding_id: &str,
    task_summary: &str,
    risk_lane: &str,
    severity: &str,
    evidence: &str,
    affected_paths: &str,
) -> String {
    format!(
        "You are the local Codex implementer for an accepted Patrol finding.\n\nFinding type: {finding_kind}\nFinding: {finding_id}\nRisk lane: {risk_lane}\nSeverity: {severity}\nSummary: {task_summary}\nAffected paths: {affected_paths}\n\nEvidence:\n{evidence}\n\nRequired loop:\n1. Work in the assigned git worktree and branch.\n2. Follow red-green TDD before implementation.\n3. Keep orchestration Temper-native: entity specs, WASM integrations, and Cedar policies.\n4. Fix the finding or codify a ratchet that prevents recurrence in the touched area.\n5. Run focused tests and relevant live/E2E verification.\n6. Produce a visual ProofPacket and finish normally. The paw-codex-worker will report WorkerRun.ReportDone or WorkerRun.ReportFailed to Temper after the local Codex process exits."
    )
}

fn entity_id(ctx: &Context) -> String {
    ctx.entity_state
        .get("entity_id")
        .and_then(Value::as_str)
        .unwrap_or(&ctx.entity_id)
        .to_string()
}

fn is_entity_type(ctx: &Context, expected: &str) -> bool {
    ctx.entity_type.eq_ignore_ascii_case(expected)
        || ctx
            .entity_type
            .eq_ignore_ascii_case(&format!("{expected}s"))
}

fn string_field(fields: &Value, snake: &str, pascal: &str) -> String {
    fields
        .get(snake)
        .and_then(Value::as_str)
        .or_else(|| fields.get(pascal).and_then(Value::as_str))
        .unwrap_or("")
        .to_string()
}

fn empty_fallback<'a>(value: &'a str, fallback: &'a str) -> &'a str {
    if value.trim().is_empty() {
        fallback
    } else {
        value
    }
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
