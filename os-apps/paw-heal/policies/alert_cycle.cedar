permit(
  principal is Admin,
  action in [
    Action::"create",
    Action::"read",
    Action::"list",
    Action::"Open",
    Action::"HealComplete",
    Action::"BeginMerge",
    Action::"MergeComplete",
    Action::"DeployDetected",
    Action::"AlertResolved",
    Action::"AlertPersists",
    Action::"TuneComplete",
    Action::"Escalate",
    Action::"CheckMergeReady",
    Action::"CheckDeployment",
    Action::"CheckAlertResolution"
  ],
  resource is AlertCycle
);

permit(
  principal,
  action in [
    Action::"create",
    Action::"read",
    Action::"list",
    Action::"Open",
    Action::"HealComplete",
    Action::"BeginMerge",
    Action::"MergeComplete",
    Action::"DeployDetected",
    Action::"AlertResolved",
    Action::"AlertPersists",
    Action::"TuneComplete",
    Action::"Escalate",
    Action::"CheckMergeReady",
    Action::"CheckDeployment",
    Action::"CheckAlertResolution"
  ],
  resource is AlertCycle
);
