// Foresight — Cedar Authorization Policies
//
// Controls access to the paw-foresight entities: ProductModel, Projection,
// Observation, Direction, and DirectionFeedback.
//
// Roles:
//   system  — WASM integration callbacks and platform automation
//   probe   — AI agents that observe projections and propose directions
//   human   — Human operators who select/archive directions
//   admin   — Full access

// ============================================================
// System agents: full access to all Foresight entities
// ============================================================

permit(
  principal,
  action,
  resource is ProductModel
) when {
  principal.agent_type == "system"
};

permit(
  principal,
  action,
  resource is Projection
) when {
  principal.agent_type == "system"
};

permit(
  principal,
  action,
  resource is Observation
) when {
  principal.agent_type == "system"
};

permit(
  principal,
  action,
  resource is Direction
) when {
  principal.agent_type == "system"
};

permit(
  principal,
  action,
  resource is DirectionFeedback
) when {
  principal.agent_type == "system"
};

// ============================================================
// Admin: full access to all Foresight entities
// ============================================================

permit(
  principal is Admin,
  action,
  resource is ProductModel
);

permit(
  principal is Admin,
  action,
  resource is Projection
);

permit(
  principal is Admin,
  action,
  resource is Observation
);

permit(
  principal is Admin,
  action,
  resource is Direction
);

permit(
  principal is Admin,
  action,
  resource is DirectionFeedback
);

// ============================================================
// Read/List: any authenticated principal can read all entities
// ============================================================

permit(
  principal,
  action in [Action::"read", Action::"list"],
  resource is ProductModel
);

permit(
  principal,
  action in [Action::"read", Action::"list"],
  resource is Projection
);

permit(
  principal,
  action in [Action::"read", Action::"list"],
  resource is Observation
);

permit(
  principal,
  action in [Action::"read", Action::"list"],
  resource is Direction
);

// DirectionFeedback in Open state: readable only by owning reviewer + system.
// Advisory: Cedar cannot inspect entity field values at policy eval time.
// The platform enforces this via application-level filtering. The general
// read permission below applies to Addressed/Resolved/Reopened feedback.
permit(
  principal,
  action in [Action::"read", Action::"list"],
  resource is DirectionFeedback
);

// ============================================================
// ProductModel: system and human can seed/refresh/manage
// ============================================================

permit(
  principal,
  action in [Action::"create", Action::"Seed", Action::"RefreshSignals", Action::"MarkStale", Action::"Reactivate"],
  resource is ProductModel
) when {
  ["human", "supervisor"].contains(principal.agent_type)
};

// WASM callbacks for seed completion/failure
permit(
  principal,
  action in [Action::"SeedComplete", Action::"SeedFailed", Action::"RefreshComplete"],
  resource is ProductModel
) when {
  principal.agent_type == "system"
};

// ============================================================
// Projection: system and human can create/manage; probes observe
// ============================================================

permit(
  principal,
  action in [Action::"create", Action::"Configure", Action::"Start", Action::"Complete", Action::"Branch"],
  resource is Projection
) when {
  ["human", "supervisor"].contains(principal.agent_type)
};

// WASM callbacks
permit(
  principal,
  action in [Action::"ProbesReady", Action::"AdvanceStep", Action::"StepComplete", Action::"Fail"],
  resource is Projection
) when {
  principal.agent_type == "system"
};

// ============================================================
// Observation: probe agents can create and record
// ============================================================

permit(
  principal,
  action in [Action::"create", Action::"Record"],
  resource is Observation
) when {
  principal.role == "probe"
};

// Confirm: probe agents can confirm observations they did NOT create.
// Cedar enforces confirmer != creator via context check.
permit(
  principal,
  action == Action::"Confirm",
  resource is Observation
) when {
  principal.role == "probe" &&
  context.confirmer_agent_id != context.creator_agent_id
};

// Escalate: any probe or human can escalate a confirmed observation
permit(
  principal,
  action == Action::"Escalate",
  resource is Observation
) when {
  principal.role == "probe" ||
  ["human", "supervisor"].contains(principal.agent_type)
};

// Fade: system, human, or any probe can fade an observation
permit(
  principal,
  action == Action::"Fade",
  resource is Observation
) when {
  principal.role == "probe" ||
  ["human", "supervisor", "system"].contains(principal.agent_type)
};

// ============================================================
// Direction: probe agents propose; humans select/archive
// ============================================================

// Probes can create and propose directions
permit(
  principal,
  action in [Action::"create", Action::"Propose"],
  resource is Direction
) when {
  principal.role == "probe"
};

// Review actions: system or probe agents
permit(
  principal,
  action in [Action::"SubmitForReview", Action::"RecordReview", Action::"RecordConfirmation", Action::"MarkCrossModelAgreement"],
  resource is Direction
) when {
  principal.role == "probe" ||
  principal.agent_type == "system"
};

// Implementation lifecycle: system manages
permit(
  principal,
  action in [Action::"BeginImplementation", Action::"ImplementationComplete", Action::"Fail"],
  resource is Direction
) when {
  ["system", "human", "supervisor"].contains(principal.agent_type)
};

// Select: only humans can select a direction for the roadmap
permit(
  principal,
  action == Action::"Select",
  resource is Direction
) when {
  ["human", "admin"].contains(principal.agent_type)
};

// Archive: humans and supervisors can archive at any stage
permit(
  principal,
  action == Action::"Archive",
  resource is Direction
) when {
  ["human", "supervisor", "admin"].contains(principal.agent_type)
};

// ============================================================
// DirectionFeedback: blind review by probe agents
// ============================================================

// Probes can create and submit feedback on directions they did NOT propose.
// Cedar enforces reviewer != proposer via context check.
permit(
  principal,
  action in [Action::"create", Action::"Submit"],
  resource is DirectionFeedback
) when {
  principal.role == "probe" &&
  context.reviewer_probe_id != context.proposer_agent_id
};

// Address: the direction proposer (or system) responds to feedback
permit(
  principal,
  action in [Action::"Address", Action::"ReAddress"],
  resource is DirectionFeedback
) when {
  principal.role == "probe" ||
  principal.agent_type == "system"
};

// Resolve: the reviewer or system resolves feedback
permit(
  principal,
  action == Action::"Resolve",
  resource is DirectionFeedback
) when {
  principal.role == "probe" ||
  principal.agent_type == "system"
};

// Reopen: the reviewer can reopen addressed feedback
permit(
  principal,
  action == Action::"Reopen",
  resource is DirectionFeedback
) when {
  principal.role == "probe" ||
  principal.agent_type == "system"
};

// ============================================================
// WASM module HTTP call authorization
// ============================================================

permit(
  principal is Agent,
  action == Action::"http_call",
  resource is HttpEndpoint
) when {
  ["seed_model", "spawn_probes", "advance_step"].contains(context.module)
};

// Allow foresight WASM modules to access secrets
permit(
  principal is Agent,
  action == Action::"access_secret",
  resource is Secret
) when {
  ["seed_model", "spawn_probes", "advance_step"].contains(context.module)
};
