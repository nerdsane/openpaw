// SessionLink -- child Session monitor policies.

permit(
  principal is Admin,
  action,
  resource is SessionLink
);

permit(
  principal,
  action in [Action::"create", Action::"read", Action::"list"],
  resource is SessionLink
) when {
  ["supervisor", "human", "system", "agent"].contains(principal.agent_type)
};

permit(
  principal,
  action in [
    Action::"Configure",
    Action::"CheckChild",
    Action::"ChildPending",
    Action::"ParentNotified",
    Action::"NotifyFailed"
  ],
  resource is SessionLink
) when {
  principal.agent_type == "system" ||
  principal == resource
};
