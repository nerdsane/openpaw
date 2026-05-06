//! WorkCycle Lifecycle - dispatch human-approved work and close source findings.
//!
//! Triggered by `WorkCycle.ApproveHumanStart` and
//! `WorkCycle.ApproveHumanCompletion`, plus `WorkCycle.Complete`. L3 work stays
//! visible in WorkCycle approval states until a human or supervisor approves;
//! only then does this integration queue the local Codex WorkerRun or complete
//! the proof-ready cycle. When completed work came from an accepted finding,
//! this module resolves the source QualityFinding or SecurityFinding with proof
//! evidence.

use temper_wasm_sdk::prelude::*;

const FACTORY_CASES_PATH: &str = "/tdata/FactoryCases";
const QUALITY_FINDINGS_PATH: &str = "/tdata/QualityFindings";
const SECURITY_FINDINGS_PATH: &str = "/tdata/SecurityFindings";
const WORKER_RUNS_PATH: &str = "/tdata/WorkerRuns";

const PATROL_CONFIGURE: &str = "TemperPaw.Patrol.Configure";
const PATROL_START_WORK: &str = "TemperPaw.Patrol.StartWork";
const PATROL_ATTACH_WORKER_RUN: &str = "TemperPaw.Patrol.AttachWorkerRun";
const PATROL_QUEUE_WORK: &str = "TemperPaw.Patrol.QueueWork";
const PATROL_COMPLETE: &str = "TemperPaw.Patrol.Complete";
const PATROL_RESOLVE: &str = "TemperPaw.Patrol.Resolve";

#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        let base_url = resolve_api_url(&ctx);
        let headers = odata_headers(&ctx);
        let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));

        match ctx.trigger_action.as_str() {
            "ApproveHumanStart" => handle_human_start_approved(&ctx, &base_url, &headers, &fields),
            "ApproveHumanCompletion" => {
                handle_human_completion_approved(&ctx, &base_url, &headers, &fields)
            }
            "Complete" => handle_complete(&ctx, &base_url, &headers, &fields),
            other => Err(format!("work_cycle_lifecycle: unsupported trigger {other}")),
        }?;

        set_success_result("", &json!({ "status": "work_cycle_lifecycle_complete" }));
        Ok(())
    })();

    if let Err(error) = result {
        set_error_result(&error);
    }
    0
}

fn handle_human_start_approved(
    ctx: &Context,
    base_url: &str,
    headers: &[(String, String)],
    fields: &Value,
) -> Result<(), String> {
    let work_cycle_id = entity_id(ctx);
    let case_id = string_field(fields, "factory_case_id", "FactoryCaseId");
    let risk_lane = string_field(fields, "risk_lane", "RiskLane");
    let risk_lane = if risk_lane.trim().is_empty() {
        "L3".to_string()
    } else {
        risk_lane
    };
    let task_summary = string_field(fields, "task_summary", "TaskSummary");
    let task_detail = string_field(fields, "task_detail", "TaskDetail");
    let approval_summary = string_param(ctx, fields, "approval_summary", "ApprovalSummary");
    let branch_name = format!("codex/paw-approved-{}", short_id(&work_cycle_id));
    let worktree_path = worktree_path(ctx, &branch_name);
    let task = if task_detail.trim().is_empty() {
        fallback_task(&work_cycle_id, &case_id, &task_summary, &approval_summary)
    } else {
        task_detail
    };

    let worker_run_id = create_entity(ctx, base_url, headers, WORKER_RUNS_PATH)?;
    let allowed_worker_id = configured_local_worker_id(ctx);

    post_action(
        ctx,
        base_url,
        headers,
        entity_set(WORKER_RUNS_PATH),
        &worker_run_id,
        PATROL_CONFIGURE,
        &json!({
            "work_cycle_id": &work_cycle_id,
            "factory_case_id": &case_id,
            "risk_lane": &risk_lane,
            "task": &task,
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
        PATROL_START_WORK,
        &json!({}),
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

    if !case_id.is_empty() {
        let case_status = get_status(
            ctx,
            base_url,
            headers,
            entity_set(FACTORY_CASES_PATH),
            &case_id,
        )?;
        if case_status == "Scoped" {
            post_action(
                ctx,
                base_url,
                headers,
                entity_set(FACTORY_CASES_PATH),
                &case_id,
                PATROL_QUEUE_WORK,
                &json!({}),
            )?;
        }
    }

    ctx.log(
        "info",
        &format!(
            "work_cycle_lifecycle: queued human-approved L3 work {work_cycle_id} as WorkerRun {worker_run_id}"
        ),
    );
    Ok(())
}

fn handle_human_completion_approved(
    ctx: &Context,
    base_url: &str,
    headers: &[(String, String)],
    fields: &Value,
) -> Result<(), String> {
    let work_cycle_id = entity_id(ctx);
    let case_id = string_field(fields, "factory_case_id", "FactoryCaseId");
    let proof_packet_id = string_field(fields, "proof_packet_id", "ProofPacketId");

    let review_passed = bool_field(fields, "review_passed", "ReviewPassed");
    let evaluation_passed = bool_field(fields, "evaluation_passed", "EvaluationPassed");
    let proof_attached = bool_field(fields, "proof_attached", "ProofAttached");
    if !(review_passed && evaluation_passed && proof_attached) {
        return Err(format!(
            "work_cycle_lifecycle: WorkCycle {work_cycle_id} cannot complete; review, evaluation, and proof are not all passed"
        ));
    }

    let status = get_status(ctx, base_url, headers, "WorkCycles", &work_cycle_id)?;
    if status == "Proving" {
        post_action(
            ctx,
            base_url,
            headers,
            "WorkCycles",
            &work_cycle_id,
            PATROL_COMPLETE,
            &json!({}),
        )?;
    }

    if !case_id.is_empty() {
        let case_status = get_status(
            ctx,
            base_url,
            headers,
            entity_set(FACTORY_CASES_PATH),
            &case_id,
        )?;
        if matches!(case_status.as_str(), "Reviewing" | "Proving") {
            post_action(
                ctx,
                base_url,
                headers,
                entity_set(FACTORY_CASES_PATH),
                &case_id,
                PATROL_COMPLETE,
                &json!({
                    "summary": format!(
                        "High-risk WorkCycle {work_cycle_id} completed after human completion approval with ProofPacket {proof_packet_id}."
                    )
                }),
            )?;
        }
    }

    Ok(())
}

fn handle_complete(
    ctx: &Context,
    base_url: &str,
    headers: &[(String, String)],
    fields: &Value,
) -> Result<(), String> {
    let work_cycle_id = entity_id(ctx);
    let source_entity_type = string_field(fields, "source_entity_type", "SourceEntityType");
    let source_entity_id = string_field(fields, "source_entity_id", "SourceEntityId");
    if source_entity_type.trim().is_empty() || source_entity_id.trim().is_empty() {
        return Ok(());
    }

    let source_set = match source_entity_type.trim() {
        "QualityFinding" | "QualityFindings" => entity_set(QUALITY_FINDINGS_PATH),
        "SecurityFinding" | "SecurityFindings" => entity_set(SECURITY_FINDINGS_PATH),
        other => {
            ctx.log(
                "warn",
                &format!("work_cycle_lifecycle: unsupported source_entity_type {other}"),
            );
            return Ok(());
        }
    };

    let status = get_status(ctx, base_url, headers, source_set, &source_entity_id)?;
    if !matches!(status.as_str(), "Accepted" | "InProgress") {
        return Ok(());
    }

    let proof_packet_id = string_field(fields, "proof_packet_id", "ProofPacketId");
    let reviewer_run_id = string_field(fields, "reviewer_run_id", "ReviewerRunId");
    let evaluation_run_id = string_field(fields, "evaluation_run_id", "EvaluationRunId");
    let resolution_summary = format!(
        "Resolved by WorkCycle {work_cycle_id}. ProofPacket: {}. ReviewRun: {}. EvaluationRun: {}. Completion required reviewer approval, automated evaluation pass, recorded live/E2E evidence, and attached proof.",
        empty_fallback(&proof_packet_id, "not recorded"),
        empty_fallback(&reviewer_run_id, "not recorded"),
        empty_fallback(&evaluation_run_id, "not recorded")
    );

    post_action(
        ctx,
        base_url,
        headers,
        source_set,
        &source_entity_id,
        PATROL_RESOLVE,
        &json!({ "resolution_summary": resolution_summary }),
    )?;
    ctx.log(
        "info",
        &format!(
            "work_cycle_lifecycle: resolved {source_entity_type} {source_entity_id} from WorkCycle {work_cycle_id}"
        ),
    );
    Ok(())
}

fn fallback_task(
    work_cycle_id: &str,
    case_id: &str,
    task_summary: &str,
    approval_summary: &str,
) -> String {
    format!(
        "You are the local Codex implementer for human-approved L3 work.\n\nFactoryCase: {case_id}\nWorkCycle: {work_cycle_id}\nSummary: {task_summary}\nHuman approval: {approval_summary}\n\nRequired loop:\n1. Work in the assigned git worktree and branch.\n2. Follow red-green TDD before implementation.\n3. Keep orchestration Temper-native: entity specs, WASM integrations, and Cedar policies.\n4. Run focused tests and relevant live/E2E verification.\n5. Produce a visual ProofPacket and self-report WorkerRun.ReportDone or WorkerRun.ReportFailed."
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

fn bool_field(fields: &Value, snake: &str, pascal: &str) -> bool {
    match fields.get(snake).or_else(|| fields.get(pascal)) {
        Some(Value::Bool(flag)) => *flag,
        Some(Value::String(flag)) => flag == "true",
        _ => false,
    }
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

fn entity_set(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

fn get_status(
    ctx: &Context,
    base_url: &str,
    headers: &[(String, String)],
    entity_set: &str,
    entity_id: &str,
) -> Result<String, String> {
    let url = format!("{base_url}/tdata/{entity_set}('{entity_id}')");
    let resp = ctx.http_call("GET", &url, headers, "")?;
    let body = parse_json_response(resp, &format!("get {entity_set}('{entity_id}')"))?;
    Ok(status_from_response(&body))
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

fn status_from_response(value: &Value) -> String {
    value
        .get("Status")
        .or_else(|| value.get("status"))
        .or_else(|| value.pointer("/state/status"))
        .or_else(|| value.pointer("/fields/Status"))
        .or_else(|| value.pointer("/fields/status"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

fn truncate(input: &str, max: usize) -> String {
    if input.len() <= max {
        input.to_string()
    } else {
        format!("{}[truncated]", input.chars().take(max).collect::<String>())
    }
}
