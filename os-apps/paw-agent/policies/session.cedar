// Session — Cedar Authorization Policies
//
// Controls who can create, configure, and interact with spec-driven sessions.
// Callback actions are permitted for system agents to enable the dispatch pipeline loop.
// Steering is permitted for supervisors, humans, and parent sessions.

// --- Creation and Configuration: admins, supervisors, humans, and runtime agents ---

permit(
  principal is Admin,
  action,
  resource is Session
);

permit(
  principal,
  action in [Action::"create"],
  resource is Session
) when {
  ["supervisor", "human", "system", "agent"].contains(principal.agent_type)
};

permit(
  principal,
  action in [Action::"Configure", Action::"Provision", Action::"ProvisionWorkspace", Action::"Resume"],
  resource is Session
) when {
  ["supervisor", "human", "system"].contains(principal.agent_type) ||
  principal == resource ||
  context.parent_session_id == principal.id
};

// --- Read and List: any authenticated agent ---

permit(
  principal,
  action in [Action::"read", Action::"list"],
  resource is Session
);

permit(
  principal,
  action in [Action::"read_specs"],
  resource is Spec
);

// --- Steering: supervisors, humans, system, and self (session acting on itself) ---

permit(
  principal,
  action in [Action::"Steer", Action::"SwitchProvider", Action::"SwitchMode"],
  resource is Session
) when {
  ["supervisor", "human", "system"].contains(principal.agent_type) ||
  principal == resource ||
  context.parent_session_id == principal.id
};

// --- Dispatch pipeline callbacks: system agents ---
// These actions are triggered by WASM integration callbacks, not by users.

permit(
  principal,
  action in [
    Action::"SandboxReady",
    Action::"ProvisionPending",
    Action::"CheckSandboxReady",
    Action::"WorkspaceReady",
    Action::"ContextReady",
    Action::"ProviderResponseReady",
    Action::"ProcessToolCalls",
    Action::"HandleToolResults",
    Action::"CheckpointToolBatch",
    Action::"RecordResult",
    Action::"NeedsCompaction",
    Action::"CompactionComplete",
    Action::"CheckSteering",
    Action::"ContinueWithSteering",
    Action::"FinalizeResult",
    Action::"DeliveryFailed",
    Action::"Heartbeat",
    Action::"TimeoutFail",
    Action::"PauseForApproval",
    Action::"PauseForPlanApproval",
    Action::"RecoverFromRestart",
    Action::"RecoveryComplete"
  ],
  resource is Session
) when {
  principal.agent_type == "system" ||
  principal == resource
};

// --- Cedar approval resume: any privileged principal ---
// Dispatched by Discord interaction handler after human clicks Approve.
permit(
  principal,
  action in [Action::"ResumeAfterApproval", Action::"ResumeWithPlanApproval", Action::"ResumeWithPlanChanges"],
  resource is Session
) when {
  ["supervisor", "human", "system", "admin"].contains(principal.agent_type)
};

// --- WASM module HTTP call authorization ---

permit(
  principal is Agent,
  action == Action::"http_call",
  resource is HttpEndpoint
) when {
  ["agent_reply", "sandbox_provisioner", "workspace_provisioner", "llm_caller", "context_preparer", "provider_caller", "provider_response_applier", "tool_runner", "monty_repl", "capability_installer", "workspace_restorer", "context_compactor", "steering_checker", "heartbeat_scan", "heartbeat_scheduler", "cron_scheduler_check", "cron_scheduler_heartbeat", "request_approval", "request_plan_review", "plan_approval_handler", "plan_review_feedback_handler", "session_recoverer"].contains(context.module)
};

// Allow agent WASM modules to access secrets
permit(
  principal is Agent,
  action == Action::"access_secret",
  resource is Secret
) when {
  ["sandbox_provisioner", "workspace_provisioner", "llm_caller", "provider_caller", "tool_runner", "monty_repl", "capability_installer", "workspace_restorer", "context_compactor", "session_recoverer"].contains(context.module)
};

// --- Failure and cancellation: admins, system callbacks, or the owning parent/self ---

permit(
  principal,
  action in [Action::"Fail", Action::"Cancel"],
  resource is Session
) when {
  principal.agent_type == "system" ||
  context.parent_session_id == principal.id ||
  principal == resource
};
