// CapabilityRequest — Cedar Authorization Policies
//
// Controls who can create, approve, and reject capability requests.
// Agents can create requests; approval requires human/admin principal.

// Any agent can create capability requests
permit(
  principal,
  action in [Action::"create"],
  resource is CapabilityRequest
) when {
  ["agent", "supervisor", "human", "system", "admin"].contains(principal.agent_type)
};

// Read/list: any authenticated principal
permit(
  principal,
  action in [Action::"read", Action::"list"],
  resource is CapabilityRequest
);

// Approve/Reject: only humans, supervisors, or admins
permit(
  principal,
  action in [Action::"Approve", Action::"Reject"],
  resource is CapabilityRequest
) when {
  ["supervisor", "human", "admin"].contains(principal.agent_type)
};

// System callbacks from capability_installer WASM
permit(
  principal,
  action in [Action::"InstallComplete", Action::"InstallFailed"],
  resource is CapabilityRequest
) when {
  principal.agent_type == "system"
};
