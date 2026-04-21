# ADR-0039: Orphaned Session Recovery

- Status: Proposed
- Date: 2026-04-21
- Deciders: OpenPaw maintainers
- Related:
  - ADR-0036 (session-liveness-migration-and-heartbeat-retirement.md): migrated Session to state_timeouts; this ADR patches the durability gap that migration left behind.
  - ADR-0037 (end-to-end-tracing-and-traceparent-propagation.md): the per-attempt timing logs from Fix B surface the symptom (log silence after Steer) that led to this investigation.
  - ADR-0038 (queue-depth-vs-steady-state-concurrency.md): admission caps throttle arrival; they don't cap steady-state. Once a session is in-flight, orphan risk is orthogonal.
  - temper ADR-0049 (state-entry-timeouts-and-durable-scheduler.md): declares state_timeouts as non-durable under the MVP — this ADR consumes the follow-up work that closes that gap.
  - temper ADR-0050 (mandatory-liveness-coverage.md): enforces that every non-terminal state has a timeout declaration. Complementary: this ADR addresses what happens *when* that timeout is supposed to fire but the platform state says otherwise.
  - ADR-0005 (temper-native-orchestration.md): Temper-primitives-only mandate. The three-layer fix here respects that — no out-of-band watchdogs, no poll loops.
  - `os-apps/paw-agent/specs/session.ioa.toml` (primary edit — six new Resume* actions, state_timeout reset_on updates).
  - `os-apps/paw-agent/wasm/monty_repl/src/lib.rs` (structural invariant at run() exit).
  - `os-apps/paw-channels/wasm/route_message/src/lib.rs` (stale-session detection + Resume* dispatch).
  - `crates/temper-server/src/entity_actor/actor.rs` + `crates/temper-server/src/state/dispatch/state_timeouts.rs` (Temper-side durability fix — in the nerdsane/temper companion ADR).
  - `crates/temper-store-turso/src/store/trajectory.rs` + `events.rs` (persist retry — same companion ADR).

## Context

On 2026-04-21, Session `ss-019db008-1b86-7e22-af25-3dfea0a43e84` entered `Executing` state and stayed there. Every Discord DM to the user's channel routed through `Channel.ReceiveMessage` → `ChannelSession.Resume` → `Session.Steer` cleanly (HTTP 200s, persists succeeding), but paw was silent. The session's state_timeout (300s, `on_timeout = TimeoutFail`) never fired. Incoming DMs piled up in `state.steering_messages` with nothing running to consume them.

Three defects composed to produce this class of failure:

1. **`Session.Steer` is a pure self-loop.** The action lists every non-terminal state as a valid `from`, has no `to`, and declares no `trigger` in its effect list (`session.ioa.toml:667-672`). By design, Steer's contract is "append to steering_messages while the session is actively running" — `monty_repl` reads new messages on its next turn. If `monty_repl` is not running, Steer's messages are queued forever. This is correct behaviour only when a session is always running when Steer arrives.

2. **`state_timeouts` are non-durable across actor passivation and server restart.** The Temper platform admits this in a comment at `crates/temper-server/src/state/dispatch/state_timeouts.rs:145`: *"Non-durable under the MVP — timers are lost across restarts."* The armed timer is a tokio task spawned from `arm_state_timeouts_if_needed`, which only runs inside `run_post_dispatch_effects` (i.e., after an action fires). An actor re-hydrated from snapshot into a non-terminal state with a declared timeout gets no timer — no state transition happened, so no arm-fn call. The session is in `Executing` with a 300s state_timeout on paper and no clock on actual disk or memory.

3. **No platform invariant that integrations cause a state transition.** When `Session.ProcessToolCalls` fires with `effect = [{ trigger = "run_tools" }]`, the `monty_repl` WASM runs. Today there is no code path that checks whether the invocation resulted in a follow-up action on the Session. If `monty_repl` times out, crashes, or — as happened on 2026-04-20 during the Turso write-block — its post-completion trajectory persist fails with `BLOCKED` and the dispatcher returns 409 without retrying, the entity is left in `Executing` with no forward-progress action. The platform has no signal that anything is wrong.

The 2026-04-21 incident's most likely ignition: the 2026-04-20 `Operation was blocked: SQL write operations are forbidden` Turso free-tier quota hit. A `monty_repl` invocation completed, tried to dispatch `ProcessToolCalls` or `HandleToolResults`, the event persist returned `BLOCKED`, the action failed, and the session was left in `Executing` without a follow-up. Defect #2 (non-durable timeouts) meant the 300s safety net never triggered. Defect #1 (Steer is self-loop) meant user DMs couldn't wake it up either.

Defect #3 creates orphans. Defect #2 means they never self-heal. Defect #1 means they stay invisible to user DMs. Fixing any single one is mitigation; fixing all three makes the class impossible by design.

## Decision

Adopt a three-layer recovery architecture. Each layer addresses one defect at the right abstraction level:

### Sub-Decision 1: Fast-recovery via per-state `Resume*` actions (this repo)

Add six actions to `session.ioa.toml`, one per state that requires active forward progress: `ResumeTools` (Executing → run_tools), `ResumeProvider` (CallingProvider → call_provider), `ResumeContext` (PreparingContext → prepare_context), `ResumeThinking` (Thinking → check_steering), `ResumeCompacting` (Compacting → compact_context), `ResumeSteering` (Steering → check_steering). Each is a self-loop whose effect is `increment progress_token` + `trigger <integration>` — uses only existing IOA primitives, no new spec grammar.

Extend `os-apps/paw-channels/wasm/route_message/src/lib.rs` to check `session.fields.last_progress_at` on every incoming DM. If the session is in a state with a declared driver AND `now - last_progress_at > 60s`, dispatch the matching Resume* action before dispatching Steer. A fresh session's `last_progress_at` is current, so Resume is a no-op on the happy path.

Rejected alternative: extend the IOA `Effect` enum with a `trigger_for_state` variant that dispatches different integrations depending on current state. Cleaner in theory but adds spec grammar, spec-parser work, IOA verification-cascade changes, and a Temper platform-level change — cost outweighed the benefit at six states. If the Session state machine grows past ~15 states with drivers, revisit.

### Sub-Decision 2: Durable state_timeouts via hydration hook (temper)

Addressed in the nerdsane/temper companion ADR. Summary: `crates/temper-server/src/entity_actor/actor.rs::pre_start` gains an `on_hydration_complete` step that calls a new `arm_state_timeouts_on_hydration` method on `ServerState`. The method derives `state_entered_at` from the event log (walking backwards for the most recent transition into `state.status`), computes `elapsed = now - state_entered_at`, and either (a) immediately dispatches `on_timeout` if `elapsed >= after_seconds`, or (b) arms a tokio task with the remaining budget.

This closes the gap declared in `temper-server/src/state/dispatch/state_timeouts.rs:145`. With this fix, state_timeouts survive actor passivation and server restarts — the exact correctness story ADR-0049 promised.

### Sub-Decision 3: Silent exits become structurally impossible (this repo + temper)

Two preventions and one regression guard:

- **3a (this repo):** `os-apps/paw-agent/wasm/monty_repl/src/lib.rs::run()` — wrap the run-body so any exit path that returns `Ok(())` without having dispatched a Session action returns `Err` from the WASM module. The integration's `on_failure = "Fail"` in `session.ioa.toml` then fires, transitioning Session → `Failed` rather than leaving it stuck in `Executing`. Today's seven exit paths all dispatch actions; this enforces the invariant for future edits.

- **3b (temper):** `crates/temper-store-turso/src/store/trajectory.rs` + `events.rs` — retry transient Hrana `BLOCKED` / stream errors with exponential backoff (250ms, 500ms, 1s, 2s, max 4 attempts). On exhaustion, propagate the error so the integration's `on_failure` path is taken. Idempotent per-seq via the existing event store contract, so retries are safe.

- **3c (temper):** `crates/temper-server/src/state/dispatch/effects.rs` — belt-and-suspenders detection. After an inline WASM integration returns, compare `pre_status` vs `post_status`. If unchanged AND the integration was a trigger, emit `temper_integration_silent_exit_total` counter + `warn` log with entity/integration fields. Under healthy operation this counter is permanently zero — any nonzero reading is a critical alert that 3a and/or 3b have regressed.

## Readiness Gates

- Retry `ss-019db008` reproduction: trigger a DM to a freshly-orphaned session (reproduce by forcing `monty_repl` to panic mid-invocation — `tools_enabled` contains a synthetic `force_panic` tool only enabled in staging). Expect Resume* dispatched within 2s, `run_tools` re-triggered, paw responds normally.
- IOA verification cascade (`cargo run -p temperpaw-cli -- verify`): L0 symbolic + L1 model check + L2 simulation + L3 property tests all PASS on the updated Session spec with six Resume* actions.
- DD APM trace for the recovery path: `tool.llm_call.*` spans present downstream of `Resume*` → `run_tools`, flame graph identical to normal turn shape.
- `temper_integration_silent_exit_total{service:openpaw}` = 0 over 24h normal load.
- `temper_state_timeout_fired_total{entity_type:Session}` monitor: alert on sudden spike (>3× 7d baseline) as early-warning for regression of 3a/3b.

## Consequences

### Positive
- DMs to orphaned sessions resolve within a single round-trip (not 300s+ state_timeout wait).
- Orphans self-heal without user action within 300s, even if no DM arrives.
- New orphans cannot be silently created — every path produces either a real transition or a `Failed` transition via on_failure.
- The class-of-bug is impossible by design, not merely absent by convention. Convention drift (2026-04-17 TimeoutFail.from incident, 2026-04-21 orphan incident) is replaced with structural guarantees.
- Temper platform gains durable state_timeouts — benefits every entity type, not just Session.

### Negative
- Spec size grows by six action blocks. Boilerplate, but legible.
- `route_message` WASM rebuild needed; small cost.
- Turso retry adds up to 3.75s of latency to any write that hits transient errors. Acceptable — the alternative is user-visible failure.
- Fix 3a narrows monty_repl's exit contract. Future edits must remember the invariant (the unit test catches regressions).

### Risks
- **Resume* dispatched to a session that IS actively running.** Double-dispatches a trigger when the integration is already mid-run. Mitigation: `run_tools` / `call_provider` integrations are idempotent per-seq (same event sourcing guarantees that make Turso retry safe). In practice, `last_progress_at` is refreshed by send_progress inside the running integration, so the 60s staleness check returns false and Resume isn't dispatched.
- **Hydration re-arm fires TimeoutFail immediately on every restart.** If a session's `state_entered_at` was 301s ago when the server restarted and the state has a 300s timeout, every hydration fires TimeoutFail. Expected behavior — the entity WAS overdue — but operators need to understand this isn't "the restart caused failures," the sessions were already dead. Dashboard panel labels: "Orphan-clearing TimeoutFails" vs "live TimeoutFails."
- **Persistence retry might mask real Turso capacity problems.** If Turso is hard-full (not transient BLOCKED), the retry delays the Fail by up to 4s. Acceptable; the metric `temper_turso_write_retries_total` exposes retry pressure for operator awareness.

## Non-Goals

- Automatic `on_silent_exit` declarative recovery field on Actions (deferred to follow-up once usage patterns are clear).
- Cross-entity cascade recovery (e.g., "if session A is orphaned, cancel channel B's queued messages to it").
- Changing the `Steer` action's semantics beyond what's strictly needed. Steer remains a pure self-loop — Resume* is its sibling, not replacement.
- Changing Temper's admission-control story (ADR-0051 territory). This ADR addresses in-flight orphans; admission prevents burst-arrival overload, orthogonal concern.

## Alternatives Considered

1. **Make Steer trigger run_tools unconditionally.** Rejected. Double-runs when session is actively running — monty_repl REPL state corruption, duplicate tool execution, duplicate LLM charges. The staleness check is what makes Resume* safe.

2. **Poll-based watchdog actor that scans all Sessions for staleness.** Rejected. Doesn't scale (linear in session count). Violates Temper-native primitive-only mandate (ADR-0005). Duplicates the abstraction that durable state_timeouts already provide.

3. **Remove state_timeouts entirely, rely on TCP keep-alive / integration-timeout_secs to catch hangs.** Rejected. `timeout_secs` is per-integration-invocation, not per-state-total. A session that hops through CallingProvider (600s) → ApplyingProviderResponse (60s) → Executing (300s) has 15+ minutes of budget. State-local accountability only comes from state_timeouts.

4. **Single Resume action with a new `trigger_for_state` Effect variant** (see Sub-Decision 1 body). Rejected on cost-benefit at N=6 states.

## Rollback Policy

- Revert commits in reverse of sequence: drop the E2E proof, dashboard, route_message WASM, Session spec actions, monty_repl invariant, then the temper-side commits (detection → hydration re-arm → Turso retry → ADR).
- State_timeout behavior reverts to pre-ADR "non-durable MVP" — known fragility, but backstop is in place via `ss-019db008`-class incidents being visible in the silent_exit counter before the revert.
- `Resume*` actions can be left in the spec after route_message revert — they remain valid inputs, just never dispatched. Harmless.
- No persistent-state migration required. Existing Session snapshots and event logs remain forward- and backward-compatible.
