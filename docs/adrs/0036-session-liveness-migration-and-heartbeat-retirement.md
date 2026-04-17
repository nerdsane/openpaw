# ADR-0036: Session Liveness Migration and Heartbeat-Scan Retirement

- Status: Proposed
- Date: 2026-04-17
- Deciders: OpenPaw maintainers
- Supersedes: implicit contract in ADR-0025 (session-recovery-and-reset.md) that `heartbeat_scan` owns session liveness
- Related:
  - temper ADR-0048: Dispatch retry (complementary; reduces transient 500s)
  - temper ADR-0049: State-entry timeouts and durable scheduler (primitive this ADR consumes)
  - temper ADR-0050: Mandatory liveness coverage (enforces this migration)
  - temper ADR-0051: Admission control (paired capacity primitive)
  - ADR-0005: Temper-native orchestration (Temper-primitives-only mandate)
  - ADR-0022 (lazy-sandbox-provisioning.md): lazy sandbox lifecycle that Provisioning state interacts with
  - `os-apps/paw-agent/specs/session.ioa.toml` (primary edit)
  - `os-apps/paw-agent/wasm/heartbeat_scan/` (deletion)
  - `os-apps/paw-agent/wasm/heartbeat_scheduler/` (deletion)

## Context

The 2026-04-17 Katagami bulk-regenerate incident traced to two defects in the Session state machine:

1. Under concurrent load, `build_session_message` dispatched 11 Session creations in 38 seconds. Eight produced `actor dispatch failed: ask timeout after 5s` errors.
2. Two sessions entered the `Provisioning` state and never left it. `heartbeat_scan` tried to apply `TimeoutFail` but the action's `from` list (`session.ioa.toml:747`) did not include `Provisioning`, so the watchdog's dispatch was rejected silently. The sessions remained stuck until an operator applied `Fail` manually.

Defect 2 is the architectural one: Session's liveness is enforced by a second automaton (`HeartbeatMonitor`) whose contract depends on `TimeoutFail.from` covering every non-terminal state by convention, not by compilation. Conventions drift. This one drifted.

temper ADRs 0048 / 0049 / 0050 / 0051 land the platform primitives that make the class of bug impossible. This ADR is the consumer-side decision: migrate Session to those primitives and retire the legacy watchdog.

## Decision

Rewrite Session's liveness story as declarative state-timeouts. Retire the `HeartbeatMonitor` automaton and the WASM modules it needs. Narrow Cedar policies accordingly. Declare admission caps so 2026-04-17's burst pattern cannot produce stuck state.

### Sub-Decision 1: Per-state `[[state_timeout]]` declarations

Every non-terminal state in Session gets a timeout or is explicitly allowlisted:

```toml
# session.ioa.toml (excerpt)
allow_indefinite_states = ["WaitingForApproval"]
# justification: Cedar approval is human-gated; the session is correctly blocked
# on a governance decision, not a liveness failure.

[[state_timeout]]
state = "Created"
after_seconds = 60
on_timeout = "Fail"
params = { error_message = "session configure never arrived" }

[[state_timeout]]
state = "Provisioning"
after_seconds = 180
on_timeout = "TimeoutFail"
reset_on = ["ProvisionPending"]           # sandbox_provisioner progress signal

[[state_timeout]]
state = "PreparingContext"
after_seconds = 120
on_timeout = "TimeoutFail"

[[state_timeout]]
state = "CallingProvider"
after_seconds = 600
on_timeout = "TimeoutFail"
reset_on = ["Heartbeat"]                  # keeps long-streaming provider calls alive

[[state_timeout]]
state = "ApplyingProviderResponse"
after_seconds = 60
on_timeout = "TimeoutFail"

[[state_timeout]]
state = "Executing"
after_seconds = 300
on_timeout = "TimeoutFail"
reset_on = ["Heartbeat", "CheckpointToolBatch"]

[[state_timeout]]
state = "Steering"
after_seconds = 60
on_timeout = "TimeoutFail"

[[state_timeout]]
state = "Compacting"
after_seconds = 300
on_timeout = "TimeoutFail"
reset_on = ["Heartbeat"]

[[state_timeout]]
state = "Recovering"
after_seconds = 120
on_timeout = "Fail"
max_occurrences = 3                       # ~ replaces recovery_count<3 guard
```

`TimeoutFail` is extended — its `from` list now includes every state in `state_timeout` declarations. The spec compiler (per temper ADR-0049) auto-wires this so authors write the timeout once, and the synthesized `TimeoutFail` gains the state atomically.

**Why per-state values and not one global timeout**: different states have different expected durations. `Compacting` can legitimately take 5 minutes; `Steering` should finish in seconds. One-size-fits-all either fails fast or never fails.

### Sub-Decision 2: Delete `HeartbeatMonitor` and friends

Concrete removals:
- `os-apps/paw-agent/specs/heartbeat_monitor.ioa.toml` — deleted.
- `os-apps/paw-agent/wasm/heartbeat_scan/` — deleted.
- `os-apps/paw-agent/wasm/heartbeat_scheduler/` — deleted.
- Cargo workspace — remove the two deleted WASM crate entries.
- `os-apps/paw-agent/policies/session.cedar` — narrow `TimeoutFail` principal to `Scheduler` (the spec-driven durable scheduler). External callers lose the ability to dispatch `TimeoutFail` directly; they use `Fail`.

The `Heartbeat` **action** on Session stays — it is the progress signal WASM modules emit during long operations. What changes is who consumes it: `reset_on` inside the scheduler, not a separate watchdog.

**Why delete rather than retain as belt-and-braces**: two watchdogs racing to dispatch `TimeoutFail` (one per-state, one cross-session) introduces drift. The per-state watchdog is strictly more precise (it knows *which* state timed out and can choose the right target action per state). The cross-session watchdog was only ever a workaround for the missing primitive. Retain nothing that could relearn the missing-`from`-state bug.

### Sub-Decision 3: Drop `max_provision_checks` and `ProvisionPending`/`CheckSandboxReady`

These encode a per-state counter + retry loop that `[[state_timeout]]` now handles generically:

- `max_provision_checks` state var — deleted.
- `provision_check_count` counter — deleted.
- `ProvisionPending` action — deleted. (Its role as "sandbox still booting" is reexpressed as a `reset_on` trigger for the `Provisioning` state timeout; the session remains in Provisioning, the timer re-arms, the scheduler reschedules the next check.)
- `CheckSandboxReady` action — deleted for same reason.

The `sandbox_provisioner` WASM module (`wasm/sandbox_provisioner/src/lib.rs`) is simplified: it no longer reads `max_provision_checks` or manages its own retry loop. It just reports `ProvisionPending` when the sandbox isn't ready, and the state-timeout + reset_on machinery handles the retry cadence.

### Sub-Decision 4: Declare admission caps

```toml
[admission]
max_concurrent_creates = 10
max_concurrent_actions = { "Configure" = 5 }
queue_depth = 100
queue_timeout_seconds = 30
```

These values are initial — tuned from the observed burst (11 simultaneous in 38s). Post-deployment, `temper_admission_*` metrics drive the next tuning pass. Ops may override via the admin endpoint (ADR-0051 sub-decision 5) without a redeploy.

**Why cap `Configure` separately from Creates**: `Configure` triggers provisioning + context preparation, the most expensive per-session work. Cheap actions (Heartbeat, Steer) stay uncapped.

### Sub-Decision 5: `heartbeat_timeout_seconds` plumbing removal

Session state var, ADR-0037 channel transports spawn path (`paw-channels/route_message/src/lib.rs:548,846`), and any downstream config that passes the value — all removed. The value is no longer consulted.

**Why a standalone sub-decision**: the state var currently ships in every Session's entity state (`session.ioa.toml:391`) with default "300". Deleting it is a schema change; entity event logs will stop carrying it. Forward-compatible because missing fields are ignored; backward-compatible because legacy sessions aren't re-processed.

## Rollout Plan

1. **Phase 0** — temper ADR-0049 lands, durable scheduler shipped. No spec changes here.
2. **Phase 1** — Rewrite `session.ioa.toml` with `[[state_timeout]]` blocks and `[admission]`. Keep `HeartbeatMonitor` running in parallel (double-watchdog) for one deploy cycle to verify parity.
3. **Phase 2** — Delete `HeartbeatMonitor`, `heartbeat_scan`, `heartbeat_scheduler`. Narrow Cedar. Flip `TEMPER_LIVENESS_ENFORCE=true` after per-C2 ADR migrations complete.
4. **Phase 3** — Delete Katagami `submit_next_queued_regeneration` (ADR-C3 scope; not this ADR).

## Readiness Gates

- Replay of 2026-04-17 incident against a staging deployment: all 11 Sessions complete; none stuck in Provisioning; admission queue visible in Datadog.
- `temper_state_timeout_fired_total{entity_type:Session,state:Provisioning}` is zero during 24h normal load (any nonzero indicates sandbox_provisioner regression).
- `temper_actor_mailbox_full_drop_total{entity_type:Session}` ≈ 0 after admission caps engage.
- Cedar policy diff review: no external principal retains `TimeoutFail` capability.

## Consequences

### Positive
- Provisioning trap state is impossible: compiler enforces coverage (temper ADR-0050).
- One watchdog per state, co-located with the state's definition. Reviewers see the liveness commitment in one place.
- Two WASM crates and one automaton removed from the build. Less code, less divergence surface.
- Admission control prevents the 11-session burst from producing contention in the first place.

### Negative
- Spec is larger (more blocks) but easier to audit per-state.
- Operators who currently monitor `HeartbeatMonitor` dashboards must migrate to `temper_state_timeout_*` metrics (mapped in the plan's observability section).

### Risks
- **Timeout values too tight.** Especially `Executing=300s` — some real LLM tool-call batches take longer, though `reset_on = ["Heartbeat", "CheckpointToolBatch"]` covers progressing work. Mitigation: `temper_state_time_in_state_seconds{state:Executing}` p95 tracked weekly; tune value before alert-noise accumulates.
- **`reset_on` not wired for some progress signals.** If a state has a slow background step that emits no action, the timer fires even though work is progressing. Mitigation: WASM modules must emit `Heartbeat` during long operations — this is already the contract; migration just makes it load-bearing.

## Non-Goals

- Rewriting `sandbox_provisioner` WASM beyond dropping its `max_provision_checks` read.
- Replacing the `Heartbeat` action itself (stays as the progress signal).
- Renaming `TimeoutFail` to match the new semantics (stays for backward-compatibility with existing Cedar policies and logs).

## Alternatives Considered

1. **Add `Provisioning` to `TimeoutFail.from` and call it a day** — Rejected. Fixes the one state but not the class; the next trap state ships as soon as someone adds a new state without updating `TimeoutFail`.
2. **Keep `HeartbeatMonitor` alongside state-timeouts as defense-in-depth** — Rejected. Two watchdogs race on terminal actions; debugging divergences is harder than running one correct watchdog.
3. **Per-app liveness DSL** — Rejected in favor of temper-native primitive (ADR-0005 mandate).

## Rollback Policy

Revert the spec change; re-introduce the deleted WASM crates from git history; re-broaden the Cedar `TimeoutFail` policy. No persistent-state migration required — Session entities don't carry timeout-specific fields that would break replay. Event logs are forward-compatible either direction.
