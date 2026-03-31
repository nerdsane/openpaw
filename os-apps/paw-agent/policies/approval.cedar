// PendingApproval — Cedar Authorization Policies
//
// Agents can create approval requests. Only humans/supervisors can
// approve or deny them. This is the guardrail that prevents agents
// from bypassing Cedar authorization on their own.

// Admins can do everything
permit(
  principal is Admin,
  action,
  resource is PendingApproval
);

// Agents and system can create and request approval
permit(
  principal,
  action in [Action::"create", Action::"Request"],
  resource is PendingApproval
) when {
  ["system", "agent"].contains(principal.agent_type)
};

// Only supervisors and humans can approve or deny
permit(
  principal,
  action in [Action::"Approve", Action::"Deny"],
  resource is PendingApproval
) when {
  ["supervisor", "human"].contains(principal.agent_type)
};

// System can expire
permit(
  principal,
  action in [Action::"Expire"],
  resource is PendingApproval
) when {
  principal.agent_type == "system"
};

// Any authenticated principal can read/list
permit(
  principal,
  action in [Action::"read", Action::"list"],
  resource is PendingApproval
);
