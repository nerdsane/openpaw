// Foresight — Cedar Authorization Policies
//
// Broad permits matching the paw-heal pattern. All principals can perform
// standard CRUD and actions on Foresight entities.

// --- ForesightModel ---
permit(
  principal,
  action in [Action::"create", Action::"read", Action::"list",
             Action::"Seed", Action::"SeedComplete", Action::"SeedFailed",
             Action::"RefreshSignals", Action::"RefreshComplete",
             Action::"MarkStale", Action::"Reactivate"],
  resource is ForesightModel
);

// --- Projection ---
permit(
  principal,
  action in [Action::"create", Action::"read", Action::"list",
             Action::"Configure", Action::"Start", Action::"ProbesReady",
             Action::"ProbeStepDone", Action::"ConvergenceComplete",
             Action::"ProjectionUpdated",
             Action::"AdvanceStep", Action::"Complete", Action::"Branch", Action::"Fail"],
  resource is Projection
);

// --- Observation ---
permit(
  principal,
  action in [Action::"create", Action::"read", Action::"list",
             Action::"Record", Action::"Confirm",
             Action::"Escalate", Action::"Fade"],
  resource is Observation
);

// --- Direction ---
permit(
  principal,
  action in [Action::"create", Action::"read", Action::"list",
             Action::"Propose", Action::"SubmitForReview",
             Action::"RecordReview", Action::"RecordConfirmation",
             Action::"MarkCrossModelAgreement",
             Action::"BeginImplementation", Action::"ImplementationComplete",
             Action::"Select", Action::"Archive"],
  resource is Direction
);

// --- DirectionFeedback ---
permit(
  principal,
  action in [Action::"create", Action::"read", Action::"list",
             Action::"Submit", Action::"Address",
             Action::"Resolve", Action::"Reopen", Action::"ReAddress"],
  resource is DirectionFeedback
);

// --- Agent session → child session management ---
// The foresight orchestrator (an agent session) must be able to create and
// configure child probe sessions. The default session.cedar policy restricts
// Configure to supervisor/human/system or parent sessions. This permit
// allows any agent to configure Sessions, enabling the orchestrator pattern.
permit(
  principal,
  action in [Action::"create", Action::"Configure"],
  resource is Session
) when {
  principal.agent_type == "agent"
};

// --- WASM module HTTP + secret access ---
permit(
  principal is Agent,
  action == Action::"http_call",
  resource is HttpEndpoint
) when {
  ["spawn_seed_agent", "spawn_orchestrator", "spawn_probes", "handle_probe_done", "handle_convergence", "handle_projection_updated"].contains(context.module)
};

permit(
  principal is Agent,
  action == Action::"access_secret",
  resource is Secret
) when {
  ["spawn_seed_agent", "spawn_orchestrator", "spawn_probes", "handle_probe_done", "handle_convergence", "handle_projection_updated"].contains(context.module)
};
