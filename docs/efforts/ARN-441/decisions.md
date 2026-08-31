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

---

**Decision:** (BLOCKER surfaced to owner, 2026-08-31, successor session) The
recorded migration premise - "WorkRequest/WorkCycle are NOT the live stage-2
driver; renamed entities driven synthetically in shadow" - is CONTRADICTED BY
SOURCE. A pure paw-patrol-only, zero-behavior-change, shadow-safe rename PR is
therefore not achievable; escalated to the team-lead/owner for an A-vs-B ruling
before any rename surgery.
**Came up because:** the reference survey went repo-wide (not just paw-patrol) and
found WorkCycle/WorkRequest are live-driven by two OTHER deployable units.
**Evidence (verified in source):**
- crates/paw-codex-worker (compiled prod worker): `temper_api.rs:111`
  `config.entity_url("WorkCycles", work_cycle_id)`; `fetch_work_cycle_until_review_passed`
  polls WorkCycle (`temper_api.rs:123-137`); `event_loop.rs` claims queued WorkerRuns
  and drives the cycle; posts `work_cycle_id` / `Fail`. ~70 refs.
- dashboard/ (Svelte): `stores/workcycles.ts` `queryEntities('WorkCycles', ...)` +
  `getEntity('WorkCycles', id)`; `app-views/paw-patrol.ts` addresses the `WorkCycles`
  set and `WorkCycleId` columns + relation graph.
- CSDL: `<EntitySet Name="WorkCycles" EntityType="TemperPaw.Patrol.WorkCycle"/>`
  (+ WorkRequests). Worker + dashboard address the OData SET names; a clean rename
  makes the set `Efforts` and 404s the live worker / blanks the dashboard.
- Also independent, MUST-NOT-TOUCH WorkCycle concepts in other apps:
  `os-apps/paw-harness/specs/work_cycle.ioa.toml` (+ its cedar/csdl) and
  `reference-projects/deep-sci-fi/dsf-harness/specs/dsf_work_cycle.ioa.toml` - separate
  entities, out of ARN-441 scope.
**Options:** (A) coordinated hard cutover across 3 pipelines (app publish+install,
paw-codex-worker rebuild+deploy, dashboard deploy) in a quiet window - clean end
state, but no atomic flip => breakage window for in-flight efforts, worker/dashboard
come into scope; (B) additive-then-flip - add Effort/Intent as new types for the new
lifecycle, keep WorkCycle/WorkRequest live for worker+dashboard until the phase-3
flip (same "retire AT the flip" discipline as gate_render/merge-permit/risk_rule),
no prod breakage, two names coexist only during the shadow window.
**Chose escalate-and-hold over proceeding on the recorded premise because:** the
premise is prod-breaking if wrong (a clean rename 404s the live worker and blanks
the dashboard), and the A-vs-B fix is an owner call, not an implementer default;
the cost was time-to-first-commit, the gain was correctness on a prod-breaking
scope decision. Recommended B (shadow-first, matches the rest of ARN-441) pending
the owner ruling.
**Where:** finding messaged to team-lead 2026-08-31; no code touched yet.

---

**Decision:** (OWNER RULING, team-lead 2026-08-31) Option B - additive-then-flip.
**Came up because:** the source-contradiction blocker above needed an A-vs-B call.
**Options:** (A) coordinated hard cutover of all three units (paw-patrol app +
paw-codex-worker + dashboard) in a quiet window - the rejected default of a literal
rename; (B) additive-then-flip - new Intent/Effort types now, legacy retires at the
phase-3 flip.
**Chose B because:** (1) the approved spec's spine is "nothing existing retires until
its entity twin proves out live" - A (hard 3-unit cutover with a breakage window)
contradicts the spec Rita approved; B is what the spec implies. (2) Rita's naming
ruling's REASON was "before more machinery hardcodes legacy names" - B honors it fully:
Intent/Effort are created NOW and every NEW piece speaks ONLY the domain names (zero
new legacy hardcoding); the legacy names survive only in already-deployed consumers
(worker, dashboard) and retire AT the flip, like risk_rule and the CI gates. Bounded
coexistence, not a permanent tax. (3) At flip time the retirement of
WorkCycles/WorkRequests is a normal coordinated change on by-then read-mostly
machinery - no quiet-window heroics.
**Scope for PR 1a (folds old 1a+1b into one additive PR):** CREATE Intent + Effort
types at full shape - Effort with Intended-initial six-stage lifecycle, chain-file
guards, chain reference fields, WorkerRun Heartbeat + state_timeout lease; Intent as
the renamed WorkRequest shape with Accept->Effort handoff - plus Cedar, CSDL sets
Intents/Efforts, and the wasm handlers for the NEW types (work_cycle_lifecycle logic
GENERALIZED, not moved). WorkCycle/WorkRequest specs/wasm/worker/dashboard UNTOUCHED.
A flip-time retirement note is added to the RFC Not-in-scope.
**Where:** team-lead ruling 2026-08-31; PR on claude/arn-441-entity-loop.

---

**Decision:** (lead confirm + lease refinement, 2026-08-31) Effort automaton
approved as proposed; lease timeout applies ONLY to owned states.
**Came up because:** the lease exists for orphan PICKUP, not death; pre-attachment
states have no owner, so a timeout there is noise, not orphan detection.
**Options:** for the pre-attachment states (Intended/Specified/Planned): (a) a long
state_timeout -> Stalled (the rejected default of "every state gets a lease"); (b)
allow_indefinite (no lease at all).
**Chose (b) allow_indefinite over (a) because:** it is the honest "no owner, no
lease" model, and the Intent entity's own intake timeouts already cover "nobody
picked this up"; given up automatic surfacing of a forgotten spec'd-but-unbuilt
effort (acceptable - with no owner there is nothing to be orphaned from). The owned
states Building/InReview/Proving/Merged/Deploying get state_timeout -> Stalled in 1b;
Stalled/Verified/Abandoned are allow_indefinite resting states. Timeout target is
Stalled (recoverable), never Fail/Abandoned.
**WorkCycle AwaitingHuman* pause states (where they went):** NOT dropped - the
completion-approval pause becomes step 3's Cedar-deny->MCP elicitation; a
start-approval pause, if still wanted, becomes a guard on StartBuild in a later step.
Both recorded in the RFC deferred list.
**Split (approved):** PR 1a = Intent + Effort types (full shape) + CSDL sets Intents/
Efforts + Cedar + Intent.Accept->Effort birth. PR 1b = chain-file guards +
effort_lifecycle wasm (generalized) + WorkerRun Heartbeat + state_timeout lease, with
a forced timeout->Stalled drive as proof. Both additive/create-only.
**Where:** lead ruling 2026-08-31; building 1a now.

---

**Decision:** (2026-08-31, successor - during the synthetic drive) The Effort
lifecycle Cedar permit grants the verified operator (agent_type "operator" &&
agentTypeVerified) the full Effort transition set, alongside Admin/system/supervisor.
**Came up because:** the verify-temperpaw drive authenticates with Bearer
TEMPER_API_KEY, which the platform resolves to agent_type "operator" (confirmed in
the boot log: `Intent.Accept ... agent_type:"operator"`). The first drive 403'd on
`Effort.Specify` - the initial permit only allowed Admin/system/supervisor.
**Options:** (a) broaden the permit to the verified operator; (b) find a way to drive
as a system agent (blocked - temperpaw's bearer path resolves to operator, and
"system" is never accepted from inbound headers per ADR-0157); (c) weaken to any
Agent (rejected - too broad).
**Chose (a) over (b)/(c) because:** the verified operator is the SAME trusted
sweep/verification authority ARN-434 already grants the S0/S1 record writes
(`agent_type == "operator" && agentTypeVerified == true`, so a self-declared header
agent never matches) - it is the legitimate driver of the shadow SDLC entities, and
(b) is not reachable from the operator credential. The per-gate authority flips
(steps 3-4) put stricter permits on Merge/Deploy, tightening this surface there.
Given up: nothing for the shadow phase; the operator surface narrows at the flips.
**Where:** `os-apps/paw-patrol/policies/patrol.cedar` (Effort lifecycle permit block).
