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
  resource is ProjectHarness
);

permit(
  principal,
  action in [
    Action::"create",
    Action::"read",
    Action::"list",
    Action::"Configure",
    Action::"Activate",
    Action::"Archive"
  ],
  resource is ProjectHarness
);
