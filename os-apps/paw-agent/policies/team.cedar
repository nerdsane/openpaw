// Team — group of agents
permit(
  principal,
  action in [Action::"create", Action::"read", Action::"list", Action::"Configure", Action::"Update"],
  resource is Team
);

permit(
  principal,
  action == Action::"Archive",
  resource is Team
) when {
  ["supervisor", "human", "admin"].contains(principal.agent_type)
};
