// DSF Harness Cedar policies for DsfWorkCycle entities.
// Allows all principals to create, read, and list.
// Allows system and agent principals to dispatch all actions.

permit(
  principal,
  action in [
    Action::"create",
    Action::"read",
    Action::"list",
    Action::"Configure",
    Action::"Activate",
    Action::"CreateWorkCycle",
    Action::"WritePlan",
    Action::"StartWork",
    Action::"UpdateSandbox",
    Action::"ReportMigrations",
    Action::"ReportTypecheck",
    Action::"ReportUnitTests",
    Action::"ReportDst",
    Action::"ReportPolicyGates",
    Action::"ReportE2e",
    Action::"BeginTesting",
    Action::"PassTests"
  ],
  resource is DsfWorkCycle
);

// Approve requires a different principal than the planner.
permit(
  principal,
  action == Action::"Approve",
  resource is DsfWorkCycle
) when {
  principal.id != resource.PlannerId
};

// RequestChanges requires a different principal than the planner.
permit(
  principal,
  action == Action::"RequestChanges",
  resource is DsfWorkCycle
) when {
  principal.id != resource.PlannerId
};

// Fail is allowed for system agents, the planner, or the approver.
permit(
  principal,
  action == Action::"Fail",
  resource is DsfWorkCycle
) when {
  principal.agent_type == "system" ||
  principal.id == resource.PlannerId ||
  principal.id == resource.ApproverId
};
