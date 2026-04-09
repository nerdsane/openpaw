// Plan Review — Cedar Authorization Policies
//
// Controls who can create, review, approve, and manage plans.

// Admins can do everything
permit(
  principal is Admin,
  action,
  resource is Plan
);

// Any agent can create plans and read/list them
permit(
  principal,
  action in [Action::"create", Action::"read", Action::"list"],
  resource is Plan
);

// Author can submit for review and update their own plan
permit(
  principal,
  action in [Action::"SubmitForReview", Action::"Activate", Action::"AddTask", Action::"Complete", Action::"Fail", Action::"UpdatePlan", Action::"AddExplorationNote"],
  resource is Plan
) when {
  ["agent", "supervisor", "human", "system"].contains(principal.agent_type)
};

// Reviewers and leads can approve, request changes, or escalate
permit(
  principal,
  action in [Action::"Approve", Action::"RequestChanges", Action::"Escalate"],
  resource is Plan
) when {
  ["supervisor", "human", "admin"].contains(principal.agent_type)
};

// Agents with lead role can also review (Ren)
permit(
  principal,
  action in [Action::"Approve", Action::"RequestChanges", Action::"Escalate"],
  resource is Plan
) when {
  principal.agent_type == "agent"
};

// Pause/Resume for plan management
permit(
  principal,
  action in [Action::"Pause", Action::"Resume"],
  resource is Plan
) when {
  ["agent", "supervisor", "human"].contains(principal.agent_type)
};
