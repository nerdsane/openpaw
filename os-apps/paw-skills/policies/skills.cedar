// Native skill installation policies.

permit(
  principal is Admin,
  action,
  resource is SkillInstall
);

permit(
  principal is Admin,
  action,
  resource is SkillPackage
);

permit(
  principal is Admin,
  action,
  resource is SkillBinding
);

permit(
  principal,
  action in [Action::"create"],
  resource is SkillInstall
) when {
  ["agent", "supervisor", "human", "system", "admin"].contains(principal.agent_type)
};

permit(
  principal,
  action in [Action::"read", Action::"list"],
  resource is SkillInstall
);

permit(
  principal,
  action in [Action::"read", Action::"list"],
  resource is SkillPackage
);

permit(
  principal,
  action in [Action::"read", Action::"list"],
  resource is SkillBinding
);

permit(
  principal,
  action in [Action::"Approve", Action::"Reject", Action::"Archive"],
  resource is SkillInstall
) when {
  ["supervisor", "human", "admin"].contains(principal.agent_type)
};

permit(
  principal,
  action in [Action::"InstallComplete", Action::"InstallFailed"],
  resource is SkillInstall
) when {
  principal.agent_type == "system"
};

permit(
  principal,
  action in [Action::"create", Action::"Register", Action::"Archive"],
  resource is SkillPackage
) when {
  principal.agent_type == "system" || principal.agent_type == "admin"
};

permit(
  principal,
  action in [Action::"create", Action::"Activate", Action::"Archive"],
  resource is SkillBinding
) when {
  principal.agent_type == "system" || principal.agent_type == "admin"
};

permit(
  principal is Agent,
  action == Action::"http_call",
  resource is HttpEndpoint
) when {
  context.module == "skill_installer"
};
