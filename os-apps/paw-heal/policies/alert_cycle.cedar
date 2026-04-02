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

permit(
  principal is Agent,
  action == Action::"http_call",
  resource is HttpEndpoint
) when {
  [
    "alert_opener",
    "alert_verifier",
    "heal_reporter",
    "cicd_initiator",
    "cicd_merger",
    "deployment_tracker"
  ].contains(context.module)
};
