// paw-autoreason Core — Cedar Policies

permit(
  principal,
  action in [Action::"create", Action::"read", Action::"list", Action::"Configure", Action::"Begin", Action::"RoundReady", Action::"BeginJudging", Action::"JudgmentSubmitted", Action::"AllJudged", Action::"NextRound", Action::"Converge", Action::"ReachMaxRounds", Action::"Fail"],
  resource is Tournament
);

permit(
  principal,
  action in [Action::"create", Action::"read", Action::"list", Action::"Configure", Action::"RecordVersions", Action::"RecordResult"],
  resource is Round
);

permit(
  principal,
  action in [Action::"create", Action::"read", Action::"list", Action::"Configure", Action::"Finalize"],
  resource is Version
);

permit(
  principal,
  action in [Action::"create", Action::"read", Action::"list", Action::"Configure", Action::"Submit"],
  resource is Judgment
);

// Blind evaluation: judges cannot read other judges' submissions
forbid(
  principal,
  action == Action::"read",
  resource is Judgment
) when {
  context.caller_entity_type == "Judgment" &&
  context.caller_entity_state == "Created"
};
