# ADR-0041: Session Hot Fields Stay Out of the Query Plane

- Status: Proposed
- Date: 2026-04-24
- Deciders: OpenPaw maintainers
- Related:
  - ADR-0026: durable query plane and bounded actor residency
  - ADR-0036: session liveness migration and heartbeat retirement
  - ADR-0039: orphaned session recovery
  - temper ADR-0058: query-plane hot-field opt-out and stable projections
  - `os-apps/paw-agent/specs/session.ioa.toml`

## Context

OpenPaw Sessions intentionally record several high-frequency operational fields:

- `last_heartbeat_at`
- `progress_token`
- `last_progress_at`

Those fields are useful for entity inspection, liveness analysis, timeout behavior, and recovery decisions. But they are poor collection-query surfaces:

- operators do not meaningfully filter session lists by exact heartbeat timestamp
- `progress_token` is a write-heavy internal counter
- each healthy turn can update these fields multiple times

Once Temper gained explicit query-plane opt-out support, leaving these fields indexed would keep paying durable field-index churn for almost no discovery value.

## Decision

The Session automaton marks these hot operational fields with:

```toml
query_indexed = false
```

Specifically:

- `last_heartbeat_at`
- `progress_token`
- `last_progress_at`

They remain first-class state fields on the Session entity and continue driving:

- timeout/reset semantics
- stale-session recovery logic
- operator inspection on direct entity reads

They no longer participate in durable collection filtering.

## Consequences

### Positive

- Session heartbeats and progress updates stop rewriting durable field-index rows for low-value fields.
- Live session loops get cheaper without weakening the Session state machine.
- Direct Session inspection remains intact, so debugging and recovery logic still see the latest values.

### Negative

- Collection queries cannot filter by those three fields anymore.

### Risks

- If future tooling starts relying on collection filtering by these fields, it will need either a different surface or the fields must be opted back in.

## Readiness Gates

- Session spec explicitly marks the three fields `query_indexed = false`.
- Temper query-plane tests prove excluded fields do not appear in `entity_field_index`.
- Live local Session E2E proves the Session still advances to completion and `entity_catalog.sequence_nr` still moves forward while those fields stay out of the index.

## Non-Goals

- Removing heartbeat/progress tracking from Sessions.
- Changing timeout or recovery semantics.
- Hiding these values from entity reads.
