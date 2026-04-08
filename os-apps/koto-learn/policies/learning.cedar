// Kotowari Learning — Cedar Policies
//
// Baseline permits for learning entity operations.
// Progressive scaffolding and tutor restrictions will be layered on top.

// --- LearnerProfile: any agent can manage profiles ---

permit(
  principal,
  action in [Action::"create", Action::"read", Action::"list", Action::"Configure", Action::"Activate", Action::"RecordEncounter", Action::"Evolve", Action::"Pause", Action::"Resume", Action::"Graduate"],
  resource is LearnerProfile
);

// --- Persona: any agent can read, system can manage ---

permit(
  principal,
  action in [Action::"create", Action::"read", Action::"list", Action::"Define", Action::"ComputeCoverage", Action::"Publish", Action::"UpdateCoverage", Action::"Deprecate"],
  resource is Persona
);

// --- KnowledgeBank: any agent can read, system can import ---

permit(
  principal,
  action in [Action::"create", Action::"read", Action::"list", Action::"Configure", Action::"BeginImport", Action::"ImportProgress", Action::"ImportComplete", Action::"ImportFailed", Action::"Refresh", Action::"MarkStale", Action::"Reactivate"],
  resource is KnowledgeBank
);

// --- Concept: any agent can read, system can manage ---

permit(
  principal,
  action in [Action::"create", Action::"read", Action::"list", Action::"Define", Action::"Publish", Action::"Recalibrate", Action::"Deprecate"],
  resource is Concept
);

// --- ConceptLink: any agent can read, system can manage ---

permit(
  principal,
  action in [Action::"create", Action::"read", Action::"list", Action::"Define", Action::"Validate", Action::"Reweight", Action::"Invalidate"],
  resource is ConceptLink
);

// --- Mastery: agents can read and assess ---

permit(
  principal,
  action in [Action::"create", Action::"read", Action::"list", Action::"Discover", Action::"Assess", Action::"Advance", Action::"Regress", Action::"MarkRegressionRisk"],
  resource is Mastery
);

// --- Encounter: full lifecycle for tutor agents ---

permit(
  principal,
  action in [Action::"create", Action::"read", Action::"list", Action::"Design", Action::"Finalize", Action::"Present", Action::"Complete", Action::"RecordEvaluation", Action::"Abandon", Action::"Fail"],
  resource is Encounter
);
