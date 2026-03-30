// Memory — Cedar Authorization Policies

// Admins can do everything
permit(
  principal is Admin,
  action,
  resource is Memory
);

// System, supervisor, and human principals can manage memories
permit(
  principal,
  action in [Action::"create", Action::"Save", Action::"Update", Action::"Recall"],
  resource is Memory
) when {
  ["system", "supervisor", "human"].contains(principal.agent_type)
};

// Agents can save, update, and recall memories scoped to their own agent identity
permit(
  principal,
  action in [Action::"create", Action::"Save", Action::"Update", Action::"Recall"],
  resource is Memory
) when {
  principal.agent_type == "agent" && resource.AgentId == principal.id
};

// Supervisors and humans can archive any memory
permit(
  principal,
  action in [Action::"Archive"],
  resource is Memory
) when {
  ["supervisor", "human"].contains(principal.agent_type)
};

// Any authenticated agent can read/list memories
permit(
  principal,
  action in [Action::"read", Action::"list"],
  resource is Memory
);
