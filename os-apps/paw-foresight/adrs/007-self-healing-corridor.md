# ADR-007: Self-healing corridors

Status: Accepted
Date: 2026-06-13

## Context

The first live v2 run (six-month world, 2026-06-12/13) did not finish on its
own. Two things compounded:

1. The local **debug** server aborted twice under the searched corridor's
   session concurrency (SIGABRT, then SIGSEGV). The release build, run on the
   same load, stayed up.
2. Every server interruption **orphaned in-flight sessions**: a repairer or
   adversary session died with the process, leaving its Path stuck in
   `Solving`/`Repaired`/`Challenged` forever, and its Claim stuck in
   `Bridging`. Recovery required hand-dispatching `RequestChallenge`,
   `Fail`+`SubmitForBridge`, and `RouteSettled` from a script — dozens of
   manual calls across three salvage rounds.

The corridor entities had `allow_indefinite_states` on their wait states
(ADR-0050), so nothing re-drove a dead route. That is the gap.

## Decision

1. **Release build for all runs.** The debug build cannot take v2 concurrency;
   the proof runbook mandates `./target/release/temperpaw-server`.

2. **State-timeout re-drive on every corridor wait state.** Replace the
   `allow_indefinite_states` entries on the in-flight states with
   `[[state_timeout]]` declarations whose `on_timeout` re-spawns the dead
   work, entity-first:
   - `Path.Solving` (1200s) → `ResumeRepair` → re-spawns a repairer for the
     same route (no new Path; reuses the revision spawn path with an empty
     brief). `reset_on` `RevisionRequested`.
   - `Path.Repaired` (600s) → `RequestChallenge` → re-spawns the adversary.
   - `Path.Challenged` (600s) → `ResumeCosting` → re-runs `aggregate_costs`.
   - `Claim.Bridging` (1800s) → `ResumeBridge` → re-runs the claim decision
     (settle / alternate / unreachable). `reset_on` the progress actions.

   Budgets sit above a healthy session's wall-clock, so a *live* route is
   never double-spawned; they only fire when work has genuinely stalled. All
   re-drive actions are system-only in Cedar (timeout-fired, never a session).

3. **Crash recovery rides the hydration re-arm (ADR-0056).** On restart, an
   orphaned entity re-arms its timeout the next time it is dispatched; an
   overdue timeout then fires immediately. The release build's stability makes
   the crash case rare; the timeouts make it self-correcting rather than
   permanent. (A proactive boot sweep that dispatches every non-terminal
   corridor entity is a noted follow-up; not required given release-build
   stability.)

## Consequences

- A dead session no longer wedges a world. The route or claim re-drives on its
  own schedule; runs complete without manual salvage.
- The re-drive reuses existing spawn modules, so no duplicate Path/Claim
  entities are created (ResumeRepair spawns into the existing Path;
  RequestChallenge/ResumeCosting/ResumeBridge re-trigger pure WASM on the
  existing entity).
- Worst case on a flapping session (a route whose adversary keeps dying, so
  its `Repaired` timeout keeps re-firing `RequestChallenge`) is bounded at the
  claim level, not the route level: a single stuck Path has no per-Path cap on
  re-challenges, but `aggregate_costs::claim_decision` settles the claim on its
  cheapest acceptable route once the route budget (ADR-004, `MAX_ROUTES`) is
  spent — it does not wait on an in-flight straggler when it already has a
  good-enough answer and no budget to open a fresh alternate. So the search
  budget bounds the self-heal: a flapping route can delay, never deadlock, a
  claim that is already reachable. A claim with no acceptable route yet still
  waits for its last in-flight route (that route is its only path to
  reachable) and is marked Unreachable only when the route both terminates and
  fails to clear the bound.
