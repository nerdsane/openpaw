permit(
  principal is Admin,
  action,
  resource
) when {
  resource is PatrolRequest ||
  resource is WorkRequest ||
  resource is Intent ||
  resource is Signal ||
  resource is FactoryCase ||
  resource is WorkCycle ||
  resource is Effort ||
  resource is WorkerRun ||
  resource is WorkerProvider ||
  resource is WorkerAgent ||
  resource is ReviewRun ||
  resource is EvaluationRun ||
  resource is ProofPacket ||
  resource is RiskRule ||
  resource is PatrolRun ||
  resource is RepoGraphSnapshot ||
  resource is QualityFinding ||
  resource is SecurityFinding ||
  resource is ObservabilityFinding ||
  resource is DailyBrief ||
  resource is PatrolSchedule ||
  resource is Adjudication ||
  resource is StandingDecision ||
  resource is ShadowVerdict
};

permit(
  principal,
  action in [Action::"read", Action::"list"],
  resource
) when {
  resource is PatrolRequest ||
  resource is WorkRequest ||
  resource is Intent ||
  resource is Signal ||
  resource is FactoryCase ||
  resource is WorkCycle ||
  resource is Effort ||
  resource is WorkerRun ||
  resource is WorkerProvider ||
  resource is WorkerAgent ||
  resource is ReviewRun ||
  resource is EvaluationRun ||
  resource is ProofPacket ||
  resource is RiskRule ||
  resource is PatrolRun ||
  resource is RepoGraphSnapshot ||
  resource is QualityFinding ||
  resource is SecurityFinding ||
  resource is ObservabilityFinding ||
  resource is DailyBrief ||
  resource is PatrolSchedule ||
  resource is Adjudication ||
  resource is StandingDecision ||
  resource is ShadowVerdict
};

permit(
  principal is Agent,
  action == Action::"http_call",
  resource is HttpEndpoint
) when {
  ["patrol_request_router", "signal_router", "worker_run_lifecycle", "work_cycle_lifecycle", "effort_lifecycle", "finding_lifecycle", "review_gate_lifecycle", "repo_sweep_lifecycle", "daily_brief_lifecycle", "patrol_schedule_lifecycle", "patrol_run_lifecycle"].contains(context.module)
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
  resource is PatrolRequest || resource is WorkRequest || resource is Signal || resource is FactoryCase
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
    Action::"RevisePlan",
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
    Action::"Register",
    Action::"ReportHeartbeat",
    Action::"MarkUnhealthy",
    Action::"Disable",
    Action::"RecordEvidence",
    Action::"AttachEvidenceLinks",
    Action::"Pause",
    Action::"Resume",
    Action::"Recover",
    Action::"Expire"
  ],
  resource
) when {
  principal.agent_type == "system" &&
  (
    resource is PatrolRequest ||
    resource is WorkRequest ||
    resource is Intent ||
    resource is Signal ||
    resource is FactoryCase ||
    resource is WorkCycle ||
    resource is WorkerRun ||
    resource is WorkerProvider ||
    resource is WorkerAgent ||
    resource is ReviewRun ||
    resource is EvaluationRun ||
    resource is ProofPacket ||
    resource is PatrolRun ||
    resource is RepoGraphSnapshot ||
    resource is QualityFinding ||
    resource is SecurityFinding ||
    resource is ObservabilityFinding ||
    resource is DailyBrief ||
    resource is PatrolSchedule
  )
  // NB: Effort is intentionally NOT in this generic system-agent list. Effort
  // lifecycle has its own permit below, and Effort creation is the intake
  // service's ALONE - granting the generic `create` here was a second birth door
  // (#491 panel round 2). The forbid below is the belt-and-suspenders guarantee.
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
  resource is QualityFinding || resource is SecurityFinding || resource is ObservabilityFinding
};

permit(
  principal is Agent,
  action in [
    Action::"create",
    Action::"Register",
    Action::"Activate",
    Action::"ReportHeartbeat",
    Action::"MarkUnhealthy",
    Action::"Disable"
  ],
  resource
) when {
  resource is WorkerProvider || resource is WorkerAgent
};

permit(
  principal is Agent,
  action in [
    Action::"create",
    Action::"Configure",
    Action::"Start",
    Action::"AttachWorkerRun",
    Action::"RecordEvidence",
    Action::"AttachEvidenceLinks",
    Action::"Complete",
    Action::"Escalate",
    Action::"Fail"
  ],
  resource is PatrolRun
);

// The registered Mac mini worker performs Datadog evidence collection and
// writes only Patrol entities needed to make that evidence inspectable. It may
// also queue low-risk Datadog follow-up WorkCycles; higher-risk code or deploy
// work still pauses at the human approval gate.
permit(
  principal is Agent,
  action in [
    Action::"create",
    Action::"Normalize",
    Action::"Triage",
    Action::"AttachCase",
    Action::"OpenFinding",
    Action::"Open",
    Action::"SetRiskFloor",
    Action::"Configure",
    Action::"LinkSource",
    Action::"WritePlan",
    Action::"StartWork",
    Action::"RequestHumanStartApproval",
    Action::"OpenWorkCycle",
    Action::"AttachWorkerRun",
    Action::"AttachDraft",
    Action::"MarkReady"
  ],
  resource
) when {
  principal.agent_type == "worker" &&
  principal.id == "mac-mini-codex-prod" &&
  (
    resource is Signal ||
    resource is ObservabilityFinding ||
    resource is FactoryCase ||
    resource is WorkCycle ||
    resource is WorkerRun ||
    resource is ProofPacket
  )
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
    Action::"Recover",
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
    Action::"AttachWorkerRun",
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
    Action::"ReportFailed",
    Action::"Heartbeat"
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

// ReleaseRun: the governed release of a completed WorkCycle through the
// computer (merge → watch rollout → git-revert rollback).
//
// The release surface is NOT open to ordinary agents (a merge to main is a
// production change gated by the WorkCycle's review/proof/approval loop). It
// is created and driven by the `patrol-release-service` principal, which is
// the elevation the declarative WorkCycle.Complete → ReleaseRun.Request
// trigger runs under. Callbacks + the timer-fired Check are dispatched by the
// release_run_lifecycle WASM / kernel timer as a service (patrol-release-service
// or the system runtime). read/list stay open within the tenant.
permit(
  principal is Admin,
  action,
  resource is ReleaseRun
);

permit(
  principal,
  action in [Action::"read", Action::"list"],
  resource is ReleaseRun
);

// Create the run and fire Request — ONLY the release service (via the
// Complete trigger's principal elevation). No ordinary-Agent grant.
permit(
  principal,
  action in [
    Action::"create",
    Action::"ReleaseRun.Request", Action::"Request"
  ],
  resource is ReleaseRun
) when {
  principal.agent_type == "patrol-release-service"
};

// Callbacks (dispatched by the WASM) + the timer-fired Check + Fail. These run
// as a service principal — either the inherited release principal or the
// system runtime that fires state_timeouts — never as an ordinary agent.
permit(
  principal,
  action in [
    Action::"ReleaseRun.Check", Action::"Check",
    Action::"ReleaseRun.MergeSucceeded", Action::"MergeSucceeded",
    Action::"ReleaseRun.CheckPending", Action::"CheckPending",
    Action::"ReleaseRun.CheckHealthy", Action::"CheckHealthy",
    Action::"ReleaseRun.CheckUnhealthy", Action::"CheckUnhealthy",
    Action::"ReleaseRun.RollbackPushed", Action::"RollbackPushed",
    Action::"ReleaseRun.Fail", Action::"Fail"
  ],
  resource is ReleaseRun
) when {
  principal.agent_type == "system" ||
  principal.agent_type == "patrol-release-service"
};

// WorkCycle release configuration — restricted to the system/patrol surface,
// a supervisor, or an admin (not any agent). Naming a production release
// target is part of the governed loop, not an open agent action.
permit(
  principal,
  action in [
    Action::"WorkCycle.ConfigureRelease", Action::"ConfigureRelease"
  ],
  resource is WorkCycle
) when {
  principal is Admin ||
  (principal is Agent && (
    principal.agent_type == "system" ||
    principal.agent_type == "patrol-release-service" ||
    principal.agent_type == "supervisor"
  ))
};

// release_run_lifecycle needs HTTP (Computer row loopback, sandbox data
// plane, health_url probe) and secrets (temper_api_url, sandbox key). Gated
// purely on the module identity — the side-effect surface is fixed by the
// WASM, and the module runs as a service principal (not necessarily `is Agent`).
permit(
  principal,
  action == Action::"http_call",
  resource is HttpEndpoint
) when {
  context.module == "release_run_lifecycle"
};

permit(
  principal,
  action == Action::"access_secret",
  resource is Secret
) when {
  context.module == "release_run_lifecycle"
};

// ARN-434: the S1 shadow sweep runs as the verified operator credential and must
// write the S0 records and the ShadowVerdict rows. Narrowly scoped - exactly the
// actions the sweep dispatches, on exactly those resources, gated on the same
// verified-operator predicate the kernel uses for operator authority
// (agent_type == "operator" && agentTypeVerified == true, so a self-declared
// header agent can never match). Nothing else is broadened.
permit(
  principal is Agent,
  action in [Action::"Ingest", Action::"IngestRecord"],
  resource is ReviewRun
) when {
  principal has agent_type &&
  principal.agent_type == "operator" &&
  principal has agentTypeVerified &&
  principal.agentTypeVerified == true
};

permit(
  principal is Agent,
  action in [Action::"Ingest", Action::"IngestProof"],
  resource is ProofPacket
) when {
  principal has agent_type &&
  principal.agent_type == "operator" &&
  principal has agentTypeVerified &&
  principal.agentTypeVerified == true
};

permit(
  principal is Agent,
  action in [Action::"Record", Action::"MarkAgree", Action::"MarkDisagree"],
  resource is ShadowVerdict
) when {
  principal has agent_type &&
  principal.agent_type == "operator" &&
  principal has agentTypeVerified &&
  principal.agentTypeVerified == true
};

// ============================ ARN-441: the entity loop ========================
// Intent (WorkRequest renamed) + Effort (WorkCycle EXTENDED). Additive in shadow;
// WorkRequest/WorkCycle policies above stay live until the phase-3 flip. Intent/
// Effort are also on the Admin blanket + read/list blanket above.

// Intent intake - open to agents, like the WorkRequest manager surface.
permit(
  principal is Agent,
  action in [
    Action::"create",
    Action::"Submit",
    Action::"AttachSignal",
    Action::"Triage",
    Action::"Accept",
    Action::"LinkPmIssue",
    Action::"Reject"
  ],
  resource is Intent
);

// RebirthEffort re-attempts the Effort birth, so it is the intake service's ALONE
// (#491 panel round 2) - not any Agent. Same principal as the birth it repeats;
// the Intent's declarative RebirthEffort trigger already runs under this elevation.
permit(
  principal,
  action == Action::"RebirthEffort",
  resource is Intent
) when {
  principal has agent_type && principal.agent_type == "patrol-intake-service"
};

// Effort lifecycle - the full set of SDLC transitions, driven by the
// system/patrol surface. Self-contained (does not lean on the system-agent block
// above), so the whole Effort authority reads in one place.
permit(
  principal,
  // Lifecycle transitions only - NOT Seed/create (birth is the intake service's
  // alone; see the birth permit below). Splitting them keeps the birth rule intact:
  // an Effort is born only by accepting an Intent, never by a lifecycle driver.
  action in [
    Action::"Specify",
    Action::"Plan",
    Action::"StartBuild",
    Action::"RejectSpec",
    Action::"RejectPlan",
    Action::"AttachWorkerRun",
    Action::"WorkerDone",
    Action::"SubmitForReview",
    Action::"AttachReviewRun",
    Action::"ReportE2e",
    Action::"PassReview",
    Action::"RequestChanges",
    Action::"PassEvaluation",
    Action::"AttachProofPacket",
    Action::"Merge",
    Action::"Deploy",
    Action::"MarkDeployVerified",
    Action::"Verify",
    Action::"Heartbeat",
    Action::"Stall",
    Action::"Resume",
    Action::"Abandon"
  ],
  resource is Effort
) when {
  principal is Admin ||
  // `principal has agent_type` before every == : app Cedar is schema-less, so a
  // principal missing the attribute must be denied explicitly, not error.
  (principal is Agent && principal has agent_type && (
    principal.agent_type == "system" ||
    principal.agent_type == "supervisor" ||
    // The verified operator drives the SHADOW Effort lifecycle - the same
    // trusted sweep/verification authority ARN-434 grants the S0/S1 record
    // writes (agentTypeVerified, so a self-declared header agent never matches).
    // The per-gate authority flips (steps 3-4) put stricter permits on
    // Merge/Deploy; this operator surface tightens there.
    (principal.agent_type == "operator" &&
     principal has agentTypeVerified && principal.agentTypeVerified == true)
  ))
};

// Effort BIRTH - create + Seed ONLY via the intake service (the elevation the
// declarative Intent.Accept -> Effort.Seed trigger runs under). No ordinary-Agent
// create grant, mirroring the ReleaseRun / patrol-release-service pattern: an
// Effort is born by accepting an Intent, never conjured directly.
permit(
  principal,
  // Only `Seed` (the bare action name the dispatch uses - confirmed in the live
  // drive log: temper.action "Seed", agent_type "patrol-intake-service"; the
  // namespaced Effort.Seed hedge is dropped) plus `create`.
  action in [
    Action::"create",
    Action::"Seed"
  ],
  resource is Effort
) when {
  principal has agent_type && principal.agent_type == "patrol-intake-service"
};

// Birth rule, enforced (#491 panel round 2): forbid beats permit, so this closes
// EVERY door to Effort creation except the intake service - the Admin blanket, the
// generic system-agent permit, a future stray permit, all of them. An Effort is
// born ONLY by accepting an Intent (the Accept trigger runs as patrol-intake-service).
forbid(
  principal,
  action in [Action::"create", Action::"Seed"],
  resource is Effort
) unless {
  principal has agent_type && principal.agent_type == "patrol-intake-service"
};
