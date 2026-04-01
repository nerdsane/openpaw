permit(
  principal is Admin,
  action in [
    Action::"create",
    Action::"read",
    Action::"list",
    Action::"Configure",
    Action::"Activate",
    Action::"Archive"
  ],
  resource is Harness
);

permit(
  principal,
  action in [
    Action::"create",
    Action::"read",
    Action::"list",
    Action::"Configure",
    Action::"Activate"
  ],
  resource is Harness
);

permit(
  principal,
  action == Action::"Archive",
  resource is Harness
) when {
  principal.agent_type == "system"
};
