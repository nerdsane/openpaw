# Decision log - ARN-441 step 1 (temperpaw side: rename migration + Effort lifecycle)

The canonical RFC/design chain is in `arni-labs/stack` `docs/efforts/ARN-441/`
(intent/spec/plan/decisions). This file is the temperpaw PR's decision log for the
paw-patrol spec+policy changes; the PR body carries these entries verbatim.

---

**Decision:** The WorkRequest->Intent / WorkCycle->Effort rename is a NEW entity
type per name; existing WorkRequest/WorkCycle prod rows are marked LEGACY, not
carried over.
**Came up because:** step 1 must decide "carry rows over or mark legacy - investigate
what the platform supports" (RFC naming section).
**Options:** (a) an automatic rename/migration that re-keys existing rows to the new
type; (b) new entity types, old rows left legacy under their old type names.
**Chose (b) over (a) because:** the platform has NO entity-type rename/alias/migration
mechanism - persistence keys on the type name (`persistence_id = "{tenant}:{entity_type}:{entity_id}"`,
temper `entity_actor/actor.rs:339`), and no genesis/registry rename path exists
(verified: no rename/alias handling in the kernel). Re-keying would be a kernel
change, which is explicitly out of scope for ARN-441. Marking legacy is safe here
because WorkRequest/WorkCycle are NOT yet the live stage-2 driver - stage 2 runs on
PR comments + check runs + bash routing (RFC "what we are addressing"), so no live
prod flow dispatches them; the renamed Intent/Effort entities are driven
synthetically in shadow (plan step 1). Given up: query continuity for any old rows
(they age out; the SDLC entities are short-lived per-effort rows, not durable state).
**Where:** paw-patrol `specs/intent.ioa.toml` (was work_request), `specs/effort.ioa.toml`
(was work_cycle), `specs/model.csdl.xml`, `policies/patrol.cedar`, wasm references.

---

**Decision:** Execute the rename as an atomic total rename across the declarative
layer AND the wasm layer in one PR, verified by cascade + wasm build + app boot -
NOT a partial "spec-only" pass.
**Came up because:** the reference survey found the rename is far larger than the
plan's "spec change only, no wasm" framing: ~700 references across ~30 files,
including 10 wasm Rust modules (review_gate_lifecycle 157, patrol_run_lifecycle 100,
work_cycle_lifecycle 39, worker_run_lifecycle 36, patrol_request_router 35,
signal_router 28, daily_brief_lifecycle 28, finding_lifecycle 26, repo_sweep 17),
plus work_cycle.ioa.toml (21), patrol.cedar (19), model.csdl.xml (18), and ~10 other
specs that cross-reference the types. The wasm modules reference WorkRequest/WorkCycle
as entity-type strings (cross-entity dispatch/guards) and field names (work_cycle_id
etc.), so a declarative-only rename would fail to install (cross-entity references to
a renamed type break; wasm dispatch to the old type 404s).
**Options:** (a) declarative-only step 1, wasm later (leaves a broken intermediate
that can't shadow-drive); (b) atomic total rename in one PR.
**Chose (b) because:** shadow-first requires nothing breaks - a half-rename breaks.
The rename must be complete (all 700 refs) to install and be driven. Given up: a
smaller first PR - but a rename cannot be half-done.
**Rename map:** `WorkRequest`->`Intent`, `WorkCycle`->`Effort`; reference fields
`work_request_id`->`intent_id`, `work_cycle_id`->`effort_id`; file renames
work_request.ioa.toml->intent.ioa.toml, work_cycle.ioa.toml->effort.ioa.toml. ADRs
are historical records - left with the then-current names (a note added). release_run
->Deployment is step-4 scope; only the `deployment_id` reference field on Effort lands
now.
**Where:** all os-apps/paw-patrol specs + model.csdl.xml + patrol.cedar + the 10 wasm
modules + APP.md.

---

**Decision:** Split ARN-441 step 1 into TWO PRs: a pure mechanical rename PR first,
the Effort-lifecycle additions second (owner ruling, 2026-08-31).
**Came up because:** the rename is ~700 refs across 10 wasm modules; the earlier
"one atomic PR" plan mixed a huge mechanical change with judgment work.
**Options:** (a) one PR (rename + lifecycle); (b) rename-only PR first, lifecycle
PR second.
**Chose (b) over (a) because:** owner ruling - a ~700-ref/10-wasm mechanical change
wants isolation. PR 1 = pure rename (types + fields + files + cedar + csdl + wasm
refs), ZERO behavior change, proven by cascade + all-wasm build + app boot +
synthetic hand-dispatch drive; it merges + publishes, then PR 2 adds the six Effort
states + chain-file guards + chain reference fields + WorkerRun Heartbeat on top as
the judgment PR. Isolating the blast radius makes both reviewable and revertible.
Given up: nothing - two smaller PRs are strictly easier to review than one.
**Where:** PR 1 (rename) then PR 2 (lifecycle) on temperpaw, `claude/arn-441-*`.

---

**Decision:** This effort is handed off to a successor session (owner-accepted,
2026-08-31); the authoring session did NOT execute the rename surgery.
**Came up because:** the mandate is ARN-441 all 5 steps to merged+published+installed;
the authoring session had run the full ARN-438 arc and was context-heavy with a
non-zero error rate at quality-critical moments, and step 1 is a prod-breaking
~700-ref rename.
**Options:** (a) push the tired context through the multi-PR mandate; (b) hand off
to a fresh-context successor with a full brief.
**Chose (b) because:** the ownership rule (quality bar over raw progress); a clean
context lowers error risk on a prod-breaking multi-step effort. Owner accepted and
recorded it as the model for the honest hand-off call.
**Where:** handoff brief at scratchpad/ARN-441-handoff-brief.md; foundation
(migration decision, rename survey/map, this chain) preserved on
`claude/arn-441-entity-loop`.
