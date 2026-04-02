// Agent identity — persistent team member
permit(
  principal,
  action in [Action::"create", Action::"read", Action::"list", Action::"Configure", Action::"Update"],
  resource is Agent
);

permit(
  principal,
  action == Action::"Archive",
  resource is Agent
) when {
  ["supervisor", "human", "admin"].contains(principal.agent_type)
};
