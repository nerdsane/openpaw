# ARN-441 spec (temperpaw slice)

Canonical RFC: `arni-labs/stack` `docs/efforts/ARN-441/spec.md`. This file is the
temperpaw-implementable slice for the paw-patrol app; read the stack RFC for the
full design and acceptance criteria.

## Owner ruling that shapes this PR (2026-08-31)

Option **B, additive-then-flip** (see decisions.md). The rename premise recorded in
the foundation ("WorkRequest/WorkCycle are not the live driver") was contradicted by
source: `crates/paw-codex-worker` (temper_api.rs:111, `entity_url("WorkCycles")`) and
`dashboard/` (`queryEntities('WorkCycles')`) drive/render the live WorkCycle via the
CSDL EntitySet names. A hard rename would 404 the live worker and blank the dashboard.
So Intent and Effort are created as NEW types the new lifecycle speaks; WorkRequest/
WorkCycle, the worker, and the dashboard stay untouched and retire at the phase-3
flip (like risk_rule and the CI gates).

## PR 1a scope (this PR)

Additive, declarative, no wasm change:

- **Intent** (`specs/intent.ioa.toml`) - the WorkRequest shape in SDLC vocabulary.
  `Accept` births an Effort via a declarative entity trigger (create + Seed) run
  under the `patrol-intake-service` principal (mirrors the ReleaseRun/
  patrol-release-service elevation).
- **Effort** (`specs/effort.ioa.toml`) - WorkCycle EXTENDED to the full lifecycle:
  `Intended (initial) -> Specified -> Planned -> Building -> InReview -> Proving ->
  Merged -> Deploying -> Verified`, with `Stalled` (recoverable) and `Abandoned`
  (terminal). Chain reference fields (intent_id, review_run_ids, proof_packet_ids,
  deployment_id, adjudication_ids, pm_issue_id), design-chain refs (spec_ref,
  plan_ref), and gate markers back the state guards. Pre-attachment states
  (Intended/Specified/Planned) are allow_indefinite (no owner, no lease).
- **CSDL** - Intent/Effort EntityTypes + EntitySets Intents/Efforts.
- **Cedar** - Intent intake for agents; Effort lifecycle for the system/patrol
  surface; Effort birth (create + Seed) locked to `patrol-intake-service`.

## PR 1b scope (next, on top of this)

Chain-file guards (Specify/Plan refuse without spec.md/plan.md on TemperFS, enforced
by a generalized `effort_lifecycle` wasm), WorkerRun `Heartbeat`, and the ownership
lease (state_timeout -> Stalled on the owned states), proven by a synthetic drive
including a forced timeout -> Stalled and a missing-spec.md refusal.

## Deferred (recorded, not dropped)

- WorkCycle's `AwaitingHuman*` pause states: completion-approval becomes step 3's
  Cedar-deny -> MCP elicitation; a start-approval pause, if wanted, becomes a
  StartBuild guard in a later step.
- `Resume` out of Stalled will require an Adjudication guard (a later step).
- Intake routing for Intent (the new-type equivalent of patrol_request_router)
  generalizes in a later step; the Effort is born at Accept without it.
- Merge as a Cedar permit (step 3); Deployment entity linkage (step 4).

## Acceptance (this PR)

L0-L3 cascade + composite cross-entity verification pass with Intent/Effort; all
wasm still build; the app boots on an isolated TURSO_URL; one synthetic Effort is
hand-driven Intent.Accept -> Effort born (Intended) -> ... -> Verified, read back via
OData at each step. Genesis publish -> install after merge.
