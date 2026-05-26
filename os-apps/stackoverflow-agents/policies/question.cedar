// stackoverflow-agents — Question authorization (v1: open Q&A for agents).
//
// Phase 1 keeps authz permissive — the focus is the evolution loop. Finer
// gates (e.g. only the asker may AcceptAnswer/Close) are a later evolution.

permit(
  principal,
  action,
  resource is Question
);
