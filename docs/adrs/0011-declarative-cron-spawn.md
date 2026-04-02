# ADR-0011: Declarative CronJob → Session Spawning

## Status

Accepted

## Context

ADR-0009 replaced cron polling with `schedule_at` self-scheduling, but left the session-creation path imperative: the `cron_trigger` WASM module made three sequential HTTP calls to create, configure, and provision a Session. This duplicated the platform's existing `spawn` effect and bypassed the entity audit trail.

The Temper platform already had a `spawn` effect for declarative parent-child entity creation. What was missing was the ability to copy fields from the parent entity's state into the child's initial action params — the `copy_fields` parameter.

## Decision

### 1. Add `copy_fields` to the Temper `spawn` effect

A new optional `copy_fields` parameter on the spawn effect reads named fields from the parent entity's state and merges them into the child's initial action params:

```toml
effect = [
  { type = "spawn", entity_type = "Session", entity_id_source = "{uuid}",
    initial_action = "Configure", store_id_in = "last_session_id",
    copy_fields = "system_prompt,user_message,model,provider,tools_enabled,soul_id,sandbox_url,max_turns" }
]
```

### 2. Session.Configure auto-provisions

Session.Configure gains a `schedule` effect with `delay_seconds = 0` that immediately dispatches Provision. Any configured session auto-starts without an explicit Provision call.

### 3. Spawn on TriggerComplete, not Trigger

The CronJob flow becomes:
- `Trigger` → increment run_count, fire `cron_compute_next` WASM
- WASM computes `next_run_at` (cron parsing) and `user_message` (template substitution)
- `TriggerComplete` → platform spawns Session with `copy_fields`, schedules next Trigger via `schedule_at`

### 4. One WASM module: `cron_compute_next`

A single module handles both modes:
- **activate**: parse cron → first `next_run_at` → `ActivateComplete`
- **trigger**: parse cron → next `next_run_at` + template substitution → `TriggerComplete`

The `cron_trigger` WASM module (183 lines, 3 HTTP calls) is deleted.

## Consequences

### Positive

- Eliminates 183 lines of imperative WASM that duplicated platform capabilities
- Session creation is now visible in the platform's spawn audit trail
- Field propagation is platform-verified — no manual HTTP call construction
- One WASM module instead of two, with shared cron parsing logic
- Session auto-provision removes a common three-step dance (create → configure → provision)

### Negative

- Callers that previously called Configure then Provision will get a guard rejection on the redundant Provision call
- `copy_fields` is a new Temper platform feature that must be maintained

### Dependencies

- Requires Temper `copy_fields` on spawn effect (merged to Temper main)
- Requires Temper `schedule` effect with `delay_seconds = 0` (existing, from ADR-0012)
