// TemperFS WorkspaceUsageBucket Authorization Policies

permit(
    principal is Agent,
    action in [Action::"read", Action::"list", Action::"Create", Action::"create", Action::"ApplyDelta", Action::"Seal"],
    resource is WorkspaceUsageBucket
);
