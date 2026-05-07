//! Review Gate Lifecycle - fan reviewer and evaluator outcomes into proof gates.
//!
//! Triggered by ReviewRun and EvaluationRun terminal actions. The worker draft is
//! not enough by itself: this module records that reviewer approved before
//! human review, waits until evaluation gates passed before proof readiness, and
//! then advances WorkCycle, ProofPacket, and FactoryCase through visible Temper
//! state transitions.
//! Contract phrase: reviewer approved before human review.

use temper_wasm_sdk::prelude::*;

const FACTORY_CASES_PATH: &str = "/tdata/FactoryCases";
const WORK_CYCLES_PATH: &str = "/tdata/WorkCycles";
const EVALUATION_RUNS_PATH: &str = "/tdata/EvaluationRuns";
const PROOF_PACKETS_PATH: &str = "/tdata/ProofPackets";

const PATROL_BEGIN_PROOF: &str = "TemperPaw.Patrol.BeginProof";
const PATROL_COMPLETE: &str = "TemperPaw.Patrol.Complete";
const PATROL_ESCALATE: &str = "TemperPaw.Patrol.Escalate";
const PATROL_FAIL: &str = "TemperPaw.Patrol.Fail";
const PATROL_PASS_REVIEW: &str = "TemperPaw.Patrol.PassReview";
const PATROL_REQUEST_CHANGES: &str = "TemperPaw.Patrol.RequestChanges";
const PATROL_REPORT_E2E: &str = "TemperPaw.Patrol.ReportE2e";
const PATROL_PASS_EVALUATION: &str = "TemperPaw.Patrol.PassEvaluation";
const PATROL_ATTACH_PROOF_PACKET: &str = "TemperPaw.Patrol.AttachProofPacket";
const PATROL_REQUEST_HUMAN_COMPLETION_APPROVAL: &str =
    "TemperPaw.Patrol.RequestHumanCompletionApproval";
const PATROL_ATTACH_DRAFT: &str = "TemperPaw.Patrol.AttachDraft";
const PATROL_MARK_READY: &str = "TemperPaw.Patrol.MarkReady";
const PATROL_REJECT: &str = "TemperPaw.Patrol.Reject";

#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        let base_url = resolve_api_url(&ctx);
        let headers = odata_headers(&ctx);
        let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));

        match ctx.trigger_action.as_str() {
            "Approve" => handle_review_approved(&ctx, &base_url, &headers, &fields),
            "RequestChanges" => handle_review_changes_requested(&ctx, &base_url, &headers, &fields),
            "Escalate" => handle_review_escalated(&ctx, &base_url, &headers, &fields),
            "Pass" => handle_evaluation_passed(&ctx, &base_url, &headers, &fields),
            "Fail" if is_entity_type(&ctx, "ReviewRun") => {
                handle_review_failed(&ctx, &base_url, &headers, &fields)
            }
            "Fail" if is_entity_type(&ctx, "EvaluationRun") => {
                handle_evaluation_failed(&ctx, &base_url, &headers, &fields)
            }
            other => Err(format!(
                "review_gate_lifecycle: unsupported {} trigger {other}",
                ctx.entity_type
            )),
        }?;

        set_success_result("", &json!({ "status": "review_gate_lifecycle_complete" }));
        Ok(())
    })();

    if let Err(error) = result {
        set_error_result(&error);
    }
    0
}

fn handle_review_approved(
    ctx: &Context,
    base_url: &str,
    headers: &[(String, String)],
    fields: &Value,
) -> Result<(), String> {
    let review_run_id = entity_id(ctx);
    let work_cycle_id = string_param(ctx, fields, "work_cycle_id", "WorkCycleId");
    let proof_packet_id = string_param(ctx, fields, "proof_packet_id", "ProofPacketId");
    let review_summary = string_param(ctx, fields, "review_summary", "ReviewSummary");
    let live_e2e_summary = string_param(ctx, fields, "live_e2e_summary", "LiveE2eSummary");
    let verdict = string_param(ctx, fields, "verdict", "Verdict");

    if work_cycle_id.is_empty() {
        return Err("review_gate_lifecycle: ReviewRun missing work_cycle_id".to_string());
    }

    let work_cycle = get_entity(
        ctx,
        base_url,
        headers,
        entity_set(WORK_CYCLES_PATH),
        &work_cycle_id,
    )?;
    if status_from_response(&work_cycle) == "Reviewing"
        && !bool_from_entity(&work_cycle, "review_passed", "ReviewPassed")
    {
        record_e2e_if_present(
            ctx,
            base_url,
            headers,
            &work_cycle_id,
            "ReviewRun",
            &review_run_id,
            &live_e2e_summary,
        )?;
        post_action(
            ctx,
            base_url,
            headers,
            entity_set(WORK_CYCLES_PATH),
            &work_cycle_id,
            PATROL_PASS_REVIEW,
            &json!({ "reviewer_run_id": &review_run_id }),
        )?;
    }

    if !proof_packet_id.is_empty() {
        update_proof_review(
            ctx,
            base_url,
            headers,
            &proof_packet_id,
            &reviewer_verdict(&verdict, &review_summary, &live_e2e_summary),
            "Automated evaluation must pass before proof readiness.",
        )?;
    }

    finalize_if_ready(
        ctx,
        base_url,
        headers,
        &work_cycle_id,
        proof_packet_id.as_str(),
        "",
    )
}

fn handle_review_changes_requested(
    ctx: &Context,
    base_url: &str,
    headers: &[(String, String)],
    fields: &Value,
) -> Result<(), String> {
    let work_cycle_id = string_param(ctx, fields, "work_cycle_id", "WorkCycleId");
    let proof_packet_id = string_param(ctx, fields, "proof_packet_id", "ProofPacketId");
    let review_summary = string_param(ctx, fields, "review_summary", "ReviewSummary");
    let live_e2e_summary = string_param(ctx, fields, "live_e2e_summary", "LiveE2eSummary");
    let verdict = string_param(ctx, fields, "verdict", "Verdict");
    let message = reviewer_verdict(&verdict, &review_summary, &live_e2e_summary);

    if !work_cycle_id.is_empty() {
        let work_cycle = get_entity(
            ctx,
            base_url,
            headers,
            entity_set(WORK_CYCLES_PATH),
            &work_cycle_id,
        )?;
        if status_from_response(&work_cycle) == "Reviewing" {
            post_action(
                ctx,
                base_url,
                headers,
                entity_set(WORK_CYCLES_PATH),
                &work_cycle_id,
                PATROL_REQUEST_CHANGES,
                &json!({ "error_message": &message }),
            )?;
        }
    }

    reject_proof_if_needed(ctx, base_url, headers, &proof_packet_id, &message)
}

fn handle_review_escalated(
    ctx: &Context,
    base_url: &str,
    headers: &[(String, String)],
    fields: &Value,
) -> Result<(), String> {
    let work_cycle_id = string_param(ctx, fields, "work_cycle_id", "WorkCycleId");
    let proof_packet_id = string_param(ctx, fields, "proof_packet_id", "ProofPacketId");
    let review_summary = string_param(ctx, fields, "review_summary", "ReviewSummary");
    let verdict = string_param(ctx, fields, "verdict", "Verdict");
    let message = reviewer_verdict(&verdict, &review_summary, "");
    fail_work_cycle_and_escalate_case(ctx, base_url, headers, &work_cycle_id, &message)?;
    reject_proof_if_needed(ctx, base_url, headers, &proof_packet_id, &message)
}

fn handle_review_failed(
    ctx: &Context,
    base_url: &str,
    headers: &[(String, String)],
    fields: &Value,
) -> Result<(), String> {
    let work_cycle_id = string_param(ctx, fields, "work_cycle_id", "WorkCycleId");
    let proof_packet_id = string_param(ctx, fields, "proof_packet_id", "ProofPacketId");
    let review_summary = string_param(ctx, fields, "review_summary", "ReviewSummary");
    let message = if review_summary.trim().is_empty() {
        "ReviewRun failed without a detailed error.".to_string()
    } else {
        review_summary
    };
    fail_work_cycle_and_escalate_case(ctx, base_url, headers, &work_cycle_id, &message)?;
    reject_proof_if_needed(ctx, base_url, headers, &proof_packet_id, &message)
}

fn handle_evaluation_passed(
    ctx: &Context,
    base_url: &str,
    headers: &[(String, String)],
    fields: &Value,
) -> Result<(), String> {
    let evaluation_run_id = entity_id(ctx);
    let work_cycle_id = string_param(ctx, fields, "work_cycle_id", "WorkCycleId");
    let results_json = string_param(ctx, fields, "results_json", "ResultsJson");
    let e2e_summary = string_param(ctx, fields, "e2e_summary", "E2eSummary");
    if work_cycle_id.is_empty() {
        return Err("review_gate_lifecycle: EvaluationRun missing work_cycle_id".to_string());
    }
    let e2e_evidence = if e2e_summary.trim().is_empty() {
        format!(
            "EvaluationRun {evaluation_run_id} passed with results_json evidence: {}",
            truncate(&results_json, 500)
        )
    } else {
        e2e_summary
    };
    record_e2e_if_present(
        ctx,
        base_url,
        headers,
        &work_cycle_id,
        "EvaluationRun",
        &evaluation_run_id,
        &e2e_evidence,
    )?;
    wait_for_bool(
        ctx,
        base_url,
        headers,
        entity_set(WORK_CYCLES_PATH),
        &work_cycle_id,
        "e2e_ok",
        "E2eOk",
    )?;
    finalize_if_ready(
        ctx,
        base_url,
        headers,
        &work_cycle_id,
        "",
        &evaluation_run_id,
    )
}

fn handle_evaluation_failed(
    ctx: &Context,
    base_url: &str,
    headers: &[(String, String)],
    fields: &Value,
) -> Result<(), String> {
    let evaluation_run_id = entity_id(ctx);
    let work_cycle_id = string_param(ctx, fields, "work_cycle_id", "WorkCycleId");
    let error_message = string_param(ctx, fields, "error_message", "ErrorMessage");
    let results_json = string_param(ctx, fields, "results_json", "ResultsJson");
    let message = if error_message.trim().is_empty() {
        format!("EvaluationRun {evaluation_run_id} failed. Results: {results_json}")
    } else {
        error_message
    };

    if work_cycle_id.is_empty() {
        return Err("review_gate_lifecycle: EvaluationRun missing work_cycle_id".to_string());
    }

    let work_cycle = get_entity(
        ctx,
        base_url,
        headers,
        entity_set(WORK_CYCLES_PATH),
        &work_cycle_id,
    )?;
    let status = status_from_response(&work_cycle);
    if status == "Reviewing" {
        post_action(
            ctx,
            base_url,
            headers,
            entity_set(WORK_CYCLES_PATH),
            &work_cycle_id,
            PATROL_REQUEST_CHANGES,
            &json!({
                "error_message": format!(
                    "Automated evaluation requested rework. EvaluationRun {evaluation_run_id}: {message}"
                )
            }),
        )?;
    } else {
        fail_work_cycle_and_escalate_case(ctx, base_url, headers, &work_cycle_id, &message)?;
    }

    let proof_packet_id = find_proof_packet(
        ctx,
        base_url,
        headers,
        &work_cycle_id,
        &evaluation_run_id,
        "",
    )?;
    reject_proof_if_needed(ctx, base_url, headers, &proof_packet_id, &message)
}

fn finalize_if_ready(
    ctx: &Context,
    base_url: &str,
    headers: &[(String, String)],
    work_cycle_id: &str,
    known_proof_packet_id: &str,
    known_passed_evaluation_run_id: &str,
) -> Result<(), String> {
    let mut work_cycle = get_entity(
        ctx,
        base_url,
        headers,
        entity_set(WORK_CYCLES_PATH),
        work_cycle_id,
    )?;
    let mut status = status_from_response(&work_cycle);
    let evaluation_run_id = string_from_entity(&work_cycle, "evaluation_run_id", "EvaluationRunId");

    if status == "Reviewing" {
        let review_passed = bool_from_entity(&work_cycle, "review_passed", "ReviewPassed");
        let evaluation_passed = !evaluation_run_id.is_empty()
            && (evaluation_run_id == known_passed_evaluation_run_id
                || get_status(
                    ctx,
                    base_url,
                    headers,
                    entity_set(EVALUATION_RUNS_PATH),
                    &evaluation_run_id,
                )? == "Passed");

        let e2e_ok = bool_from_entity(&work_cycle, "e2e_ok", "E2eOk");

        if review_passed && evaluation_passed && e2e_ok {
            post_action(
                ctx,
                base_url,
                headers,
                entity_set(WORK_CYCLES_PATH),
                work_cycle_id,
                PATROL_PASS_EVALUATION,
                &json!({ "evaluation_run_id": &evaluation_run_id }),
            )?;
            work_cycle = wait_for_entity_status(
                ctx,
                base_url,
                headers,
                entity_set(WORK_CYCLES_PATH),
                work_cycle_id,
                "Proving",
                &["AwaitingHumanCompletionApproval", "Complete"],
            )?;
            status = status_from_response(&work_cycle);
        } else {
            ctx.log(
                "info",
                "review_gate_lifecycle: waiting for review, evaluation, and ReportE2e before proof readiness",
            );
            return Ok(());
        }
    }

    if status != "Proving" {
        return Ok(());
    }

    begin_case_proof_if_needed(ctx, base_url, headers, &work_cycle)?;

    let proof_packet_id = find_proof_packet(
        ctx,
        base_url,
        headers,
        work_cycle_id,
        &evaluation_run_id,
        known_proof_packet_id,
    )?;
    if proof_packet_id.is_empty() {
        return Err(format!(
            "review_gate_lifecycle: WorkCycle {work_cycle_id} has no ProofPacket to mark ready"
        ));
    }

    mark_proof_ready_if_needed(ctx, base_url, headers, &proof_packet_id)?;

    work_cycle = get_entity(
        ctx,
        base_url,
        headers,
        entity_set(WORK_CYCLES_PATH),
        work_cycle_id,
    )?;
    if status_from_response(&work_cycle) == "Proving"
        && !bool_from_entity(&work_cycle, "proof_attached", "ProofAttached")
    {
        post_action(
            ctx,
            base_url,
            headers,
            entity_set(WORK_CYCLES_PATH),
            work_cycle_id,
            PATROL_ATTACH_PROOF_PACKET,
            &json!({ "proof_packet_id": &proof_packet_id }),
        )?;
        work_cycle = wait_for_bool(
            ctx,
            base_url,
            headers,
            entity_set(WORK_CYCLES_PATH),
            work_cycle_id,
            "proof_attached",
            "ProofAttached",
        )?;
    }

    if status_from_response(&work_cycle) == "Proving"
        && bool_from_entity(&work_cycle, "review_passed", "ReviewPassed")
        && bool_from_entity(&work_cycle, "evaluation_passed", "EvaluationPassed")
        && bool_from_entity(&work_cycle, "proof_attached", "ProofAttached")
        && bool_from_entity(&work_cycle, "e2e_ok", "E2eOk")
        && requires_human_completion_approval(&work_cycle)
    {
        // L3 completion pauses here: human completion approval required before WorkCycle.Complete.
        // A human or supervisor unblocks it with ApproveHumanCompletion.
        post_action(
            ctx,
            base_url,
            headers,
            entity_set(WORK_CYCLES_PATH),
            work_cycle_id,
            PATROL_REQUEST_HUMAN_COMPLETION_APPROVAL,
            &json!({
                "approval_summary": format!(
                    "Risk lane {} requires human completion approval after reviewer, evaluator, and ProofPacket gates passed.",
                    string_from_entity(&work_cycle, "risk_lane", "RiskLane")
                )
            }),
        )?;
        return Ok(());
    }

    if status_from_response(&work_cycle) == "Proving"
        && bool_from_entity(&work_cycle, "review_passed", "ReviewPassed")
        && bool_from_entity(&work_cycle, "evaluation_passed", "EvaluationPassed")
        && bool_from_entity(&work_cycle, "proof_attached", "ProofAttached")
        && bool_from_entity(&work_cycle, "e2e_ok", "E2eOk")
    {
        post_action(
            ctx,
            base_url,
            headers,
            entity_set(WORK_CYCLES_PATH),
            work_cycle_id,
            PATROL_COMPLETE,
            &json!({}),
        )?;
    }

    work_cycle = get_entity(
        ctx,
        base_url,
        headers,
        entity_set(WORK_CYCLES_PATH),
        work_cycle_id,
    )?;
    if status_from_response(&work_cycle) == "Complete" {
        complete_case_if_needed(ctx, base_url, headers, &work_cycle, &proof_packet_id)?;
    }

    Ok(())
}

fn update_proof_review(
    ctx: &Context,
    base_url: &str,
    headers: &[(String, String)],
    proof_packet_id: &str,
    reviewer_verdict: &str,
    residual_risks: &str,
) -> Result<(), String> {
    let proof = get_entity(
        ctx,
        base_url,
        headers,
        entity_set(PROOF_PACKETS_PATH),
        proof_packet_id,
    )?;
    if status_from_response(&proof) != "Drafting" {
        return Ok(());
    }

    post_action(
        ctx,
        base_url,
        headers,
        entity_set(PROOF_PACKETS_PATH),
        proof_packet_id,
        PATROL_ATTACH_DRAFT,
        &json!({
            "work_cycle_id": string_from_entity(&proof, "work_cycle_id", "WorkCycleId"),
            "worker_run_id": string_from_entity(&proof, "worker_run_id", "WorkerRunId"),
            "review_run_id": string_from_entity(&proof, "review_run_id", "ReviewRunId"),
            "evaluation_run_id": string_from_entity(&proof, "evaluation_run_id", "EvaluationRunId"),
            "summary_markdown": string_from_entity(&proof, "summary_markdown", "SummaryMarkdown"),
            "proof_json": string_from_entity(&proof, "proof_json", "ProofJson"),
            "visual_summary_url": string_from_entity(&proof, "visual_summary_url", "VisualSummaryUrl"),
            "state_diagram_mermaid": string_from_entity(&proof, "state_diagram_mermaid", "StateDiagramMermaid"),
            "changed_files_map": string_from_entity(&proof, "changed_files_map", "ChangedFilesMap"),
            "reviewer_verdict": reviewer_verdict,
            "residual_risks": residual_risks
        }),
    )?;
    Ok(())
}

fn record_e2e_if_present(
    ctx: &Context,
    base_url: &str,
    headers: &[(String, String)],
    work_cycle_id: &str,
    source_kind: &str,
    source_id: &str,
    e2e_summary: &str,
) -> Result<(), String> {
    let evidence = e2e_summary.trim();
    if evidence.is_empty() || work_cycle_id.is_empty() {
        return Ok(());
    }

    let work_cycle = get_entity(
        ctx,
        base_url,
        headers,
        entity_set(WORK_CYCLES_PATH),
        work_cycle_id,
    )?;
    if status_from_response(&work_cycle) != "Reviewing"
        || bool_from_entity(&work_cycle, "e2e_ok", "E2eOk")
    {
        return Ok(());
    }

    post_action(
        ctx,
        base_url,
        headers,
        entity_set(WORK_CYCLES_PATH),
        work_cycle_id,
        PATROL_REPORT_E2E,
        &json!({
            "e2e_summary": format!("{source_kind} {source_id}: {evidence}")
        }),
    )?;
    Ok(())
}

fn mark_proof_ready_if_needed(
    ctx: &Context,
    base_url: &str,
    headers: &[(String, String)],
    proof_packet_id: &str,
) -> Result<(), String> {
    let proof = get_entity(
        ctx,
        base_url,
        headers,
        entity_set(PROOF_PACKETS_PATH),
        proof_packet_id,
    )?;
    if status_from_response(&proof) != "Drafting" {
        return Ok(());
    }

    let summary = final_summary(&proof);
    let proof_json = final_proof_json(&proof);
    let visual_summary_url = final_visual_summary_svg(&proof);
    let state_diagram_mermaid = final_state_diagram_mermaid(&proof);
    let changed_files_map = final_changed_files_map(&proof);

    post_action(
        ctx,
        base_url,
        headers,
        entity_set(PROOF_PACKETS_PATH),
        proof_packet_id,
        PATROL_ATTACH_DRAFT,
        &json!({
            "work_cycle_id": string_from_entity(&proof, "work_cycle_id", "WorkCycleId"),
            "worker_run_id": string_from_entity(&proof, "worker_run_id", "WorkerRunId"),
            "review_run_id": string_from_entity(&proof, "review_run_id", "ReviewRunId"),
            "evaluation_run_id": string_from_entity(&proof, "evaluation_run_id", "EvaluationRunId"),
            "summary_markdown": &summary,
            "proof_json": &proof_json,
            "visual_summary_url": visual_summary_url,
            "state_diagram_mermaid": state_diagram_mermaid,
            "changed_files_map": changed_files_map,
            "reviewer_verdict": "Approved by independent reviewer; automated evaluation passed.",
            "residual_risks": "No residual blockers recorded by review_gate_lifecycle."
        }),
    )?;

    post_action(
        ctx,
        base_url,
        headers,
        entity_set(PROOF_PACKETS_PATH),
        proof_packet_id,
        PATROL_MARK_READY,
        &json!({
            "summary_markdown": summary,
            "proof_json": proof_json
        }),
    )?;
    Ok(())
}

fn reject_proof_if_needed(
    ctx: &Context,
    base_url: &str,
    headers: &[(String, String)],
    proof_packet_id: &str,
    residual_risks: &str,
) -> Result<(), String> {
    if proof_packet_id.is_empty() {
        return Ok(());
    }
    let proof = get_entity(
        ctx,
        base_url,
        headers,
        entity_set(PROOF_PACKETS_PATH),
        proof_packet_id,
    )?;
    if matches!(status_from_response(&proof).as_str(), "Drafting" | "Ready") {
        post_action(
            ctx,
            base_url,
            headers,
            entity_set(PROOF_PACKETS_PATH),
            proof_packet_id,
            PATROL_REJECT,
            &json!({ "residual_risks": residual_risks }),
        )?;
    }
    Ok(())
}

fn fail_work_cycle_and_escalate_case(
    ctx: &Context,
    base_url: &str,
    headers: &[(String, String)],
    work_cycle_id: &str,
    message: &str,
) -> Result<(), String> {
    if work_cycle_id.is_empty() {
        return Ok(());
    }

    let work_cycle = get_entity(
        ctx,
        base_url,
        headers,
        entity_set(WORK_CYCLES_PATH),
        work_cycle_id,
    )?;
    if matches!(
        status_from_response(&work_cycle).as_str(),
        "Planning" | "Planned" | "InProgress" | "Testing" | "Reviewing" | "Proving"
    ) {
        post_action(
            ctx,
            base_url,
            headers,
            entity_set(WORK_CYCLES_PATH),
            work_cycle_id,
            PATROL_FAIL,
            &json!({ "error_message": message }),
        )?;
    }

    escalate_case_if_needed(ctx, base_url, headers, &work_cycle, message)
}

fn begin_case_proof_if_needed(
    ctx: &Context,
    base_url: &str,
    headers: &[(String, String)],
    work_cycle: &Value,
) -> Result<(), String> {
    let case_id = string_from_entity(work_cycle, "factory_case_id", "FactoryCaseId");
    if case_id.is_empty() {
        return Ok(());
    }

    let case = get_entity(
        ctx,
        base_url,
        headers,
        entity_set(FACTORY_CASES_PATH),
        &case_id,
    )?;
    if status_from_response(&case) == "Reviewing" {
        post_action(
            ctx,
            base_url,
            headers,
            entity_set(FACTORY_CASES_PATH),
            &case_id,
            PATROL_BEGIN_PROOF,
            &json!({}),
        )?;
    }
    Ok(())
}

fn complete_case_if_needed(
    ctx: &Context,
    base_url: &str,
    headers: &[(String, String)],
    work_cycle: &Value,
    proof_packet_id: &str,
) -> Result<(), String> {
    let case_id = string_from_entity(work_cycle, "factory_case_id", "FactoryCaseId");
    if case_id.is_empty() {
        return Ok(());
    }

    let case = get_entity(
        ctx,
        base_url,
        headers,
        entity_set(FACTORY_CASES_PATH),
        &case_id,
    )?;
    if matches!(
        status_from_response(&case).as_str(),
        "Reviewing" | "Proving"
    ) {
        post_action(
            ctx,
            base_url,
            headers,
            entity_set(FACTORY_CASES_PATH),
            &case_id,
            PATROL_COMPLETE,
            &json!({
                "summary": format!(
                    "WorkCycle {} completed with ProofPacket {} ready.",
                    entity_id_from_response(work_cycle).unwrap_or_default(),
                    proof_packet_id
                )
            }),
        )?;
    }
    Ok(())
}

fn escalate_case_if_needed(
    ctx: &Context,
    base_url: &str,
    headers: &[(String, String)],
    work_cycle: &Value,
    message: &str,
) -> Result<(), String> {
    let case_id = string_from_entity(work_cycle, "factory_case_id", "FactoryCaseId");
    if case_id.is_empty() {
        return Ok(());
    }
    let case = get_entity(
        ctx,
        base_url,
        headers,
        entity_set(FACTORY_CASES_PATH),
        &case_id,
    )?;
    if matches!(
        status_from_response(&case).as_str(),
        "Triaging" | "Scoped" | "Queued" | "InProgress" | "Reviewing" | "Proving"
    ) {
        post_action(
            ctx,
            base_url,
            headers,
            entity_set(FACTORY_CASES_PATH),
            &case_id,
            PATROL_ESCALATE,
            &json!({ "escalation_reason": message }),
        )?;
    }
    Ok(())
}

fn find_proof_packet(
    ctx: &Context,
    base_url: &str,
    headers: &[(String, String)],
    work_cycle_id: &str,
    evaluation_run_id: &str,
    known_proof_packet_id: &str,
) -> Result<String, String> {
    if !known_proof_packet_id.is_empty() {
        return Ok(known_proof_packet_id.to_string());
    }

    let work_cycle = get_entity(
        ctx,
        base_url,
        headers,
        entity_set(WORK_CYCLES_PATH),
        work_cycle_id,
    )?;
    let proof_from_cycle = string_from_entity(&work_cycle, "proof_packet_id", "ProofPacketId");
    if !proof_from_cycle.is_empty() {
        return Ok(proof_from_cycle);
    }

    if !evaluation_run_id.is_empty() {
        if let Some(proof) = query_first_entity(
            ctx,
            base_url,
            headers,
            entity_set(PROOF_PACKETS_PATH),
            &format!(
                "evaluation_run_id eq '{}'",
                escape_odata_string(evaluation_run_id)
            ),
        )? {
            return Ok(entity_id_from_response(&proof).unwrap_or_default());
        }
    }

    if !work_cycle_id.is_empty() {
        if let Some(proof) = query_first_entity(
            ctx,
            base_url,
            headers,
            entity_set(PROOF_PACKETS_PATH),
            &format!("work_cycle_id eq '{}'", escape_odata_string(work_cycle_id)),
        )? {
            return Ok(entity_id_from_response(&proof).unwrap_or_default());
        }
    }

    Ok(String::new())
}

fn final_summary(proof: &Value) -> String {
    let mut existing = string_from_entity(proof, "summary_markdown", "SummaryMarkdown")
        .replace("# Worker Proof Draft", "# Patrol Proof Ready")
        .replace(
            "Review status: pending independent reviewer.\nEvaluation status: queued.",
            "Review status: approved by independent reviewer.\nEvaluation status: passed.\nProof status: ready.",
        );
    let footer = "\n\nFinal gate: independent review passed, automated evaluation passed, live/E2E evidence was recorded, and the ProofPacket is ready for human-readable review only if the risk lane requires it.\n";
    if existing.contains("Review status: pending independent reviewer.") {
        existing = existing.replace(
            "Review status: pending independent reviewer.",
            "Review status: approved by independent reviewer.",
        );
    }
    if existing.contains("Evaluation status: queued.") {
        existing = existing.replace("Evaluation status: queued.", "Evaluation status: passed.");
    }
    if existing.trim().is_empty() {
        format!("# Patrol Proof Ready\n{footer}")
    } else if existing.contains("Final gate:") {
        existing
    } else {
        format!("{existing}{footer}")
    }
}

fn final_proof_json(proof: &Value) -> String {
    let raw = string_from_entity(proof, "proof_json", "ProofJson");
    let mut value = serde_json::from_str(&raw).unwrap_or_else(|_| json!({}));
    if let Some(object) = value.as_object_mut() {
        object.insert("review_gate".to_string(), json!("passed"));
        object.insert("e2e_gate".to_string(), json!("passed"));
        object.insert("proof_ready".to_string(), json!(true));
        object.insert(
            "human_review_position".to_string(),
            json!("after independent reviewer and evaluator"),
        );
    }
    value.to_string()
}

fn final_state_diagram_mermaid(_proof: &Value) -> String {
    [
        "stateDiagram-v2",
        "  WorkerRun --> ReviewRun: ReportDone created independent reviewer",
        "  ReviewRun --> EvaluationRun: reviewer approved",
        "  EvaluationRun --> WorkCycle: ReportE2e recorded live evidence",
        "  EvaluationRun --> ProofPacket: automated gates passed",
        "  ProofPacket --> WorkCycle: MarkReady + AttachProofPacket",
        "  WorkCycle --> Complete: review + evaluation + E2E + proof satisfied",
    ]
    .join("\n")
}

fn final_changed_files_map(proof: &Value) -> String {
    let raw = string_from_entity(proof, "changed_files_map", "ChangedFilesMap");
    let mut value = serde_json::from_str(&raw).unwrap_or_else(|_| json!({}));
    if let Some(object) = value.as_object_mut() {
        object.insert("review_status".to_string(), json!("approved"));
        object.insert("evaluation_status".to_string(), json!("passed"));
        object.insert("proof_status".to_string(), json!("ready"));
        replace_pending_map_value(
            object,
            "changed_files",
            "reviewed by independent reviewer; see WorkerRun branch/worktree evidence",
        );
        replace_pending_map_value(
            object,
            "dependency_map",
            "automated evaluation passed; see EvaluationRun results_json",
        );
    }
    value.to_string()
}

fn replace_pending_map_value(
    object: &mut serde_json::Map<String, Value>,
    key: &str,
    final_value: &str,
) {
    let needs_replacement = object
        .get(key)
        .map(|value| match value {
            Value::String(value) => value.trim().is_empty() || value.contains("pending"),
            _ => false,
        })
        .unwrap_or(true);
    if needs_replacement {
        object.insert(key.to_string(), json!(final_value));
    }
}

fn final_visual_summary_svg(proof: &Value) -> String {
    let worker_run_id = string_from_entity(proof, "worker_run_id", "WorkerRunId");
    let work_cycle_id = string_from_entity(proof, "work_cycle_id", "WorkCycleId");
    let review_run_id = string_from_entity(proof, "review_run_id", "ReviewRunId");
    let evaluation_run_id = string_from_entity(proof, "evaluation_run_id", "EvaluationRunId");
    let title = if worker_run_id.trim().is_empty() {
        "Patrol proof ready".to_string()
    } else {
        format!("WorkerRun {worker_run_id}")
    };
    let subtitle = if work_cycle_id.trim().is_empty() {
        "Review and evaluation gates passed".to_string()
    } else {
        format!("WorkCycle {work_cycle_id}")
    };
    let svg = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='1200' height='720' viewBox='0 0 1200 720'><rect width='1200' height='720' fill='#f7f4ed'/><rect x='56' y='56' width='1088' height='608' rx='18' fill='#ffffff' stroke='#14532d' stroke-width='3'/><text x='96' y='132' font-family='Inter, Arial, sans-serif' font-size='44' font-weight='700' fill='#111827'>Patrol Proof Ready</text><text x='96' y='190' font-family='Inter, Arial, sans-serif' font-size='24' fill='#374151'>{}</text><text x='96' y='236' font-family='Inter, Arial, sans-serif' font-size='22' fill='#4b5563'>{}</text><g font-family='Inter, Arial, sans-serif' font-size='22' font-weight='700'><rect x='96' y='310' width='210' height='86' rx='12' fill='#dcfce7' stroke='#16a34a'/><text x='130' y='363' fill='#166534'>Worker done</text><rect x='380' y='310' width='230' height='86' rx='12' fill='#dbeafe' stroke='#2563eb'/><text x='414' y='363' fill='#1e3a8a'>Review passed</text><rect x='684' y='310' width='250' height='86' rx='12' fill='#ede9fe' stroke='#7c3aed'/><text x='718' y='363' fill='#4c1d95'>Evaluation passed</text></g><path d='M306 353 L380 353' stroke='#374151' stroke-width='4'/><path d='M610 353 L684 353' stroke='#374151' stroke-width='4'/><text x='96' y='492' font-family='Inter, Arial, sans-serif' font-size='22' fill='#111827'>ReviewRun: {}</text><text x='96' y='536' font-family='Inter, Arial, sans-serif' font-size='22' fill='#111827'>EvaluationRun: {}</text><text x='96' y='590' font-family='Inter, Arial, sans-serif' font-size='20' fill='#4b5563'>Human review is no longer the first line of review; Patrol has proof, review, and evaluation evidence.</text></svg>",
        escape_xml(&title),
        escape_xml(&subtitle),
        escape_xml(if review_run_id.trim().is_empty() {
            "attached"
        } else {
            review_run_id.as_str()
        }),
        escape_xml(if evaluation_run_id.trim().is_empty() {
            "attached"
        } else {
            evaluation_run_id.as_str()
        })
    );
    format!("data:image/svg+xml,{}", percent_encode_svg(&svg))
}

fn requires_human_completion_approval(work_cycle: &Value) -> bool {
    string_from_entity(work_cycle, "risk_lane", "RiskLane").eq_ignore_ascii_case("L3")
        && !bool_from_entity(
            work_cycle,
            "human_completion_approved",
            "HumanCompletionApproved",
        )
}

fn reviewer_verdict(verdict: &str, review_summary: &str, live_e2e_summary: &str) -> String {
    let mut parts = Vec::new();
    if !verdict.trim().is_empty() {
        parts.push(format!("Verdict: {verdict}"));
    }
    if !review_summary.trim().is_empty() {
        parts.push(format!("Review: {review_summary}"));
    }
    if !live_e2e_summary.trim().is_empty() {
        parts.push(format!("Live/E2E: {live_e2e_summary}"));
    }
    if parts.is_empty() {
        "Reviewer did not provide a detailed verdict.".to_string()
    } else {
        parts.join("\n")
    }
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

fn string_from_entity(entity: &Value, snake: &str, pascal: &str) -> String {
    entity
        .get(snake)
        .and_then(Value::as_str)
        .or_else(|| entity.get(pascal).and_then(Value::as_str))
        .or_else(|| {
            entity
                .pointer(&format!("/fields/{snake}"))
                .and_then(Value::as_str)
        })
        .or_else(|| {
            entity
                .pointer(&format!("/fields/{pascal}"))
                .and_then(Value::as_str)
        })
        .unwrap_or("")
        .to_string()
}

fn bool_from_entity(entity: &Value, snake: &str, pascal: &str) -> bool {
    let value = entity
        .get(snake)
        .or_else(|| entity.get(pascal))
        .or_else(|| entity.pointer(&format!("/fields/{snake}")))
        .or_else(|| entity.pointer(&format!("/fields/{pascal}")));
    match value {
        Some(Value::Bool(flag)) => *flag,
        Some(Value::String(flag)) => flag == "true",
        _ => false,
    }
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
    let entity = get_entity(ctx, base_url, headers, entity_set, entity_id)?;
    Ok(status_from_response(&entity))
}

fn wait_for_entity_status(
    ctx: &Context,
    base_url: &str,
    headers: &[(String, String)],
    entity_set: &str,
    entity_id: &str,
    expected_status: &str,
    acceptable_later_statuses: &[&str],
) -> Result<Value, String> {
    let mut last_entity = json!({});
    let mut last_status = String::new();
    for _ in 0..8 {
        last_entity = get_entity(ctx, base_url, headers, entity_set, entity_id)?;
        last_status = status_from_response(&last_entity);
        if last_status == expected_status
            || acceptable_later_statuses
                .iter()
                .any(|status| *status == last_status)
        {
            return Ok(last_entity);
        }
    }

    Err(format!(
        "review_gate_lifecycle: {entity_set} {entity_id} did not reach {expected_status}; last observed status was {last_status}"
    ))
}

fn wait_for_bool(
    ctx: &Context,
    base_url: &str,
    headers: &[(String, String)],
    entity_set: &str,
    entity_id: &str,
    snake: &str,
    pascal: &str,
) -> Result<Value, String> {
    let mut last_entity = json!({});
    for _ in 0..8 {
        last_entity = get_entity(ctx, base_url, headers, entity_set, entity_id)?;
        if bool_from_entity(&last_entity, snake, pascal) {
            return Ok(last_entity);
        }
    }

    Err(format!(
        "review_gate_lifecycle: {entity_set} {entity_id} did not set {snake}"
    ))
}

fn get_entity(
    ctx: &Context,
    base_url: &str,
    headers: &[(String, String)],
    entity_set: &str,
    entity_id: &str,
) -> Result<Value, String> {
    let url = format!("{base_url}/tdata/{entity_set}('{entity_id}')");
    let resp = ctx.http_call("GET", &url, headers, "")?;
    parse_json_response(resp, &format!("get {entity_set}('{entity_id}')"))
}

fn query_first_entity(
    ctx: &Context,
    base_url: &str,
    headers: &[(String, String)],
    entity_set: &str,
    filter: &str,
) -> Result<Option<Value>, String> {
    let url = format!(
        "{base_url}/tdata/{entity_set}?$filter={}&$top=1",
        urlencoded(filter)
    );
    let resp = ctx.http_call("GET", &url, headers, "")?;
    let body = parse_json_response(resp, &format!("query {entity_set}"))?;
    Ok(body
        .get("value")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .cloned())
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

fn escape_odata_string(input: &str) -> String {
    input.replace('\'', "''")
}

fn urlencoded(input: &str) -> String {
    let mut encoded = String::new();
    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char)
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn percent_encode_svg(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char)
            }
            b' ' => encoded.push_str("%20"),
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

fn truncate(input: &str, max: usize) -> String {
    if input.len() <= max {
        input.to_string()
    } else {
        format!("{}[truncated]", input.chars().take(max).collect::<String>())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn final_proof_summary_removes_stale_draft_gate_text() {
        let proof = json!({
            "fields": {
                "summary_markdown": "# Worker Proof Draft\n\nReview status: pending independent reviewer.\nEvaluation status: queued.\n"
            }
        });

        let summary = final_summary(&proof);

        assert!(summary.contains("# Patrol Proof Ready"));
        assert!(summary.contains("Review status: approved by independent reviewer."));
        assert!(summary.contains("Evaluation status: passed."));
        assert!(summary.contains("Final gate: independent review passed"));
        assert!(!summary.contains("pending independent reviewer"));
        assert!(!summary.contains("queued."));
    }

    #[test]
    fn final_proof_visual_and_map_report_ready_gates() {
        let proof = json!({
            "fields": {
                "work_cycle_id": "wc-1",
                "worker_run_id": "wr-1",
                "review_run_id": "rr-1",
                "evaluation_run_id": "ev-1",
                "changed_files_map": "{\"branch_name\":\"codex/test\"}",
                "proof_json": "{\"worker_run_id\":\"wr-1\"}"
            }
        });

        let visual = final_visual_summary_svg(&proof);
        let changed_map = final_changed_files_map(&proof);
        let proof_json = final_proof_json(&proof);

        assert!(visual.starts_with("data:image/svg+xml,"));
        assert!(visual.contains("Patrol%20Proof%20Ready"));
        assert!(visual.contains("Review%20passed"));
        assert!(visual.contains("Evaluation%20passed"));
        assert!(changed_map.contains("\"review_status\":\"approved\""));
        assert!(changed_map.contains("\"evaluation_status\":\"passed\""));
        assert!(!changed_map.contains("pending"));
        assert!(proof_json.contains("\"proof_ready\":true"));
    }

    #[test]
    fn final_changed_files_map_preserves_actual_file_lists() {
        let proof = json!({
            "fields": {
                "changed_files_map": "{\"branch_name\":\"codex/test\",\"changed_files\":[\"crates/temperpaw/src/discord.rs\"],\"dependency_map\":{\"crate\":\"temperpaw\"}}"
            }
        });

        let changed_map: Value = serde_json::from_str(&final_changed_files_map(&proof))
            .expect("changed map should stay valid json");

        assert_eq!(
            changed_map["changed_files"],
            json!(["crates/temperpaw/src/discord.rs"])
        );
        assert_eq!(changed_map["dependency_map"], json!({"crate": "temperpaw"}));
        assert_eq!(changed_map["proof_status"], "ready");
    }
}
