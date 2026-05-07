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
const SIGNALS_PATH: &str = "/tdata/Signals";
const OBSERVABILITY_FINDINGS_PATH: &str = "/tdata/ObservabilityFindings";
const FACTORY_CASES_PATH: &str = "/tdata/FactoryCases";
const PROOF_PACKETS_PATH: &str = "/tdata/ProofPackets";
const PATROL_CONFIGURE: &str = "TemperPaw.Patrol.Configure";
const PATROL_WRITE_PLAN: &str = "TemperPaw.Patrol.WritePlan";
const PATROL_START_WORK: &str = "TemperPaw.Patrol.StartWork";
const PATROL_ATTACH_WORKER_RUN: &str = "TemperPaw.Patrol.AttachWorkerRun";
const PATROL_ATTACH_EVIDENCE_LINKS: &str = "TemperPaw.Patrol.AttachEvidenceLinks";
const PATROL_ESCALATE: &str = "TemperPaw.Patrol.Escalate";
const PATROL_RECORD_EVIDENCE: &str = "TemperPaw.Patrol.RecordEvidence";
const PATROL_COMPLETE: &str = "TemperPaw.Patrol.Complete";
const PATROL_OPEN: &str = "TemperPaw.Patrol.Open";
const PATROL_SET_RISK_FLOOR: &str = "TemperPaw.Patrol.SetRiskFloor";
const PATROL_LINK_SOURCE: &str = "TemperPaw.Patrol.LinkSource";
const PATROL_OPEN_WORK_CYCLE: &str = "TemperPaw.Patrol.OpenWorkCycle";
const PATROL_ATTACH_CASE: &str = "TemperPaw.Patrol.AttachCase";
const PATROL_NORMALIZE: &str = "TemperPaw.Patrol.Normalize";
const PATROL_TRIAGE: &str = "TemperPaw.Patrol.Triage";
const PATROL_OPEN_FINDING: &str = "TemperPaw.Patrol.OpenFinding";
const PATROL_ATTACH_DRAFT: &str = "TemperPaw.Patrol.AttachDraft";
const PATROL_MARK_READY: &str = "TemperPaw.Patrol.MarkReady";
const PATROL_REQUEST_HUMAN_START_APPROVAL: &str = "TemperPaw.Patrol.RequestHumanStartApproval";

#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));
        let base_url = resolve_api_url(&ctx);
        let headers = odata_headers(&ctx);

        if ctx.trigger_action == "RecordEvidence" {
            handle_record_evidence(&ctx, &base_url, &headers, &fields)?;
            return Ok(());
        }
        if ctx.trigger_action != "Start" {
            return Err(format!(
                "patrol_run_lifecycle: unsupported trigger {}",
                ctx.trigger_action
            ));
        }

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
        "You are the local Codex Datadog MCP Patrol agent for TemperPaw paw-patrol.\n\nPatrolRun: {patrol_run_id}\nPatrolKind: datadog_observability\nWorkCycle: {work_cycle_id}\nSummary: {summary}\n\nRequired loop:\n1. Work in the assigned git worktree, but do not edit files for this patrol run.\n2. Use your authenticated Datadog MCP tools to investigate monitors, logs, traces, metrics, incidents, and dashboards for OpenPaw, Temper, TemperPaw, Railway, Discord, OData, WASM, Cedar, workers, and dashboard health.\n3. Do not read, echo, or print secret values.\n4. Return structured findings and proof data between DATADOG_PATROL_RESULT_JSON_BEGIN and DATADOG_PATROL_RESULT_JSON_END. The paw-codex-worker validates that JSON and reports it to PatrolRun.RecordEvidence; paw-patrol WASM creates Signals, ObservabilityFindings, FactoryCases, WorkCycles, ProofPackets, and risk-gated follow-up WorkerRuns.\n5. Create findings only for actionable issues that are present or strongly evidenced now. High-risk or production-impacting fixes must require human approval before implementation.\n6. If a Datadog surface is unavailable through MCP, include that surface in evidence_scope with the limitation explained."
    )
}

fn handle_record_evidence(
    ctx: &Context,
    base_url: &str,
    headers: &[(String, String)],
    fields: &Value,
) -> Result<(), String> {
    let patrol_run_id = entity_id(ctx);
    let evidence_json = string_param(ctx, fields, "evidence_json", "EvidenceJson");
    let existing_signal_ids = string_param(ctx, fields, "signal_ids", "SignalIds");
    if !json_array_is_empty(&existing_signal_ids) {
        set_success_result("", &json!({ "status": "patrol_evidence_already_fanned_out" }));
        return Ok(());
    }

    let evidence: Value = serde_json::from_str(&evidence_json)
        .map_err(|err| format!("PatrolRun.RecordEvidence evidence_json was not valid JSON: {err}"))?;
    let summary = string_value(&evidence, "summary", "Datadog MCP Patrol evidence");
    let findings = evidence
        .get("findings")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let evidence_scope = evidence
        .get("evidence_scope")
        .cloned()
        .unwrap_or_else(|| json!([]));

    let mut signal_ids = Vec::new();
    let mut finding_ids = Vec::new();
    let mut case_ids = Vec::new();
    let mut work_cycle_ids = Vec::new();
    let mut implementer_worker_run_ids = Vec::new();

    for finding in findings.iter().take(8) {
        let title = string_value(finding, "title", "Untitled Datadog Patrol finding");
        let severity = string_value(finding, "severity", "warn");
        let risk_lane = string_value(finding, "risk_lane", "L1");
        let finding_evidence = datadog_finding_evidence(&patrol_run_id, &summary, &evidence_scope, finding);

        let signal_id = create_entity_with_body(
            ctx,
            base_url,
            headers,
            SIGNALS_PATH,
            &json!({
                "fields": {
                    "source": "datadog_mcp",
                    "payload": finding_evidence.to_string(),
                    "source_url": string_value(finding, "source_url", ""),
                    "severity": &severity
                }
            }),
        )?;
        post_action(
            ctx,
            base_url,
            headers,
            "Signals",
            &signal_id,
            PATROL_NORMALIZE,
            &json!({
                "summary": &title,
                "severity": &severity,
            }),
        )?;
        post_action(
            ctx,
            base_url,
            headers,
            "Signals",
            &signal_id,
            PATROL_TRIAGE,
            &json!({
                "summary": format!("Datadog MCP Patrol found actionable evidence: {title}"),
            }),
        )?;

        let finding_id = create_entity(ctx, base_url, headers, OBSERVABILITY_FINDINGS_PATH)?;
        post_action(
            ctx,
            base_url,
            headers,
            "ObservabilityFindings",
            &finding_id,
            PATROL_OPEN_FINDING,
            &json!({
                "title": &title,
                "severity": &severity,
                "risk_lane": &risk_lane,
                "source": "datadog_mcp",
                "datadog_monitor_id": string_value(finding, "datadog_monitor_id", ""),
                "evidence_json": finding_evidence.to_string(),
                "affected_services": affected_services_json(finding),
                "fingerprint": string_value(finding, "fingerprint", ""),
                "patrol_run_id": &patrol_run_id,
                "signal_id": &signal_id,
            }),
        )?;

        let case_id = create_entity(ctx, base_url, headers, FACTORY_CASES_PATH)?;
        post_action(
            ctx,
            base_url,
            headers,
            "FactoryCases",
            &case_id,
            PATROL_OPEN,
            &json!({
                "summary": &title,
                "signal_id": &signal_id,
                "patrol_request_id": "",
                "work_request_id": "",
            }),
        )?;
        post_action(
            ctx,
            base_url,
            headers,
            "FactoryCases",
            &case_id,
            PATROL_SET_RISK_FLOOR,
            &json!({
                "minimum_risk_lane": &risk_lane,
                "risk_floor_source": "datadog_patrol:mcp_agent_investigation",
                "risk_evidence": finding_evidence.to_string(),
            }),
        )?;

        let work_cycle_id = create_entity(ctx, base_url, headers, WORK_CYCLES_PATH)?;
        let task_detail = datadog_followup_task(&patrol_run_id, finding, &summary, &evidence_scope);
        post_action(
            ctx,
            base_url,
            headers,
            "WorkCycles",
            &work_cycle_id,
            PATROL_CONFIGURE,
            &json!({
                "factory_case_id": &case_id,
                "pm_issue_id": "",
                "task_summary": string_value(finding, "work_summary", &title),
                "task_detail": &task_detail,
                "risk_lane": &risk_lane,
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
                "source_entity_type": "ObservabilityFinding",
                "source_entity_id": &finding_id,
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
                "plan_summary": "Investigate the Datadog MCP evidence, reproduce or explain the runtime issue, then make the smallest Temper-native fix with red-green tests, targeted live/E2E evidence, independent review, evaluation gates, and a visual ProofPacket.",
            }),
        )?;
        if finding_requires_start_approval(finding, &risk_lane, &severity) {
            post_action(
                ctx,
                base_url,
                headers,
                "WorkCycles",
                &work_cycle_id,
                PATROL_REQUEST_HUMAN_START_APPROVAL,
                &json!({
                    "approval_summary": format!(
                        "Datadog MCP Patrol classified this as {severity} / {risk_lane}; approve before code or deploy changes are queued. Finding: {title}"
                    ),
                }),
            )?;
        } else {
            let implementer_worker_run_id = create_entity(ctx, base_url, headers, WORKER_RUNS_PATH)?;
            let branch_name = datadog_followup_branch_name(&title, &work_cycle_id);
            let worktree_path = datadog_followup_worktree_path(ctx, &branch_name);
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
                &implementer_worker_run_id,
                PATROL_CONFIGURE,
                &json!({
                    "work_cycle_id": &work_cycle_id,
                    "factory_case_id": &case_id,
                    "risk_lane": &risk_lane,
                    "task": &task_detail,
                    "branch_name": &branch_name,
                    "worktree_path": &worktree_path,
                    "runner_kind": "local_codex",
                    "allowed_worker_id": configured_local_worker_id(ctx),
                    "provider_id": "local-codex",
                    "required_capabilities": "local_codex,repo_write,datadog_query",
                }),
            )?;
            post_action(
                ctx,
                base_url,
                headers,
                "WorkCycles",
                &work_cycle_id,
                PATROL_ATTACH_WORKER_RUN,
                &json!({ "implementer_worker_run_id": &implementer_worker_run_id }),
            )?;
            implementer_worker_run_ids.push(implementer_worker_run_id);
        }

        post_action(
            ctx,
            base_url,
            headers,
            "FactoryCases",
            &case_id,
            PATROL_OPEN_WORK_CYCLE,
            &json!({ "work_cycle_id": &work_cycle_id }),
        )?;
        post_action(
            ctx,
            base_url,
            headers,
            "Signals",
            &signal_id,
            PATROL_ATTACH_CASE,
            &json!({ "factory_case_id": &case_id }),
        )?;

        signal_ids.push(signal_id);
        finding_ids.push(finding_id);
        case_ids.push(case_id);
        work_cycle_ids.push(work_cycle_id);
    }

    let proof_packet_id = create_entity(ctx, base_url, headers, PROOF_PACKETS_PATH)?;
    let worker_run_id = string_param(ctx, fields, "worker_run_id", "WorkerRunId");
    let proof_json = datadog_evidence_with_created(
        &evidence,
        &signal_ids,
        &finding_ids,
        &case_ids,
        &work_cycle_ids,
        &implementer_worker_run_ids,
    );
    let proof_summary = datadog_proof_summary_markdown(
        &patrol_run_id,
        &worker_run_id,
        &summary,
        &evidence_scope,
        &findings,
        &signal_ids,
        &finding_ids,
        &case_ids,
        &work_cycle_ids,
        &implementer_worker_run_ids,
    );
    post_action(
        ctx,
        base_url,
        headers,
        "ProofPackets",
        &proof_packet_id,
        PATROL_ATTACH_DRAFT,
        &json!({
            "work_cycle_id": work_cycle_ids.first().cloned().unwrap_or_default(),
            "worker_run_id": &worker_run_id,
            "review_run_id": "",
            "evaluation_run_id": "",
            "summary_markdown": &proof_summary,
            "proof_json": proof_json.to_string(),
            "visual_summary_url": datadog_visual_summary_url(evidence_scope.as_array().map(Vec::len).unwrap_or(0), finding_ids.len(), work_cycle_ids.len()),
            "state_diagram_mermaid": datadog_state_diagram_mermaid(),
            "changed_files_map": "Datadog MCP Patrol does not edit repository files. Actionable findings become WorkCycles with their own implementation, review, evaluation, and proof loop.",
            "reviewer_verdict": "Patrol evidence packet generated from the local Codex agent's Datadog MCP investigation. Follow-up implementation WorkCycles require independent review before completion.",
            "residual_risks": residual_risks_text(&evidence),
        }),
    )?;
    post_action(
        ctx,
        base_url,
        headers,
        "ProofPackets",
        &proof_packet_id,
        PATROL_MARK_READY,
        &json!({
            "summary_markdown": &proof_summary,
            "proof_json": proof_json.to_string(),
        }),
    )?;
    post_action(
        ctx,
        base_url,
        headers,
        "PatrolRuns",
        &patrol_run_id,
        PATROL_ATTACH_EVIDENCE_LINKS,
        &json!({
            "observability_finding_ids": json_string_array(&finding_ids),
            "signal_ids": json_string_array(&signal_ids),
            "factory_case_ids": json_string_array(&case_ids),
            "work_cycle_ids": json_string_array(&work_cycle_ids),
        }),
    )?;

    set_success_result(
        "Complete",
        &json!({
            "summary": format!(
                "Datadog MCP Patrol investigated {} surface(s), opened {} observability finding(s), and queued {} low-risk implementer run(s).",
                evidence_scope.as_array().map(Vec::len).unwrap_or(0),
                finding_ids.len(),
                implementer_worker_run_ids.len()
            ),
            "proof_packet_id": proof_packet_id,
            "completed_at": unix_to_iso8601(now_secs()),
        }),
    );

    ctx.log(
        "info",
        &format!(
            "patrol_run_lifecycle: fanned out Datadog Patrol evidence for {patrol_run_id}; action={PATROL_RECORD_EVIDENCE}, complete={PATROL_COMPLETE}"
        ),
    );
    Ok(())
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
    create_entity_with_body(ctx, base_url, headers, path, &json!({}))
}

fn create_entity_with_body(
    ctx: &Context,
    base_url: &str,
    headers: &[(String, String)],
    path: &str,
    body: &Value,
) -> Result<String, String> {
    let url = format!("{base_url}{path}");
    let entity_set = path.rsplit('/').next().unwrap_or(path);
    let resp = ctx.http_call("POST", &url, headers, &body.to_string())?;
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

fn string_param(ctx: &Context, fields: &Value, snake: &str, pascal: &str) -> String {
    ctx.trigger_params
        .get(snake)
        .and_then(Value::as_str)
        .or_else(|| ctx.trigger_params.get(pascal).and_then(Value::as_str))
        .map(str::to_string)
        .unwrap_or_else(|| string_from_fields(fields, snake, pascal))
}

fn string_value(value: &Value, key: &str, fallback: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(fallback)
        .to_string()
}

fn bool_value(value: &Value, key: &str) -> bool {
    value
        .get(key)
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn json_array_is_empty(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed.is_empty() || trimmed == "[]"
}

fn json_string_array(values: &[String]) -> String {
    serde_json::to_string(values).unwrap_or_else(|_| "[]".to_string())
}

fn affected_services_json(finding: &Value) -> String {
    finding
        .get("affected_services")
        .cloned()
        .filter(Value::is_array)
        .unwrap_or_else(|| json!([]))
        .to_string()
}

fn datadog_finding_evidence(
    patrol_run_id: &str,
    summary: &str,
    evidence_scope: &Value,
    finding: &Value,
) -> Value {
    json!({
        "patrol_run_id": patrol_run_id,
        "source": "datadog_mcp",
        "summary": summary,
        "evidence_scope": evidence_scope,
        "finding": finding,
    })
}

fn finding_requires_start_approval(finding: &Value, risk_lane: &str, severity: &str) -> bool {
    let risk_lane = risk_lane.to_ascii_lowercase();
    let severity = severity.to_ascii_lowercase();
    bool_value(finding, "requires_human_approval")
        || matches!(risk_lane.as_str(), "l2" | "l3")
        || matches!(severity.as_str(), "error" | "critical")
        || sensitive_followup_surface(finding)
}

fn sensitive_followup_surface(finding: &Value) -> bool {
    let affected = finding
        .get("affected_services")
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap_or_default();
    let combined = format!(
        "{} {} {} {}",
        affected,
        string_value(finding, "title", ""),
        string_value(finding, "work_summary", ""),
        string_value(finding, "work_detail", "")
    )
    .to_ascii_lowercase();
    [
        "paw-agent",
        "paw-channels",
        "discord",
        "channel",
        "transport",
        "railway",
        "deploy",
        "deployment",
        "production",
        "secret",
        "cedar",
    ]
    .iter()
    .any(|needle| combined.contains(needle))
}

fn datadog_followup_task(
    patrol_run_id: &str,
    finding: &Value,
    summary: &str,
    evidence_scope: &Value,
) -> String {
    let title = string_value(finding, "title", "Datadog Patrol follow-up");
    let severity = string_value(finding, "severity", "warn");
    let risk_lane = string_value(finding, "risk_lane", "L1");
    let source_url = string_value(finding, "source_url", "");
    let work_detail = string_value(
        finding,
        "work_detail",
        "Investigate the Datadog MCP evidence and make the smallest safe Temper-native fix.",
    );
    format!(
        "You are the local Codex implementer for a Paw Patrol Datadog MCP observability finding.\n\nPatrolRun: {patrol_run_id}\nPatrol kind: datadog_observability\nFinding: {title}\nSeverity: {severity}\nRisk lane: {risk_lane}\nSource URL: {source_url}\n\nPatrol summary:\n{summary}\n\nEvidence JSON:\n{}\n\nRequired loop:\n1. Work in the assigned git worktree and branch only after this WorkCycle is allowed to start.\n2. Use Datadog MCP read-only evidence to reproduce or explain the issue; run extra Datadog MCP queries when needed.\n3. Keep all orchestration Temper-native: specs, WASM integrations, Cedar policies, dashboard views, and Temper actions.\n4. Make the smallest safe fix with red-green TDD, then run focused tests and live/E2E checks.\n5. Produce a visual ProofPacket with state diagrams, OData links, Datadog links, tests, residual risks, and reviewer/evaluator verdicts.\n\nAgent-provided work detail:\n{work_detail}",
        datadog_finding_evidence(patrol_run_id, summary, evidence_scope, finding)
    )
}

fn datadog_followup_branch_name(title: &str, work_cycle_id: &str) -> String {
    format!(
        "codex/paw-datadog-{}-{}",
        slug(title, 42),
        short_id(work_cycle_id)
    )
}

fn datadog_followup_worktree_path(ctx: &Context, branch_name: &str) -> String {
    format!(
        "{}/{}",
        configured_local_worktree_root(ctx).trim_end_matches('/'),
        branch_name.replace('/', "-")
    )
}

fn slug(input: &str, max: usize) -> String {
    let mut slug = String::new();
    let mut last_dash = false;
    for ch in input.chars() {
        let next = if ch.is_ascii_alphanumeric() {
            last_dash = false;
            Some(ch.to_ascii_lowercase())
        } else if !last_dash {
            last_dash = true;
            Some('-')
        } else {
            None
        };
        if let Some(next) = next {
            slug.push(next);
        }
        if slug.len() >= max {
            break;
        }
    }
    let slug = slug.trim_matches('-').to_string();
    if slug.is_empty() {
        "finding".to_string()
    } else {
        slug
    }
}

fn datadog_evidence_with_created(
    evidence: &Value,
    signal_ids: &[String],
    finding_ids: &[String],
    case_ids: &[String],
    work_cycle_ids: &[String],
    implementer_worker_run_ids: &[String],
) -> Value {
    let mut with_created = evidence.clone();
    if let Some(object) = with_created.as_object_mut() {
        object.insert(
            "created".to_string(),
            json!({
                "signals": signal_ids,
                "observability_findings": finding_ids,
                "factory_cases": case_ids,
                "work_cycles": work_cycle_ids,
                "implementer_worker_runs": implementer_worker_run_ids,
            }),
        );
    }
    with_created
}

fn datadog_proof_summary_markdown(
    patrol_run_id: &str,
    worker_run_id: &str,
    summary: &str,
    evidence_scope: &Value,
    findings: &[Value],
    signal_ids: &[String],
    finding_ids: &[String],
    case_ids: &[String],
    work_cycle_ids: &[String],
    implementer_worker_run_ids: &[String],
) -> String {
    let surfaces = evidence_scope
        .as_array()
        .map(|items| {
            items
                .iter()
                .map(|scope| {
                    format!(
                        "- {}: {}",
                        string_value(scope, "surface", "surface"),
                        truncate(&string_value(scope, "result_summary", ""), 300)
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default();
    let findings_text = findings
        .iter()
        .map(|finding| {
            format!(
                "- {} [{} / {}] -> {}",
                string_value(finding, "title", "Untitled finding"),
                string_value(finding, "severity", "warn"),
                string_value(finding, "risk_lane", "L1"),
                string_value(finding, "work_summary", "follow-up work")
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let findings_text = if findings_text.trim().is_empty() {
        "- No actionable findings opened.".to_string()
    } else {
        findings_text
    };

    format!(
        "# Datadog MCP Patrol Proof\n\nPatrolRun `{patrol_run_id}` was executed by WorkerRun `{worker_run_id}` using the local Codex agent and authenticated Datadog MCP tools.\n\n```mermaid\n{}\n```\n\n## Result\n\n{}\n\n## Evidence Scope\n\n{}\n\n## Findings\n\n{}\n\n## Created Temper Entities\n\n- Signals: {}\n- ObservabilityFindings: {}\n- FactoryCases: {}\n- WorkCycles: {}\n- Low-risk implementer WorkerRuns queued: {}\n\n## Gate Posture\n\nThe patrol does not mutate code or production. Actionable findings become WorkCycles; high-risk or production-impacting work pauses before implementation.",
        datadog_state_diagram_mermaid(),
        summary.trim(),
        if surfaces.trim().is_empty() { "- No evidence scope recorded." } else { &surfaces },
        findings_text,
        signal_ids.len(),
        finding_ids.len(),
        case_ids.len(),
        work_cycle_ids.len(),
        implementer_worker_run_ids.len(),
    )
}

fn datadog_state_diagram_mermaid() -> &'static str {
    "flowchart LR\n  Run[\"PatrolRun datadog_observability\"] --> Worker[\"WorkerRun\"]\n  Worker --> Codex[\"Codex agent\"]\n  Codex --> MCP[\"Datadog MCP investigation\"]\n  MCP --> Scope[\"monitors logs traces metrics incidents dashboards\"]\n  Scope --> Signals[\"Signals\"]\n  Scope --> Findings[\"ObservabilityFindings\"]\n  Findings --> Cases[\"FactoryCases\"]\n  Cases --> Work[\"Risk-gated WorkCycles\"]\n  Worker --> Proof[\"Visual ProofPacket\"]\n  Proof --> Complete[\"PatrolRun Complete\"]"
}

fn datadog_visual_summary_url(
    evidence_surface_count: usize,
    finding_count: usize,
    work_cycle_count: usize,
) -> String {
    let svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"960\" height=\"540\" viewBox=\"0 0 960 540\"><rect width=\"960\" height=\"540\" fill=\"#f7f5ef\"/><rect x=\"40\" y=\"36\" width=\"880\" height=\"468\" rx=\"8\" fill=\"#ffffff\" stroke=\"#d5d2c6\"/><text x=\"70\" y=\"96\" font-family=\"Arial\" font-size=\"32\" font-weight=\"700\" fill=\"#202124\">Datadog MCP Patrol Proof</text><text x=\"70\" y=\"142\" font-family=\"Arial\" font-size=\"16\" fill=\"#64615a\">Codex investigated Datadog via MCP; Patrol WASM created Temper work.</text><text x=\"90\" y=\"260\" font-family=\"Arial\" font-size=\"72\" font-weight=\"700\" fill=\"#a15c00\">{evidence_surface_count}</text><text x=\"90\" y=\"300\" font-family=\"Arial\" font-size=\"18\" fill=\"#64615a\">Evidence surfaces</text><text x=\"390\" y=\"260\" font-family=\"Arial\" font-size=\"72\" font-weight=\"700\" fill=\"#174ea6\">{finding_count}</text><text x=\"390\" y=\"300\" font-family=\"Arial\" font-size=\"18\" fill=\"#64615a\">Findings</text><text x=\"650\" y=\"260\" font-family=\"Arial\" font-size=\"72\" font-weight=\"700\" fill=\"#137333\">{work_cycle_count}</text><text x=\"650\" y=\"300\" font-family=\"Arial\" font-size=\"18\" fill=\"#64615a\">WorkCycles</text><text x=\"70\" y=\"420\" font-family=\"Arial\" font-size=\"18\" fill=\"#202124\">PatrolRun -> Codex -> Datadog MCP -> Signals -> Findings -> Cases -> WorkCycles -> ProofPacket</text></svg>"
    );
    format!("data:image/svg+xml,{}", percent_encode_data_url(&svg))
}

fn percent_encode_data_url(input: &str) -> String {
    input
        .bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (byte as char).to_string()
            }
            other => format!("%{other:02X}"),
        })
        .collect()
}

fn residual_risks_text(evidence: &Value) -> String {
    evidence
        .get("residual_risks")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
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
    if input.chars().count() <= max {
        input.to_string()
    } else {
        format!("{}[truncated]", input.chars().take(max).collect::<String>())
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
