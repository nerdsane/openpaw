// ToolHook — Cedar Authorization Policies

// Admins can do everything
permit(
  principal is Admin,
  action,
  resource is ToolHook
);

// Supervisors and humans can manage hooks
permit(
  principal,
  action in [Action::"create", Action::"Register", Action::"Disable", Action::"Enable"],
  resource is ToolHook
) when {
  ["supervisor", "human"].contains(principal.agent_type)
};

// Any authenticated agent can read hooks (tool_runner queries them)
permit(
  principal,
  action in [Action::"read", Action::"list"],
  resource is ToolHook
);

// --- Tool governance policies for deep-sci-fi team harness ---

// SWE agents can create PRs but not merge
permit(
  principal,
  action == Action::"tool_execute",
  resource
)
when {
  resource.tool_name == "bash" &&
  resource.command_pattern like "*gh pr create*" &&
  principal.soul_id == "SWE"
};

forbid(
  principal,
  action == Action::"tool_execute",
  resource
)
when {
  resource.tool_name == "bash" &&
  resource.command_pattern like "*gh pr merge*" &&
  principal.soul_id == "SWE"
};

// Block dangerous commands for all agents
forbid(
  principal,
  action == Action::"tool_execute",
  resource
)
when {
  resource.tool_name == "bash" &&
  (resource.command_pattern like "*rm -rf /*" ||
   resource.command_pattern like "*git push --force*origin*main*")
};
