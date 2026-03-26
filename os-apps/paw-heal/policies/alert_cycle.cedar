permit(
  principal is Admin,
  action in [
    Action::"create",
    Action::"read",
    Action::"list",
    Action::"Open",
    Action::"HealComplete",
    Action::"TuneComplete",
    Action::"Escalate"
  ],
  resource is AlertCycle
);

permit(
  principal,
  action in [
    Action::"create",
    Action::"read",
    Action::"list",
    Action::"Open",
    Action::"HealComplete",
    Action::"TuneComplete",
    Action::"Escalate"
  ],
  resource is AlertCycle
);
