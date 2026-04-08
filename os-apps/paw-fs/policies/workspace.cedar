// TemperFS Workspace Authorization Policies

// Agents can create workspaces
permit(
    principal is Agent,
    action == Action::"Create",
    resource is Workspace
);

// Workspace members can read and use their workspace
permit(
    principal is Agent,
    action in [Action::"IncrementUsage", Action::"DecrementUsage", Action::"UpdateQuota"],
    resource is Workspace
) when {
    resource.id == principal.workspaceid
};

// Agents can perform filesystem operations (FUSE-mapped: mkdir, creat, addchild, counters)
permit(
    principal is Agent,
    action in [Action::"MkDir", Action::"CreateFile", Action::"DirCreated", Action::"FileCreated",
               Action::"IncrementFileCount", Action::"DecrementFileCount"],
    resource is Workspace
);

// Agents can manage directories (addchild, create, rename)
permit(
    principal is Agent,
    action in [Action::"Create", Action::"AddChild", Action::"RemoveChild", Action::"DirectoryRename", Action::"DirectoryArchive"],
    resource is Directory
);

// Only workspace admins can freeze, thaw, or archive
permit(
    principal is Agent,
    action in [Action::"Freeze", Action::"Thaw", Action::"WorkspaceArchive"],
    resource is Workspace
) when {
    principal in Group::"WorkspaceAdmins" && resource.id == principal.workspaceid
};
