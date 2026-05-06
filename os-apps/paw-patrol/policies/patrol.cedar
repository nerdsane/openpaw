permit(
  principal is Admin,
  action,
  resource
) when {
  resource is PatrolRequest ||
  resource is Signal ||
  resource is FactoryCase ||
  resource is WorkCycle ||
  resource is WorkerRun ||
  resource is ReviewRun ||
  resource is EvaluationRun ||
  resource is ProofPacket ||
  resource is RiskRule ||
  resource is RepoGraphSnapshot ||
  resource is QualityFinding ||
  resource is SecurityFinding ||
  resource is DailyBrief ||
  resource is PatrolSchedule
};

permit(
  principal,
  action in [Action::"read", Action::"list"],
  resource
) when {
  resource is PatrolRequest ||
  resource is Signal ||
  resource is FactoryCase ||
  resource is WorkCycle ||
  resource is WorkerRun ||
  resource is ReviewRun ||
  resource is EvaluationRun ||
  resource is ProofPacket ||
  resource is RiskRule ||
  resource is RepoGraphSnapshot ||
  resource is QualityFinding ||
  resource is SecurityFinding ||
  resource is DailyBrief ||
  resource is PatrolSchedule
};

permit(
  principal is Agent,
  action == Action::"http_call",
  resource is HttpEndpoint
) when {
  ["patrol_request_router", "signal_router", "worker_run_lifecycle", "work_cycle_lifecycle", "finding_lifecycle", "review_gate_lifecycle", "repo_sweep_lifecycle", "daily_brief_lifecycle", "patrol_schedule_lifecycle"].contains(context.module)
};

// Manager surfaces such as OpenClaw may submit requests and route approvals, but
// v1 denies code-writing and repo mutation outside the local Codex worker.
permit(
  principal is Agent,
  action in [
    Action::"create",
    Action::"Submit",
    Action::"Ingest",
    Action::"Normalize",
    Action::"Triage",
    Action::"AttachSignal",
    Action::"AttachCase",
    Action::"Archive",
    Action::"AcceptAsCase",
    Action::"LinkPmIssue",
    Action::"Reject",
    Action::"RouteFailed"
  ],
  resource
) when {
  resource is PatrolRequest || resource is Signal || resource is FactoryCase
};

// Patrol's own WASM integrations perform intake routing after the trigger
// boundary. These actions still appear as entity transitions; the policy only
// grants the registered system agent surface enough room to create the case,
// PM link, work cycle, and queued local worker run.
permit(
  principal,
  action in [
    Action::"create",
    Action::"read",
    Action::"list",
    Action::"Triage",
    Action::"AcceptAsCase",
    Action::"LinkPmIssue",
    Action::"RouteFailed",
    Action::"Normalize",
    Action::"AttachCase",
    Action::"Archive",
    Action::"Open",
    Action::"SetRiskFloor",
    Action::"RaiseRisk",
    Action::"OpenWorkCycle",
    Action::"QueueWork",
    Action::"Configure",
    Action::"LinkSource",
    Action::"WritePlan",
    Action::"RequestHumanStartApproval",
    Action::"StartWork",
    Action::"AttachWorkerRun",
    Action::"BeginWork",
    Action::"BeginReview",
    Action::"WorkerDone",
    Action::"SubmitForReview",
    Action::"AttachReviewRun",
    Action::"AttachEvaluationRun",
    Action::"ReportE2e",
    Action::"PassReview",
    Action::"RequestChanges",
    Action::"PassEvaluation",
    Action::"AttachProofPacket",
    Action::"RequestHumanCompletionApproval",
    Action::"Complete",
    Action::"Fail",
    Action::"Escalate",
    Action::"Request",
    Action::"Queue",
    Action::"AttachDraft",
    Action::"MarkReady",
    Action::"Reject",
    Action::"BeginProof",
    Action::"StartScan",
    Action::"ScanComplete",
    Action::"AttachAssessmentSession",
    Action::"AssessmentComplete",
    Action::"ScanFailed",
    Action::"OpenFinding",
    Action::"Accept",
    Action::"Resolve",
    Action::"Start",
    Action::"AttachSession",
    Action::"Render",
    Action::"Publish",
    Action::"Activate",
    Action::"ActivateComplete",
    Action::"Trigger",
    Action::"TriggerComplete",
    Action::"Pause",
    Action::"Resume",
    Action::"Expire"
  ],
  resource
) when {
  principal.agent_type == "system" &&
  (
    resource is PatrolRequest ||
    resource is Signal ||
    resource is FactoryCase ||
    resource is WorkCycle ||
    resource is WorkerRun ||
    resource is ReviewRun ||
    resource is EvaluationRun ||
    resource is ProofPacket ||
    resource is RepoGraphSnapshot ||
    resource is QualityFinding ||
    resource is SecurityFinding ||
    resource is DailyBrief ||
    resource is PatrolSchedule
  )
};

permit(
  principal is Agent,
  action in [
    Action::"Accept",
    Action::"Waive",
    Action::"Resolve",
    Action::"Escalate"
  ],
  resource
) when {
  resource is QualityFinding || resource is SecurityFinding
};

permit(
  principal is Agent,
  action in [
    Action::"ApproveHumanStart",
    Action::"ApproveHumanCompletion"
  ],
  resource is WorkCycle
) when {
  ["supervisor", "human"].contains(principal.agent_type)
};

permit(
  principal is Agent,
  action in [
    Action::"create",
    Action::"Configure",
    Action::"Activate",
    Action::"ActivateComplete",
    Action::"Trigger",
    Action::"TriggerComplete",
    Action::"Pause",
    Action::"Resume",
    Action::"Expire",
    Action::"Fail"
  ],
  resource is PatrolSchedule
);

permit(
  principal is Agent,
  action in [
    Action::"create",
    Action::"Start",
    Action::"AttachSession",
    Action::"Render",
    Action::"Publish",
    Action::"Fail"
  ],
  resource is DailyBrief
);

permit(
  principal is Agent,
  action in [
    Action::"StartScan",
    Action::"ScanComplete",
    Action::"AttachAssessmentSession",
    Action::"AssessmentComplete",
    Action::"ScanFailed"
  ],
  resource is RepoGraphSnapshot
);

permit(
  principal is Agent,
  action in [
    Action::"Claim"
  ],
  resource is WorkerRun
) when {
  principal.agent_type == "system" ||
  (
    principal.agent_type == "worker" &&
    (
      (resource has AllowedWorkerId && principal.id == resource.AllowedWorkerId) ||
      (resource has allowed_worker_id && principal.id == resource.allowed_worker_id)
    )
  )
};

permit(
  principal is Agent,
  action in [
    Action::"StartLocal",
    Action::"ReportDone",
    Action::"ReportFailed"
  ],
  resource is WorkerRun
) when {
  principal.agent_type == "system" ||
  (
    principal.agent_type == "worker" &&
    (
      (resource has WorkerId && principal.id == resource.WorkerId) ||
      (resource has worker_id && principal.id == resource.worker_id)
    )
  )
};

permit(
  principal is Agent,
  action in [
    Action::"Request",
    Action::"Claim"
  ],
  resource is ReviewRun
) when {
  ["agent", "worker", "system", "supervisor", "human"].contains(principal.agent_type)
};

permit(
  principal is Agent,
  action in [
    Action::"StartReview",
    Action::"RequestChanges",
    Action::"Approve",
    Action::"Escalate",
    Action::"Fail"
  ],
  resource is ReviewRun
) when {
  principal.agent_type == "system" ||
  ["supervisor", "human"].contains(principal.agent_type) ||
  (
    ["agent", "worker"].contains(principal.agent_type) &&
    (
      (resource has ReviewerId && principal.id == resource.ReviewerId) ||
      (resource has reviewer_id && principal.id == resource.reviewer_id)
    )
  )
};

permit(
  principal is Agent,
  action in [
    Action::"Claim"
  ],
  resource is EvaluationRun
) when {
  principal.agent_type == "worker" &&
  (
    (!(resource has EvaluatorId) && !(resource has evaluator_id)) ||
    (resource has EvaluatorId && (resource.EvaluatorId == "" || principal.id == resource.EvaluatorId)) ||
    (resource has evaluator_id && (resource.evaluator_id == "" || principal.id == resource.evaluator_id))
  )
};

permit(
  principal is Agent,
  action in [
    Action::"Start",
    Action::"Pass",
    Action::"Fail"
  ],
  resource is EvaluationRun
) when {
  principal.agent_type == "worker" &&
  (
    (resource has EvaluatorId && principal.id == resource.EvaluatorId) ||
    (resource has evaluator_id && principal.id == resource.evaluator_id)
  )
};
