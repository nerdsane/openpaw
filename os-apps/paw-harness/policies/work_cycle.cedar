permit(
  principal is Admin,
  action in [
    Action::"create",
    Action::"read",
    Action::"list",
    Action::"Configure",
    Action::"WritePlan",
    Action::"StartWork",
    Action::"BeginTesting",
    Action::"PassTests",
    Action::"Approve",
    Action::"Fail"
  ],
  resource is WorkCycle
);

permit(
  principal,
  action in [
    Action::"create",
    Action::"read",
    Action::"list",
    Action::"Configure",
    Action::"WritePlan",
    Action::"StartWork",
    Action::"BeginTesting",
    Action::"PassTests"
  ],
  resource is WorkCycle
);

permit(
  principal,
  action == Action::"Approve",
  resource is WorkCycle
) when {
  principal.id != resource.PlannerId
};

permit(
  principal,
  action == Action::"Fail",
  resource is WorkCycle
) when {
  principal.agent_type == "system" ||
  principal.id == resource.PlannerId ||
  principal.id == resource.ApproverId
};
