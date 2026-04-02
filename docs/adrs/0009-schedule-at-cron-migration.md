# ADR-0009: schedule_at Effect — Eliminate Cron Polling Infrastructure

## Status

Accepted

## Context

OpenPaw's cron system uses three layers of workaround to schedule recurring agent runs:

1. **Rust polling loop** (`CronTrigger` in `crates/paw-transport/src/cron/trigger.rs`, ~494 lines) — queries all active CronJob entities every 60 seconds and dispatches `Trigger` on due ones.
2. **WASM heartbeat hack** (`cron_scheduler_heartbeat`) — simulates sleeping by long-polling the `/observe/entities/.../wait` endpoint with a `__never__` filter that always times out.
3. **CronScheduler entity** (`cron_scheduler.ioa.toml`) — a per-tenant entity that bounces between `Idle` and `Checking` states to simulate deferred execution.

All of this exists because the Temper platform had no way to say "fire this action at the timestamp in this field."

Temper ADR-0012 introduced `schedule` (fixed `delay_seconds`), and Sub-Decision 3b now adds `schedule_at` — an effect that reads an ISO 8601 timestamp from an entity field and schedules an action at that time. With `schedule_at`, CronJob becomes self-scheduling: each `TriggerComplete` computes the next `next_run_at` and the platform enqueues the timer.

## Decision

### Replace polling with `schedule_at` self-scheduling

The CronJob entity uses `schedule_at` effects on `ActivateComplete` and `TriggerComplete` to schedule the next `Trigger` action:

```toml
[[action]]
name = "TriggerComplete"
from = ["Active"]
params = ["last_session_id", "last_result", "next_run_at"]
effect = [{ type = "schedule_at", field = "next_run_at", action = "Trigger" }]
```

A new `cron_activate` WASM module parses the cron expression from the `schedule` field and computes the first `next_run_at`. The existing `cron_trigger` WASM is extended to compute subsequent `next_run_at` values on each `TriggerComplete`.

### Remove all polling infrastructure

- Delete `CronScheduler` entity and its WASM modules (`cron_scheduler_check`, `cron_scheduler_heartbeat`)
- Delete the Rust `CronTrigger` polling loop from `paw-transport`
- Remove `spawn_cron_trigger()` from startup

## Consequences

### Positive

- Eliminates ~700 lines of workaround code across Rust, WASM, and IOA specs
- CronJob scheduling is now driven by the platform's timer queue — no external polling
- Exact timing: the platform fires actions at the computed timestamp, not at the next 60-second polling interval
- Self-scheduling pattern is model-checkable and visible in entity event history
- Follows ADR-0005 Temper-Native Orchestration: no Rust business logic in the scheduling path

### Negative

- Timer non-durability: same as `schedule` — timers in flight are lost on server restart. Mitigation: on replay, check if `next_run_at` passed without a corresponding `Trigger` event and re-schedule
- Cron expression parsing moves from Rust to WASM (manual parsing). The initial implementation supports simple interval patterns (`*/N * * * *`, `0 */N * * *`, `0 0 * * *`)

### Dependencies

- Requires Temper `schedule_at` effect (ADR-0012 Sub-Decision 3b) merged to `main`
