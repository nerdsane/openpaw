# ADR-0055: Directed Evolution Hot-Loaded Variant Runtimes

## Status

Accepted.

## Context

Directed Evolution now creates real `WorkItem` brain runs and can generate
candidate app changes in local Codex worktrees. That is not enough for the
intended pipeline: simulated users and reviewers must evaluate running
variants, not just inspect local diffs. Railway deployments are costly and
should not be used to materialize every candidate. Temper-native apps already
support the cheaper route: publish a pinned Genesis app ref and install it into
a running Temper tenant through `App.Install`.

## Decision

The Directed Evolution Codex worker will materialize each generated variant as
a hot-loaded Genesis runtime:

- A `variant_generator` work item still edits and commits the organism app in an
  isolated local worktree.
- After the commit, the worker pushes the commit to the Genesis git remote as
  candidate bytes, but does not advance the canonical app's latest version.
  Promotion is the step that decides whether a pinned candidate becomes the new
  parent.
- The worker installs the pinned app ref into a variant-scoped tenant through
  `App.Install`, using a deterministic tenant name derived from the work item.
- The worker reports the pinned `app_ref`, branch, diff ref, and
  `temper://tenant/<tenant>/app/<app_ref>` runtime ref back to Directed
  Evolution.
- If a `variant_generator` Codex process exits or times out after leaving real
  git changes, the worker may recover those changes as a candidate, mark the
  output as recovered, commit and hot-load it, and leave viability to the later
  evaluation stages. This recovery can be disabled with
  `PAW_DE_RECOVER_VARIANT_CHANGES_ON_CODEX_ERROR=false`.
- Reviewer and simulated-user work items are cancelled before claim when their
  target `StageResult` or `Variant` has already been eliminated. Generation can
  queue multiple stages up front, so late queued work must not waste background
  Codex runs or add confusing evidence after a candidate is dead.

This is an agent capability surface, not a hidden orchestration layer. The
episode still advances through Directed Evolution entities and WASM transitions;
the worker only supplies the external agent work product and self-reports it
through the existing `WorkItem` and `BrainRun` actions.

## Consequences

- Variant evaluation can point simulated users at a real live tenant without a
  Railway deploy.
- Multiple variants can be active concurrently by using separate tenants.
- Promotion can later install the winner into the main organism tenant and
  advance the canonical app version by the same pinned Genesis app ref.
- The worker needs Genesis registry URL and tenant configuration, and it must
  fail the work item if publish or install fails instead of returning a
  pretend runtime.
- Recovery makes variant generation tolerant of agent finalization failures
  without treating unfinished code as a winner; a recovered variant must still
  survive hot-load, simulated-user trials, review, selection, and promotion.
- Stale work cancellation keeps evaluation throughput focused on live variants
  while preserving entity-level auditability through `CancelWorkItem`.
