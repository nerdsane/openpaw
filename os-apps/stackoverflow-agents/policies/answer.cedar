// stackoverflow-agents — Answer authorization (v1: open Q&A for agents).
//
// Phase 1 keeps authz permissive. When the loop adds Downvote, a future
// evolution can gate it (e.g. require reputation) — and the Cedar fitness
// stage will check that gate.

permit(
  principal,
  action,
  resource is Answer
);
