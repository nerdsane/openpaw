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

---

**Decision:** (#491 panel round 1, 2026-08-31) Split the Effort Cedar permits: birth
(create + Seed) ONLY for patrol-intake-service; the lifecycle permit drops Seed.
**Came up because:** both reviewers found the lifecycle grant let verified operators
Seed, bypassing the birth rule (an Effort is born only by accepting an Intent).
**Options:** (a) keep one combined permit (rejected - it lets a lifecycle driver
conjure Efforts); (b) split birth from lifecycle.
**Chose (b) because:** it makes the birth rule hold in policy, not just convention.
Also dropped the Effort.Seed-vs-Seed dual-id hedge (the live drive log shows the bare
`Seed` fires: temper.action "Seed"), and hoisted `principal has agent_type` before
every == (schema-less Cedar must deny-on-missing explicitly). Given up: nothing.
**Where:** patrol.cedar Effort lifecycle + birth permits; verified by a live negative
drive (operator POST /tdata/Efforts -> 403).

---

**Decision:** (#491 panel round 1) Chain-id collections are real `list` fields, not
JSON-in-a-string.
**Came up because:** both reviewers flagged review_run_ids/proof_packet_ids/
adjudication_ids as JSON-blob strings - an entity-first violation (outside code must
parse/rewrite blobs).
**Options:** (a) keep JSON strings; (b) real list fields appended per-link via
list_append.
**Chose (b) because:** the kernel has a real list type + list_append effect (ARN-92:
the kernel takes real arrays). The attach actions append one element each; CSDL is
Collection(Edm.String). Verified live: review_run_ids reads back as ["rev-1"] (a JSON
array, not a string). Given up: nothing. **This decision flows into 1b's chain-file
guards** (they read the lists, not blobs).
**Where:** effort.ioa.toml (3 list fields + list_append on AttachReviewRun/
AttachProofPacket/Resume), model.csdl.xml (Collection).

---

**Decision:** (#491 panel round 1) pm_issue_id has a single source - the Intent -
and is dropped from the Effort row.
**Came up because:** pm_issue_id was copied to the Effort at birth, but Intent.
LinkPmIssue can set it AFTER Accept, so the copy goes stale.
**Options:** (a) copy at birth (stale); (b) add an Effort update path when the Intent
links later (more machinery + the Intent->Effort reverse-ref problem); (c) drop the
copy, read via intent_id.
**Chose (c) because:** at Accept the pm_issue_id is not even set yet (LinkPmIssue is
Accepted->Linked, after Accept), so the birth copy is premature AND stale-prone;
reading it from the Intent via intent_id is the single source. Given up: pm_issue_id
is not directly on the Effort row (one join via intent_id) - acceptable, the Intent
IS the Linear mirror holder.
**Where:** effort.ioa.toml (dropped field + Seed param), model.csdl.xml, intent params_from.

---

**Decision:** (#491 panel round 1) A failed Effort birth is recoverable via a
RebirthEffort repair action; intent.md is referenced at birth; AttachSignal is a
self-loop.
**Came up because:** three reviewer findings - (2) Accept commits before the birth
trigger with no repair path; (5) the birth did not reference intent.md (an acceptance
criterion); (6) AttachSignal from Triaged regressed the Intent to Submitted.
**Options:** (2) entity-trigger on_failure (uncertain support) vs a repair action;
(5) copy the ref vs attach the file; (6) self-loop vs split per-from-state.
**Chose:** (2) a RebirthEffort action (Accepted->Accepted) carrying the same create
trigger - a supported, explicit repair (semantics: use only when no Effort exists for
the Intent, so it cannot double-birth); (5) copy intent_ref through Seed (guaranteed
at birth, verified live); (6) drop AttachSignal's `to` so it stays in the current
state. Given up: nothing.
**Where:** intent.ioa.toml (RebirthEffort, intent_ref, AttachSignal self-loop),
effort.ioa.toml (intent_ref), paw_patrol_foundation.rs (birth-wiring test - the
"take it" consider).

---

**Decision:** (#491 panel round 2, 2026-08-31) Close the second birth door with a
Cedar `forbid`, remove Effort from the generic system-agent permit, and restrict
RebirthEffort to the intake service; widen RebirthEffort to be reachable from Linked.
**Came up because:** round 2 found (1) the generic system-agent permit granted
`create` on Effort - a second birth door; (2) RebirthEffort was callable by any
Agent; (4) RebirthEffort was unreachable after LinkPmIssue->Linked.
**Options:** (1) narrow the generic permit vs add a forbid - the lead said forbid
beats permit, use it if the permit can't be narrowed cleanly; (4) widen from-states
with an explicit `to` (regresses Linked->Accepted) vs a self-loop (no `to`, stays put).
**Chose:** BOTH narrow AND forbid for (1) - removed Effort from the generic list AND
added `forbid(create/Seed on Effort) unless patrol-intake-service`, so every door
(Admin blanket, system, future strays) is closed; verified live (operator POST
/tdata/Efforts -> 403, birth via Accept still works). (2) moved RebirthEffort into an
intake-service-only permit. (4) self-loop (dropped `to` and the trigger's optional
`to_state`) so RebirthEffort stays in Accepted/Linked and is reachable from both.
Given up: nothing.
**Where:** patrol.cedar (forbid + permits), intent.ioa.toml (RebirthEffort self-loop).

---

**Decision:** (DESIGN POINT to owner, #491 round 2) The idempotency guard for
RebirthEffort (#2) and the intent_ref-non-empty enforcement (#3) need the
wasm-validated-guard pattern; recommend implementing them in 1b's effort_lifecycle
wasm, not 1a.
**Came up because:** both fixes hit kernel mechanism limits (verified in source):
kernel ACTION guards have NO string check (only Always/StateIn/CounterMin,Max/
BoolTrue,False/ListContains/ListLengthMin/CrossEntityStateIn/And - temper-jit
table/guard.rs), so "effort_id != ''" / "intent_ref != ''" cannot be a kernel guard;
and the entity-trigger (resolve_target=create) cannot write the created id back to
the source (only the `spawn` effect's store_id_in can, but spawn's copy_fields copies
same-named parent STATE fields and cannot map the Intent's `Id` to Seed's intent_id).
**Options:** (a) switch birth to `spawn` (gets effort_id back-ref) + solve the
intent_id mapping somehow; (b) enforce both via a wasm that validates (query-before-
create for idempotency; non-empty for intent_ref) - the SAME pattern 1b's
effort_lifecycle wasm already uses for spec.md/plan.md; (c) a kernel change (out of
scope).
**Chose (b), recommend to 1b because:** the Cedar/declarative hardening that closes
the SECURITY-critical open door (#1 forbid, #2 permit, #4) lands in 1a NOW; the
wasm-validated integrity guards (idempotency, non-empty) are the identical pattern to
1b's chain-file guards and belong with them, keeping 1a declarative. Pending owner
adjudication (round 3 residual).
**Where:** recommendation messaged to team-lead; guards to land in 1b effort_lifecycle wasm.

---

**Decision:** (1b drive, owner ruling A, 2026-08-31) Add a narrow verified-operator
File-create grant to paw-fs Cedar (CROSS-APP touch) so the shadow SDLC drive can
create the Ready chain Files the Effort guards require.
**Came up because:** the chain-file guards (Specify/Plan/Seed) require Ready
intent.md/spec.md/plan.md paw-fs Files; the verify-temperpaw drive authenticates as
the verified operator, which paw-fs File Cedar does not let create Files (only
agent_type "system" or a workspace-matched agent). Because intent.md is validated at
BIRTH, nearly the whole positive-path drive was blocked.
**Options:** (A) a narrow verified-operator File-create grant in paw-fs (same
shadow-authority justification as the accepted Effort lifecycle grant); (B) drive
File creation as a system principal (no reachable path - "system" never accepted from
headers); (C) prove only the negative/refusal paths + cascade.
**Chose (A) because:** the drive identity IS our governed operator, and A unblocks the
FULL proof (positive path + idempotency block + forced timeout->Stalled), not just the
refusals. Scoped to the NARROWEST surface (Create + StreamUpdated on File only,
verified-operator predicate with has-guards). Given up: a cross-app Cedar touch - it
lives in THIS PR, gets flagged in the panel intent as such, and is FLIP-RELEVANT (at
the phase-3 flip chain files come from intake/worker principals, not the drive
operator - revisit/remove this grant then).
**Where:** os-apps/paw-fs/policies/file.cedar (verified-operator File write permit).

---

**Decision (P0 FINDING, surfaced to owner, 2026-08-31): the #2/#3 cross-entity
machinery boot-crashes temperpaw; awaiting owner A/B/C ruling before the drive.**
**Came up because:** booting 1b to drive it, temperpaw stack-overflows at startup
(`tokio-rt-worker overflowed its stack; fatal runtime error: stack overflow`).
**Diagnosis (exhaustive):** git-bisect via clean full checkouts, log-verified for
"stack overflow" (NOT healthz - healthz answers transiently before the crash):
lease-only (3c533d591) and chain-file-guards (1f9215875) boot STABLE; the #2/#3
commit (60bb97c9c) crashes. RUST_MIN_STACK=64MB does NOT help -> genuine INFINITE
recursion (cycle), not depth. The composite cascade PASSES (tolerates cycles); the
RUNTIME reaction-graph builder does not. The cyclic topology: Intent.Accept/
RebirthEffort -> Effort.Seed, Effort.Seed -> Intent.LinkEffort (back-ref),
cross_entity_state guard Intent->Effort, + a wasm trigger on the create-dispatched
Seed. A real temper gap worth its own issue (verifier accepts cyclic cross-entity
trigger graphs the runtime cannot build).
**Options:** (A) ship 1b CORE now - lease + Specify/Plan chain-file guards (both
boot-stable) + intent.md enforced at Specify (not the create-dispatched Seed);
defer #2/#3 idempotency to a follow-up once the cyclic-graph issue is fixed. (B)
redesign #2/#3 acyclic (wasm query-before-create, no back-ref/guard cycle). (C) fix
the temper reaction-graph recursion first, keep the current design.
**Recommended:** A. Pending owner ruling; nothing lost (all committed/pushed).
**Where:** temperpaw #492 (60bb97c9c); boot logs in scratchpad/boot-*.log.

---

**Decision:** (owner ruling A executed, 2026-08-31, successor 2) Strip the #2/#3
cyclic machinery from 1b and enforce intent.md at the Specify door instead;
RebirthEffort's idempotency is DEFERRED A SECOND TIME (reason: ARN-448).
**Came up because:** the P0 finding above - commit 60bb97c9c boot-crashes temperpaw
via a cyclic cross-entity trigger graph (ARN-448), and the owner ruled A (ship 1b
core, defer #2/#3).
**Options:** (A) ship 1b core - lease + Specify/Plan chain-file guards + intent.md
enforced at Specify (all boot-stable); defer #2/#3 idempotency until ARN-448 is
fixed. (B) redesign #2/#3 acyclic now (a wasm that query-before-creates, no
back-ref/guard cycle) - more machinery on an unfixed kernel gap. (C) fix the temper
reaction-graph recursion first (out of ARN-441 scope, blocks the whole effort).
**Chose (A) because:** it is the owner ruling, and it lands the honest guarantee
(an Effort cannot leave Intended without a Ready intent.md) with zero cyclic
topology - the Specify arm validates BOTH intent_ref and spec_ref. intent.md is an
at-birth criterion, but the create-dispatched Seed cannot carry a wasm trigger
without reintroducing the exact cycle that crashes, so the earliest safe door checks
it. Given up: enforcement at the literal birth instant (an Effort can exist in
Intended with no intent.md, but it cannot ADVANCE) and RebirthEffort double-birth
idempotency (a repair action a monitor calls only when no Effort exists; the risk is
a duplicate Effort if misused, not a crash). Follow-up: the idempotency + effort_id
back-ref rides AFTER ARN-448 lands; option B (acyclic wasm query-before-create) is
the fallback design if the kernel fix is far out.
**Where:** temperpaw f5121cbf0 (reverts 60bb97c9c; effort_lifecycle Specify arm
checks intent_ref+spec_ref); RebirthEffort back to a plain repair in intent.ioa.toml.

---

**Decision (P0 RE-DIAGNOSIS, surfaced to owner/team-lead, 2026-08-31, successor 2):
the boot stack-overflow is PRE-EXISTING and cycle-independent - the ARN-448
"#2/#3 cyclic graph crashes the runtime" root-cause is refuted by base-main
reproduction.**
**Came up because:** after executing Ruling A (stripping the #2/#3 cycle, cascade +
composite green, Effort scope acyclic), temperpaw STILL boot-crashes with the
identical `tokio-rt-worker overflowed its stack / fatal runtime error: stack
overflow` signature. So the strip did not fix boot, meaning the cycle was not the
cause.
**Bisect (log-verified for "stack overflow", process-alive-to-45s, NOT healthz):**
- HEAD (the strip, f5121cbf0): overflow ~12s.
- 1f9215875 (predecessor's claimed "boot-stable" chain-file commit): overflow ~35s.
- 2b9275ec8 (BASE MAIN, parent of 1a #491, entirely pre-ARN-441): overflow ~29s.
All exit 134, identical signature. The crash reproduces with ZERO ARN-441
cross-entity cycles present -> ARN-448's premise is wrong; the predecessor's bisect
that pinned it on 60bb97c9c was confounded by timing (the crash is probabilistic;
they caught stable boots on the same commits, this session catches crashes).
**Crash locus (all three logs):** after all apps reconcile, during the PawFS
File/FileVersion reconcile cascade over the many ADR/doc Files
(file_stream_updated_creates_version -> FileVersion.Create ->
record_newest_version_on_file -> File.RecordVersion), then "actor system shutting
down", then the tokio-worker overflow. paw-fs base infra, not paw-patrol.
**Not stack-tunable (decisive):** the tokio worker stack is ALREADY an explicit 16MB
(crates/temperpaw/src/main.rs TOKIO_WORKER_THREAD_STACK_BYTES, introduced in
45147ead3 "Implement bounded-history PawFS artifact batches" - the PawFS cascade
already needed a bigger stack). A local bump to 256MB (16x) did NOT prevent the
overflow (crash at 37s after ~56k log lines vs ~6.7k at 16MB) - so this is a genuine
(timing/load-dependent) infinite reaction loop in the base PawFS reconcile, not a
deep-but-finite recursion. RUST_MIN_STACK is irrelevant because thread_stack_size is
set explicitly (that is why the predecessor's 64MB "didn't help" - NOT proof of
infinite recursion by itself). Machine was at load avg 13-19 (16 CPUs, ~16 agents).
**Options:** (A) keep retrying local boots for a quiet-window stable boot to run the
1b drive; (B) run the 1b drive on the arni-big universal computer; (C) hold 1b, file
the PawFS/runtime boot-recursion as its own issue and repoint/correct ARN-448, and
let the owner decide sequencing. Escalated a/b/c to team-lead; awaiting ruling.
**Chose escalate-and-hold over grinding/kernel-hacking because:** the fix is a
temper-runtime/PawFS change, explicitly OUT of ARN-441 scope ("any temper-kernel
change" is Not-in-scope), and the correct-root-cause matters (ARN-448 + the P0 entry
above + anything blog-bound must not claim the #2/#3 cycle crashes the kernel). The
strip itself stands: correct per Ruling A, cascade + composite green, acyclic.
**Where:** boot logs scratchpad/boot-{A-clean,1f92-bt,basemain,256mb}.log; strip at
f5121cbf0; finding messaged to team-lead 2026-08-31.

**Refinement (team-lead, 2026-08-31):** frame the ARN-448 correction precisely - the
cycle theory was right about the MECHANISM (recursive dispatch of a cascading
reaction graph overflows the worker stack) and wrong about the LOCATION (the suspect
loop is base paw-fs's File<->FileVersion choreography during boot reconcile, not the
ARN-441 #2/#3 machinery). The base-main repro removes the EVIDENCE that a cyclic
cross-entity trigger graph crashes the runtime, but does not disprove it - so "can a
cyclic cross-entity trigger graph also crash the reaction-graph builder" stays a
SECONDARY OPEN question in ARN-448, neither proven nor refuted. Ruling A STANDS: the
#2/#3 machinery is NOT re-landed mid-flight - it is deferred until boots are stable
and the cycle question is answered deliberately, not on a hunch. The 16MB stack
constant (45147ead3) is named as a stack-size band-aid over the first instance (the
no-band-aids rule); the kernel fix is iterative/queued dispatch, not a bigger stack.
