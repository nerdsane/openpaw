# ADR-0065: Directed Evolution Observer Source Discovery

## Status

Accepted

## Context

Directed Evolution observers must infer pressures from real app usage and
runtime evidence. The first Agent Answers walkthrough exposed a failure mode:
observer prompts and UI links could narrow the observer to a small set of
preselected Datadog queries. That made the observer look scripted, and it could
turn a seed claim or zero-result query into a direction without proving the
claim against the available system.

The observer needs help finding sources, but source hints must not become the
observation plan.

## Decision

Observer WorkItems receive a source inventory from the worker before Codex
executes the role. The inventory is a map, not a script. It can include:

- Genesis control-plane state such as WorkItems, WorkerRuns, EvidenceArtifacts,
  Signals, Pressures, Directions, Episodes, Trials, Measurements, and Mutations.
- Runtime OData metadata and sampled entity state for the target app tenant.
- Datadog log and trace samples when credentials are available.
- App source or description files, preferably from the canonical Genesis bundle
  for the pinned app ref.
- Explicit unavailable or zero-result source records.

The observer role contract requires the Codex observer to inspect additional
accessible sources when useful, reject unsupported interpretations, and report
evidence coverage in `evidence_scope`. Datadog remains mandatory when
credentials exist, but it is one source among the observed system, not the
script for the observation.

## Consequences

- Human reviewers can see which surfaces were read, empty, or unavailable.
- Observers can produce multiple candidate directions from the same evidence
  instead of forcing a single preselected direction.
- Canonical Genesis bundle materialization lets read-only observers inspect the
  actual seed app even when no local repo mapping exists.
- Source inventories may be stale or partial; observer outputs must still state
  what they independently inspected and why each source supports or does not
  support the direction.
