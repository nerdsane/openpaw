// paw-consilium Core — Cedar Policies

permit(
  principal,
  action in [Action::"create", Action::"read", Action::"list", Action::"Configure", Action::"Convene", Action::"AllSpawned", Action::"SpawnFailed", Action::"PerspectiveComplete", Action::"PerspectiveFailed", Action::"BeginSynthesis", Action::"SynthesisComplete", Action::"SynthesisFailed"],
  resource is Deliberation
);

permit(
  principal,
  action in [Action::"create", Action::"read", Action::"list", Action::"Configure", Action::"Begin", Action::"Complete", Action::"Fail"],
  resource is Perspective
);

permit(
  principal,
  action in [Action::"create", Action::"read", Action::"list", Action::"Draft", Action::"Publish", Action::"Archive"],
  resource is Brief
);

// Context isolation: forbid reading other Perspectives during Generating state.
// Perspectives must never see each other's output.
forbid(
  principal,
  action == Action::"read",
  resource is Perspective
) when {
  context.caller_entity_type == "Perspective" &&
  context.caller_entity_state == "Generating"
};
