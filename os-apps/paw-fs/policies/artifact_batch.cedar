// TemperFS ArtifactBatch Authorization Policies

permit(
    principal is Agent,
    action in [Action::"read", Action::"list", Action::"create", Action::"Submit", Action::"Apply", Action::"RecordFileApplied",
               Action::"RecordFileFailed", Action::"Complete", Action::"Fail"],
    resource is ArtifactBatch
);
