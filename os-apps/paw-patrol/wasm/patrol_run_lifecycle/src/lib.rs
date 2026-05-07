//! Patrol Run Lifecycle - queue capable workers for active Risk Patrol.
//!
//! `PatrolRun.Start` is the Temper-native control point for active
//! investigations such as `datadog_observability`. This module looks up a
//! registered `WorkerAgent` with the required capability, creates a WorkCycle
//! and WorkerRun for the local Codex Datadog MCP Patrol, then records the
//! linkage on the PatrolRun. If no capable worker exists, it escalates visibly.

use temper_wasm_sdk::prelude::*;

const WORKER_AGENTS_PATH: &str = "/tdata/WorkerAgents";
const WORK_CYCLES_PATH: &str = "/tdata/WorkCycles";
const WORKER_RUNS_PATH: &str = "/tdata/WorkerRuns";
const PATROL_CONFIGURE: &str = "TemperPaw.Patrol.Configure";
const PATROL_WRITE_PLAN: &str = "TemperPaw.Patrol.WritePlan";
const PATROL_START_WORK: &str = "TemperPaw.Patrol.StartWork";
const PATROL_ATTACH_WORKER_RUN: &str = "TemperPaw.Patrol.AttachWorkerRun";
const PATROL_ESCALATE: &str = "TemperPaw.Patrol.Escalate";

#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));
        let base_url = resolve_api_url(&ctx);
        let headers = odata_headers(&ctx);

        let patrol_run_id = entity_id(&ctx);
        let patrol_kind = nonempty_or(
            &string_from_fields(&fields, "patrol_kind", "PatrolKind"),
            "datadog_observability",
        );
        let summary = nonempty_or(
            &string_from_fields(&fields, "summary", "Summary"),
            "Datadog observability Risk Patrol",
        );
        let required_capabilities = nonempty_or(
            &string_from_fields(&fields, "required_capabilities", "RequiredCapabilities"),
            "datadog_query",
        );

        if patrol_kind != "datadog_observability" {
            set_success_result(
                "Escalate",
                &json!({
                    "error_message": format!("Unsupported PatrolRun kind '{patrol_kind}'."),
                    "integration": PATROL_ESCALATE
                }),
            );
            return Ok(());
        }

        let worker = match find_capable_worker(&ctx, &base_url, &headers, &required_capabilities)? {
            Some(worker) => worker,
            None => {
                set_success_result(
                    "Escalate",
                    &json!({
                        "error_message": format!(
                        "No active WorkerAgent advertises required_capabilities '{required_capabilities}' for Datadog Patrol."
                        ),
                        "integration": PATROL_ESCALATE
                    }),
                );
                return Ok(());
            }
        };

        let work_cycle_id = create_entity(&ctx, &base_url, &headers, WORK_CYCLES_PATH)?;
        let worker_run_id = create_entity(&ctx, &base_url, &headers, WORKER_RUNS_PATH)?;
        let branch_name = format!("codex/paw-datadog-patrol-{}", short_id(&patrol_run_id));
        let worktree_path = format!(
            "{}/{}",
            configured_local_worktree_root(&ctx).trim_end_matches('/'),
            branch_name.replace('/', "-")
        );
        let task = datadog_patrol_task(&patrol_run_id, &work_cycle_id, &summary);

        post_action(
            &ctx,
            &base_url,
            &headers,
            "WorkCycles",
            &work_cycle_id,
            PATROL_CONFIGURE,
            &json!({
                "factory_case_id": "",
                "pm_issue_id": "",
                "task_summary": format!("Risk Patrol: {summary}"),
                "task_detail": &task,
                "risk_lane": "L1"
            }),
        )?;
        post_action(
            &ctx,
            &base_url,
            &headers,
            "WorkCycles",
            &work_cycle_id,
            PATROL_WRITE_PLAN,
            &json!({
                "plan_summary": "Run Datadog observability Risk Patrol, create Signals/ObservabilityFindings/Cases/WorkCycles for real issues, and produce proof for review."
            }),
        )?;
        post_action(
            &ctx,
            &base_url,
            &headers,
            "WorkCycles",
            &work_cycle_id,
            PATROL_START_WORK,
            &json!({}),
        )?;
        post_action(
            &ctx,
            &base_url,
            &headers,
            "WorkerRuns",
            &worker_run_id,
            PATROL_CONFIGURE,
            &json!({
                "work_cycle_id": &work_cycle_id,
                "factory_case_id": "",
                "risk_lane": "L1",
                "task": &task,
                "branch_name": &branch_name,
                "worktree_path": &worktree_path,
                "runner_kind": "local_codex",
                "allowed_worker_id": worker.worker_id,
                "provider_id": worker.provider_id,
                "required_capabilities": required_capabilities
            }),
        )?;
        post_action(
            &ctx,
            &base_url,
            &headers,
            "WorkCycles",
            &work_cycle_id,
            PATROL_ATTACH_WORKER_RUN,
            &json!({ "implementer_worker_run_id": &worker_run_id }),
        )?;

        set_success_result(
            "AttachWorkerRun",
            &json!({
                "worker_run_id": worker_run_id,
                "started_at": unix_to_iso8601(now_secs())
            }),
        );
        Ok(())
    })();

    if let Err(error) = result {
        set_error_result(&error);
    }
    0
}

struct CapableWorker {
    worker_id: String,
    provider_id: String,
}

fn find_capable_worker(
    ctx: &Context,
    base_url: &str,
    headers: &[(String, String)],
    required_capabilities: &str,
) -> Result<Option<CapableWorker>, String> {
    let required = capability_list(required_capabilities);
    let url = format!("{base_url}{WORKER_AGENTS_PATH}");
    let resp = ctx.http_call("GET", &url, headers, "")?;
    let body = parse_json_response(resp, "list WorkerAgents")?;
    let values = body
        .get("value")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    for entity in values {
        let fields = entity.get("fields").cloned().unwrap_or_else(|| json!({}));
        let status = string_from_entity(&entity, &fields, "status", "Status");
        if status != "Active" && status != "Registered" {
            continue;
        }
        let capabilities = capability_list(&string_from_entity(
            &entity,
            &fields,
            "capabilities",
            "Capabilities",
        ));
        if required.iter().all(|capability| capabilities.contains(capability)) {
            let worker_id = nonempty_or(
                &string_from_entity(&entity, &fields, "worker_id", "WorkerId"),
                &entity_id_from_response(&entity).unwrap_or_default(),
            );
            let provider_id = nonempty_or(
                &string_from_entity(&entity, &fields, "provider_id", "ProviderId"),
                "local-codex",
            );
            if !worker_id.is_empty() {
                return Ok(Some(CapableWorker {
                    worker_id,
                    provider_id,
                }));
            }
        }
    }

    // Fallback for first boot when seed data has not appeared yet.
    let local_worker = configured_local_worker_id(ctx);
    if !local_worker.is_empty() && required.iter().all(|capability| capability == "datadog_query") {
        return Ok(Some(CapableWorker {
            worker_id: local_worker,
            provider_id: "local-codex".to_string(),
        }));
    }

    Ok(None)
}

fn datadog_patrol_task(patrol_run_id: &str, work_cycle_id: &str, summary: &str) -> String {
    format!(
        "You are the local Codex Datadog MCP Patrol agent for TemperPaw paw-patrol.\n\nPatrolRun: {patrol_run_id}\nPatrolKind: datadog_observability\nWorkCycle: {work_cycle_id}\nSummary: {summary}\n\nRequired loop:\n1. Work in the assigned git worktree, but do not edit files for this patrol run.\n2. Use your authenticated Datadog MCP tools to investigate monitors, logs, traces, metrics, incidents, and dashboards for OpenPaw, Temper, TemperPaw, Railway, Discord, OData, WASM, Cedar, workers, and dashboard health.\n3. Do not read, echo, or print secret values.\n4. Return structured findings and proof data between DATADOG_PATROL_RESULT_JSON_BEGIN and DATADOG_PATROL_RESULT_JSON_END. The paw-codex-worker validates that JSON and writes Signals, ObservabilityFindings, FactoryCases, WorkCycles, ProofPackets, and PatrolRun evidence back through Temper actions.\n5. Create findings only for actionable issues that are present or strongly evidenced now. High-risk or production-impacting fixes must require human approval before implementation.\n6. If a Datadog surface is unavailable through MCP, include that surface in evidence_scope with the limitation explained."
    )
}

fn capability_list(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect()
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

fn string_from_entity(entity: &Value, fields: &Value, snake: &str, pascal: &str) -> String {
    entity
        .get(snake)
        .and_then(Value::as_str)
        .or_else(|| entity.get(pascal).and_then(Value::as_str))
        .or_else(|| fields.get(snake).and_then(Value::as_str))
        .or_else(|| fields.get(pascal).and_then(Value::as_str))
        .unwrap_or("")
        .to_string()
}

fn string_from_fields(fields: &Value, snake: &str, pascal: &str) -> String {
    fields
        .get(snake)
        .and_then(Value::as_str)
        .or_else(|| fields.get(pascal).and_then(Value::as_str))
        .unwrap_or("")
        .to_string()
}

fn entity_id_from_response(value: &Value) -> Option<String> {
    value
        .get("entity_id")
        .or_else(|| value.get("id"))
        .or_else(|| value.get("Id"))
        .and_then(Value::as_str)
        .map(str::to_string)
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

fn entity_id(ctx: &Context) -> String {
    if ctx.entity_id.trim().is_empty() {
        "unknown".to_string()
    } else {
        ctx.entity_id.clone()
    }
}

fn nonempty_or(value: &str, fallback: &str) -> String {
    if value.trim().is_empty() {
        fallback.to_string()
    } else {
        value.to_string()
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

fn truncate(input: &str, max: usize) -> String {
    if input.len() <= max {
        input.to_string()
    } else {
        format!("{}[truncated]", &input[..max])
    }
}

fn now_secs() -> u64 {
    (Context::get_time_millis() / 1000) as u64
}

fn unix_to_iso8601(secs: u64) -> String {
    let mut days = (secs / 86_400) as i64;
    let day_secs = secs % 86_400;
    let hour = day_secs / 3_600;
    let minute = (day_secs % 3_600) / 60;
    let second = day_secs % 60;

    let mut year = 1970i64;
    loop {
        let ydays = if is_leap_year(year) { 366 } else { 365 };
        if days < ydays {
            break;
        }
        days -= ydays;
        year += 1;
    }

    let leap = is_leap_year(year);
    let mdays = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut month = 0usize;
    for (index, month_days) in mdays.iter().enumerate() {
        if days < *month_days as i64 {
            month = index + 1;
            break;
        }
        days -= *month_days as i64;
    }
    let day = days + 1;

    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

fn is_leap_year(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}
