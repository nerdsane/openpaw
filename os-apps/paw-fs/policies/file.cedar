// TemperFS File Authorization Policies

// Any authenticated principal can read/list files
permit(
    principal,
    action in [Action::"read", Action::"list"],
    resource is File
);

// Agents can create, update, lock, and unlock files in their own workspace
permit(
    principal is Agent,
    action in [Action::"StreamUpdated", Action::"Create", Action::"create", Action::"update", Action::"Lock", Action::"Unlock"],
    resource is File
) when {
    resource.workspaceId == principal.workspaceid
};

// System agents execute Temper-native PawFS workflows such as ArtifactBatch and
// Monty write helpers; their target workspace is carried in the entity fields.
permit(
    principal is Agent,
    action in [Action::"StreamUpdated", Action::"Create", Action::"create", Action::"update"],
    resource is File
) when {
    principal.agent_type == "system"
};

permit(
    principal is Agent,
    action == Action::"RecordVersion",
    resource is File
) when {
    principal.agent_type == "file-service"
};

// Locked files reject StreamUpdated (defense in depth — transition table also rejects)
forbid(
    principal,
    action == Action::"StreamUpdated",
    resource is File
) when {
    resource.status == "Locked"
};

// Archive requires workspace admin
permit(
    principal is Agent,
    action == Action::"FileArchive",
    resource is File
) when {
    principal in Group::"WorkspaceAdmins" && resource.workspaceId == principal.workspaceid
};
