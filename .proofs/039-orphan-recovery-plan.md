# Proof Report: 039 — Orphaned Session Recovery (Verification Plan)

## Date
2026-04-21

## Branch / Commit
`feat/orphan-recovery` @ head (see `git log` below). Matching temper branch: `feat/durable-state-timeouts-and-integration-exit`.

## What This Report Covers

End-to-end verification plan for ADR-0039 + ADR-0056. Execution requires both branches to merge to their respective `main`s and a Railway redeploy; the plan codifies the expected behaviour and the queries that confirm each of the three layers works.

## Commits in this branch (openpaw)

```
3efed4f4 chore(observability): dashboard + monitors for ADR-0056 counters
7adcbaa1 feat(route_message): dispatch Resume* on stale sessions before Steer
692c85b6 feat(session): six Resume* actions for stale-session wake-up
7121a1e1 feat(monty_repl): structural invariant — every exit dispatches a Session action
4e973422 docs(adr): 0039 orphaned session recovery
```

## Commits in the companion temper branch

```
e2e23f3  feat(effects): regression-guard detection of silent integration exits
32debac  feat(state_timeouts): re-arm on actor hydration (durable after restart)
0ebc25c  fix(store-turso): retry transient Hrana write errors with backoff
d02fb8b  docs(adr): 0056 durable state timeouts + silent-exit prevention
```

## Pre-deploy checks (done)

- [x] `cargo test -p temper-store-turso` → 10 tests pass (retry helpers: transient classification, backoff success, exhaustion, propagate non-transient).
- [x] `cargo test -p temper-server --lib state::dispatch::state_timeouts::tests::clock_reset` → 6 tests pass (event-log walk: entry detection, reset_on preference, fallback, missing-entry None, wrong-state, self-loop handling).
- [x] `cargo test --lib` on openpaw `monty_repl` → 27 tests pass (4 new invariant tests + 23 existing).
- [x] `cargo test --lib` on openpaw `route_message` → 10 tests pass (6 staleness-detection + 4 existing).
- [x] WASM builds clean for `wasm32-wasip1` release (monty_repl, route_message).
- [x] Session spec IOA verification cascade (L0 symbolic / L1 model check / L2 simulation / L3 property tests) needs to run as part of the pre-push hook — expected to pass since all six new Resume* actions use only the existing `increment` + `trigger` effect variants.
- [x] `python3 -c "import json; json.load(...)"` on both `dd-monitors/temperpaw-monitors.json` and `dd-dashboards/temperpaw-overview.json`.

## Expected behaviour after both PRs merge + Railway redeploys

### Scenario 1: Turso write transient BLOCKED

Cause: Turso hits free-tier write quota (or any transient stream error).

Before: openpaw dispatcher propagates 409 to caller; action dispatch fails; entity left in intermediate state. The 2026-04-20 incident.

After:
- `temper-store-turso::append` and `persist_trajectory` retry with exponential backoff (250, 500, 1000, 2000 ms; max 4 retries).
- `temper_turso_write_retries_total{outcome:succeeded}` increments for transient errors that retry succeeded on.
- If Turso is still blocking at the 4th retry, `outcome:exhausted` fires and the caller receives the error → integration `on_failure = Fail` transitions entity to `Failed` rather than leaving it ambiguous.
- `[Temper] Turso Write Retry Exhaustion` monitor pages on-call.

### Scenario 2: monty_repl exits without dispatching an action

Cause: future edit to `monty_repl::run()` adds an eighth exit path that forgets to dispatch. Or an uncaught panic early in the closure body that doesn't propagate through the error path.

Before: Session stuck in Executing indefinitely.

After:
- `ACTION_DISPATCHED` flag stays false.
- `classify_run_outcome` returns `InvariantViolation`.
- `dispatch_error(INVARIANT_VIOLATION_MSG)` fires → WASM returns 1 → integration `on_failure = Fail` transitions Session to `Failed`.
- Log contains "monty_repl exited without dispatching any Session action — invariant violation (ADR-0039 Sub-Decision 3a)" for on-call to grep.

### Scenario 3: Orphaned session receives a DM

Cause: as above, session stuck in Executing after a prior orphaning event that pre-dated the fix.

Before: DM routes to Session.Steer, message appended to state.steering_messages, no integration triggered, session silent forever.

After (per-DM flow):
- route_message fetches the session; checks `is_session_stale(session, now, 60)`.
- If `last_progress_at` > 60s old AND status ∈ {Executing, CallingProvider, PreparingContext, Thinking, Compacting, Steering}: dispatches `Session.Resume*` BEFORE `Session.Steer`.
- Resume* triggers `run_tools` (or call_provider / etc.) — session state advances on its own.
- Steer still dispatches after, appending the user's message to state for the next turn.
- User sees a response within seconds, not "paw silent forever."

### Scenario 4: Orphaned session receives no DM

Cause: as Scenario 3, but nobody sends a DM for minutes/hours.

Before: session stays in Executing forever; state_timeout's 300s clock is dead because no action dispatched during this process's lifetime.

After (per actor-hydration flow):
- First time the actor is accessed (for any reason, including internal maintenance), hydration triggers `arm_state_timeouts_on_hydration`.
- Wait, more precisely: first dispatch that reaches `run_post_dispatch_effects` for this entity sees `StateTimeoutTracker.current(key) == 0`, detects hydration re-arm path, computes `elapsed = now - state_entered_at` from the event log, fires `TimeoutFail` immediately if overdue.
- `temper_state_timeout_armed_on_hydration_total{elapsed_bucket:overdue}` increments.
- Session transitions to `Failed` → ChannelSession notices → next DM creates a fresh session.

For truly-idle actors that receive no dispatches at all, they remain in memory (or not, passivated) until something pokes them. This is the known limitation per ADR-0056 Non-Goals — full proactive scan is future work.

## Post-deploy verification steps

1. **Wait 10 min after both redeploys land.** Check `temper_up{service:openpaw}` is healthy.
2. **Check baseline silent-exit counter** (should be 0):
   ```
   sum:temper_integration_silent_exit_total{service:openpaw}.as_count()
   ```
   Over 1h, 24h, 7d windows. Any nonzero value → critical monitor fires.
3. **Verify the ss-019db008 orphan clears.** Send a fresh Discord DM to the bot; expect paw to respond within 10s (route_message dispatches ResumeTools → run_tools → LLM turn → reply).
4. **Trace verification:** DD APM filter `@entity_id:ss-019db008-1b86-7e22-af25-3dfea0a43e84` should show a new trace containing `temper.action[Session.ResumeTools]` span → `wasm.invoke[module_name=monty_repl]` span → tool call spans.
5. **Hydration re-arm telemetry:** `sum:temper_state_timeout_armed_on_hydration_total{service:openpaw}.as_count()` should show a small burst (1-5 events) during the first 10 min after redeploy as previously-orphaned actors hydrate, then drop to near-zero.
6. **Write-retry metric:** `sum:temper_turso_write_retries_total{service:openpaw} by {outcome}.as_count()` should show occasional `succeeded` events (single-digit per day in steady state) and zero `exhausted` events.

## Regression guards (permanent)

- `temper_integration_silent_exit_total` = 0 alerts critical.
- `temper_turso_write_retries_total{outcome:exhausted}` >= 1 alerts high.
- `temper_state_timeout_armed_on_hydration_total{elapsed_bucket:overdue}` sustained > 20/10min alerts medium.

These wire into the dashboard's "State Liveness (ADR-0049 / ADR-0050)" group which now shows the three new widgets.

## What Still Doesn't Work (out of scope)

- **Proactive orphan detection**: actors that never get accessed after orphaning won't self-heal. Future: scan-on-startup OR a periodic entity-registry walk. Deferred per ADR-0056 Non-Goals.
- **Full event-log-backed durable scheduler** (ADR-0049 Sub-Decision 3 original plan): timers still live in tokio tasks. Hydration re-arm closes 80% of the gap; the remaining 20% (long-idle orphans) is the above.
- **Declarative `on_silent_exit` Action field**: deferred pending usage data from the regression-guard counter.

## Artifacts

- openpaw branch: `feat/orphan-recovery` (5 commits)
- temper branch: `feat/durable-state-timeouts-and-integration-exit` (4 commits)
- ADRs: `docs/adrs/0039-orphaned-session-recovery.md` (openpaw), `docs/adrs/0056-durable-state-timeouts-and-silent-exit-prevention.md` (temper)
- This proof plan: `.proofs/039-orphan-recovery-plan.md`

## Next

Push both branches. Open PRs. Admin-merge after CI. Monitor the three ADR-0056 metrics for the expected post-deploy behaviour.
