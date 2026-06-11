// TemperFS File Authorization Policies

// Any authenticated principal can read/list files. Session tool reads
// (temper.read) authorize content access through a family of capitalized
// action names relayed as service:wasm-runtime — without them every session
// file read dies in WaitingForApproval (found live: hindcast surveyor could
// not read its own frozen corpus).
permit(
    principal,
    action in [
        Action::"read", Action::"list",
        Action::"Read", Action::"Download", Action::"GetContent",
        Action::"GetValue", Action::"Stream", Action::"Open",
        Action::"GetText", Action::"FetchContent", Action::"Content"
    ],
    resource is File
);

// Agents can create, update, lock, and unlock files in their own workspace.
// Resource attributes are the entity's raw fields (snake_case): an earlier
// form of this permit compared resource.workspaceId, which never exists at
// evaluation time, so the permit was dead code — found when corridor
// sessions hit 403 on their first workspace-scoped write.
permit(
    principal is Agent,
    action in [Action::"StreamUpdated", Action::"Create", Action::"create", Action::"update", Action::"Lock", Action::"Unlock"],
    resource is File
) when {
    resource.workspace_id == principal.workspaceid
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

// The wasm-runtime service writes files on behalf of sessions (turn
// artifacts, agent temper.write via the directory tree). Without this
// permit every fresh tenant needs a runtime decision approval before any
// session can persist content — found live, twice, in corridor e2e runs.
permit(
    principal == Agent::"service:wasm-runtime",
    action in [Action::"create", Action::"Create", Action::"StreamUpdated", Action::"update"],
    resource is File
);
