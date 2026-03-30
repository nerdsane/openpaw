permit(
  principal is Admin,
  action in [
    Action::"create",
    Action::"read",
    Action::"list",
    Action::"Received",
    Action::"Validated",
    Action::"Routed",
    Action::"Processed",
    Action::"ValidationFailed",
    Action::"RouteFailed",
    Action::"ProcessFailed"
  ],
  resource is WebhookEvent
);

permit(
  principal,
  action in [
    Action::"create",
    Action::"read",
    Action::"list",
    Action::"Received",
    Action::"Validated",
    Action::"Routed",
    Action::"Processed",
    Action::"ValidationFailed",
    Action::"RouteFailed",
    Action::"ProcessFailed"
  ],
  resource is WebhookEvent
);

permit(
  principal is Admin,
  action in [
    Action::"create",
    Action::"read",
    Action::"list",
    Action::"Register",
    Action::"Update",
    Action::"Disable",
    Action::"Enable"
  ],
  resource is WebhookRoute
);

permit(
  principal,
  action in [
    Action::"create",
    Action::"read",
    Action::"list",
    Action::"Register",
    Action::"Update",
    Action::"Disable",
    Action::"Enable"
  ],
  resource is WebhookRoute
);
