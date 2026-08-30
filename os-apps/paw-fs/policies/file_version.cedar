// TemperFS FileVersion Authorization Policies

// FileVersion metadata is the immutable counterpart to readable File metadata.
// Runtime modules need it to verify that bytes still match the exact version
// selected by the caller.
permit(
    principal,
    action in [Action::"read", Action::"list"],
    resource is FileVersion
);

permit(
    principal is Agent,
    action in [Action::"Create", Action::"Supersede"],
    resource is FileVersion
) when {
    principal.agent_type == "file-service"
};
