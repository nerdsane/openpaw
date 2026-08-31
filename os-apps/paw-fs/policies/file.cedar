// TemperFS File Authorization Policies

// Any authenticated principal can read/list files
permit(
    principal,
    action in [Action::"read", Action::"list"],
    resource is File
);

// Session tool reads (temper.read) authorize content access through a
// family of capitalized action names relayed as service:wasm-runtime —
// without this permit every session file read dies in WaitingForApproval
// (found live: the hindcast surveyor could not read its own frozen
// corpus). Scoped to the relay principal, mirroring the write permit below.
permit(
    principal == Agent::"service:wasm-runtime",
    action in [
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

// ARN-441 (temperpaw #492, CROSS-APP touch - flagged for panel): the shadow SDLC
// drive is our governed verified operator, and the Effort chain-file guards
// (Specify/Plan/Seed) require Ready intent.md/spec.md/plan.md Files. Grant that
// operator the NARROWEST File write surface - Create + StreamUpdated (the action
// that marks a File Ready) on File only, gated on the same verified-operator
// predicate as the paw-patrol Effort lifecycle grant (agent_type "operator" &&
// agentTypeVerified, so a self-declared header agent never matches).
// FLIP-RELEVANT: at the phase-3 flip the chain files come from the intake/worker
// principals, not the drive operator - revisit (likely remove) this grant then.
permit(
    principal is Agent,
    action in [Action::"Create", Action::"create", Action::"StreamUpdated"],
    resource is File
) when {
    principal has agent_type &&
    principal.agent_type == "operator" &&
    principal has agentTypeVerified &&
    principal.agentTypeVerified == true
};
