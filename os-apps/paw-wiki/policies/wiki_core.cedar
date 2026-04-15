// paw-wiki Core — Cedar Policies

permit(
  principal,
  action in [Action::"create", Action::"read", Action::"list", Action::"Submit", Action::"Index", Action::"IndexFailed"],
  resource is WikiSource
);

permit(
  principal,
  action in [Action::"create", Action::"read", Action::"list", Action::"Draft", Action::"Publish", Action::"UpdateLintStatus", Action::"BeginRevision", Action::"Revise", Action::"Republish", Action::"Archive"],
  resource is WikiPage
);

permit(
  principal,
  action in [Action::"create", Action::"read", Action::"list", Action::"Configure", Action::"Submit", Action::"SessionSpawned", Action::"Start", Action::"RecordProgress", Action::"Complete", Action::"Fail"],
  resource is WikiJob
);
