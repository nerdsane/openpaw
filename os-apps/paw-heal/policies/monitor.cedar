permit(
  principal is Admin,
  action in [
    Action::"create",
    Action::"read",
    Action::"list",
    Action::"Configure",
    Action::"Activate",
    Action::"AlertFired",
    Action::"Tune",
    Action::"Pause",
    Action::"Archive"
  ],
  resource is Monitor
);

permit(
  principal,
  action in [
    Action::"create",
    Action::"read",
    Action::"list",
    Action::"Configure",
    Action::"Activate",
    Action::"AlertFired",
    Action::"Tune",
    Action::"Pause",
    Action::"Archive"
  ],
  resource is Monitor
);
