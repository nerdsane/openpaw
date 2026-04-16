// paw-managed-agents — broad app-local authorization policy.
//
// This app is currently intended as an internal orchestration surface, so the
// policy stays permissive for the managed-agent entities while still granting
// the extra OpenPaw permissions the WASM bridge needs.

permit(
  principal,
  action in [Action::"create", Action::"read", Action::"list",
             Action::"Archive"],
  resource is ManagedEnvironment
);

permit(
  principal,
  action in [Action::"create", Action::"read", Action::"list",
             Action::"Archive", Action::"BindInnerAgent",
             Action::"SyncComplete",
             Action::"UpdateManagedAgent"],
  resource is ManagedAgent
);

permit(
  principal,
  action in [Action::"create", Action::"read", Action::"list",
             Action::"StartSession", Action::"IdleSession", Action::"ResumeSession",
             Action::"TerminateSession", Action::"Archive",
             Action::"InnerSessionReady",
             Action::"InnerSessionPending", Action::"CheckInnerSession",
             Action::"InnerSessionFailed", Action::"UpdateUsage"],
  resource is ManagedSession
);

permit(
  principal,
  action in [Action::"create", Action::"read", Action::"list"],
  resource is AgentMcpServer
);

permit(
  principal,
  action in [Action::"create", Action::"read", Action::"list"],
  resource is AgentSkill
);

permit(
  principal,
  action in [Action::"create", Action::"read", Action::"list"],
  resource is AgentTool
);

permit(
  principal,
  action in [Action::"create", Action::"read", Action::"list"],
  resource is AgentToolConfig
);

permit(
  principal,
  action in [Action::"create", Action::"read", Action::"list"],
  resource is SessionEvent
);

permit(
  principal,
  action in [Action::"create", Action::"read", Action::"list"],
  resource is SessionResource
);

permit(
  principal,
  action in [Action::"create", Action::"read", Action::"list"],
  resource is EnvironmentPackage
);

// Allow the managed-agents bridge to create/update OpenPaw inner entities.
permit(
  principal,
  action in [Action::"create", Action::"Configure", Action::"Update", Action::"Archive"],
  resource is Agent
) when {
  principal.agent_type == "agent" || principal.agent_type == "system"
};

permit(
  principal,
  action in [Action::"create", Action::"read", Action::"list",
             Action::"Configure", Action::"Steer", Action::"Cancel", Action::"Fail",
             Action::"RecordResult"],
  resource is Session
) when {
  principal.agent_type == "agent" || principal.agent_type == "system"
};
