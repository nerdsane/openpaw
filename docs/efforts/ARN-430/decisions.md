## Decisions & Tradeoffs

**Decision:** Extend `review_run` and `proof_packet` additively (keep their whole
existing lifecycle; add the record fields, a `Recorded/Superseded` sub-lifecycle,
and the ingest actions) instead of replacing their automata with the definitive
`Running > Recorded > Superseded` shape.
**Came up because:** the task and `stage3-spec.md`'s definitive shape describe
ReviewRun/ProofRun with a `Running > Recorded > Superseded` automaton, but that
is the later full-migration target; phase 1 is scoped "no Effort rename, no gate
changes."
**Options:** replace the automata now (rejected); additive extension (chosen).
**Chose additive because:** `work_cycle` references `review_run`/`proof_packet`
by id, `review_gate_lifecycle` wasm is fired by `review_run`'s existing actions,
and `crates/temperpaw/tests/paw_patrol_foundation.rs` asserts specific existing
actions (`ReviewRun.RequestChanges`, `ReviewRun.Escalate`) and states. Replacing
the automata would ripple into `work_cycle` and the gate wasm - both out of
phase-1 scope - and break the foundation test. AGENTS.md's rule applies: "if the
spec contradicts the repo's reality, follow the repo and record the deviation."
Given up: a single clean record automaton now; the full collapse lands with the
Effort migration (a later phase), where the gate wasm and work_cycle move too.
**Where:** `specs/review_run.ioa.toml`, `specs/proof_packet.ioa.toml`.

**Decision:** All record-shape validation (40-char commit sha; review has a
non-empty `reviewers_ran`; proof has a non-empty `changed_surface` and no
feature with `verdict=="fail"`) lives in the `record_ingest` boundary as
`parse_ok`, NOT in transition guards. The entities keep only a `record_present`
bool and an invariant that asserts it in the record states.
**Came up because:** the task asks for these as guards, but two facts about the
platform (temper-spec / temper-jit rev 43f9379) make them inexpressible as
guards: (1) the guard grammar has no string-length, regex, or per-element-array
predicate - only `state_in`, `min_count`/`max_count`, `is_true`/`is_false`,
`list_contains`, `list_length_min`, `cross_entity_state`; and (2) `build_eval_context`
builds the guard context from the entity's CURRENT state only - the incoming
action's params are NOT visible to guards, and guards are preconditions
evaluated before effects apply. So a `list_length_min reviewers_ran 1` guard on
`IngestRecord` would read the pre-ingest empty list and always fail; a guard
fundamentally cannot inspect the record being written.
**Options:** invent guard predicates + params-in-guard-context in the kernel
(rejected - out of scope, this is an app effort); split the record into `list`
fields plus a companion `counter` set via `set_counter_from_param` so an
invariant could assert `count > 0` (rejected - added redundant fields and an
artificial count, and still could not check the sha shape or per-feature
verdict); enforce everything in the ingest boundary and keep one `record_present`
invariant (chosen).
**Chose the ingest boundary because:** a record only reaches `IngestRecord` /
`IngestProof` when the module returned `parse_ok`, and `parse_ok` already
requires all four checks; so a malformed record cannot become a Recorded run.
`record_present` (set by the ingest action) plus the invariant `when [Recorded,
Superseded] assert record_present` make "a run in the record states went through
a checked ingest" a durable, expressible guarantee. Every record field is a
plain `string` (findings/features/tests as JSON), matching how `proof_packet`
already stores `proof_json` - no `list`/`counter` fields, so no first-adopter
risk on an unused field type. Given up: declarative, in-spec enforcement of the
shape - it is enforced in tested module code instead. Residual risk: someone
dispatching `IngestRecord` by hand with a bad record bypasses the module; noted
for the later phase that moves writes behind a governed (Cedar) action.
**Where:** `wasm/record_ingest/src/lib.rs` (`parse_record`, `record_shape_ok`,
`is_full_sha`); `record_present` invariant in `specs/review_run.ioa.toml` and
`specs/proof_packet.ioa.toml`.

**Decision:** Register the three new entities (`Adjudication`,
`StandingDecision`, `ShadowVerdict`) in `policies/patrol.cedar`'s Admin permit
and the any-principal read/list permit only; do not add them to the
action-bound system-agent permit.
**Came up because:** the phase-1 scope says "no Cedar", but a Temper entity with
no matching permit is default-denied and cannot be acted on at all - the same
Admin catch-all that governs every existing paw-patrol entity has to list them
or they are dead.
**Options:** leave them out of Cedar entirely (rejected - they would be
un-actionable, even in tests/e2e); add them to every permit including the
system-agent action-bound block (rejected - that block enumerates specific
actions per principal, so adding the new actions there is authorization *logic*,
which is the Cedar work phase 1 excludes); register them under the Admin +
read/list permits (chosen).
**Chose the minimal registration because:** it makes the entities governable and
readable exactly like their siblings without introducing new authorization
logic. In S0 nothing writes these entities as a system agent (record_ingest only
parses and returns; the write-back orchestration is a later phase), so the
system-agent write permits are not needed yet and are deferred to the phase that
wires their writers. Given up: system-agent write access now.
**Where:** `os-apps/paw-patrol/policies/patrol.cedar` (Admin permit + read/list
permit).

**Decision:** Implement `record_ingest` with the SDK's `temper_module!` macro
(returns a `Value`), not the manual `#[no_mangle] fn run` pattern the other
paw-patrol modules use.
**Came up because:** every existing module is a trigger that makes OData writes
via `Context::from_host`; `record_ingest` must instead be a pure parser that only
returns fields.
**Options:** copy the manual pattern (rejected - it is built around dispatching
writes, the opposite of this module); use `temper_module!` (chosen).
**Chose the macro because:** it is exactly "read context, return a Value", which
is this module's whole job, and it keeps the parser a plain unit-testable
function. Given up: visual consistency with the sibling modules. The parsing lives
in `fn parse_record(body: &str) -> ParsedRecord`, host-testable with no wasm.
**Where:** `wasm/record_ingest/src/lib.rs`.

**Decision:** Pin `record_ingest`'s `temper-wasm-sdk` to rev
`43f9379cc51545b9a47b8a28ccb202c31957a0e9` (the server's current rev), while the
existing modules are still pinned to `b0c79312...`.
**Came up because:** the task specifies the server's current rev; the other
modules have not been re-pinned since their last bump.
**Options:** match the siblings at `b0c79312` (rejected - not the server rev);
pin to `43f9379` (chosen).
**Chose 43f9379 because:** it is the rev the running server builds against, so the
committed `.wasm` matches the host ABI it will load under. Given up: a single
uniform SDK rev across the app until the others are bumped - noted as a follow-up,
not blocking, since the ABI is compatible.
**Where:** `wasm/record_ingest/Cargo.toml`.
