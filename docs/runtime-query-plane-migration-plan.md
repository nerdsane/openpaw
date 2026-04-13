# Runtime Query Plane Migration Plan

## Goal

Re-architect OpenPaw and Temper so collection queries run on durable projections while actors remain a bounded execution cache.

This plan implements ADR-0026 in phases that can ship independently and be verified with production telemetry.

Related follow-on: [ADR-0028](./adrs/0028-bounded-startup-surface-and-wasm-artifact-contract.md) and the [startup hardening plan](./startup-hardening-plan.md) cover the remaining startup-surface and WASM artifact work that ADR-0026 intentionally does not solve by itself.

## Success Criteria

The migration is complete when all of the following are true:

- Startup no longer hydrates large numbers of actors solely for field-index rebuilds.
- Collection OData reads use durable query-plane tables by default.
- Actor count reflects live work, not total discovered entities.
- Small-instance memory usage is stable under restart and idle workloads.
- Projection correctness is verified with shadow reads before cutover.

## Current Reality

Today, the relevant behavior is:

- startup populates `entity_index` from the event store
- startup also launches field-index backfill for OData push-down
- field-index backfill hydrates actors when snapshots are missing
- transitions update the field index incrementally
- actor passivation existed in Temper but was not started by OpenPaw until the immediate fix in this branch

This means the system already has the beginnings of a query plane, but it still relies on the execution plane to repair that query plane.

## Workstreams

There are four workstreams that move in parallel but cut over in sequence.

### Workstream A: Observability and invariants

- Add metrics:
  `temper_indexed_entities`
- Add metrics:
  `temper_projected_entities`
- Add metrics:
  `temper_projection_backfill_candidates`
- Add metrics:
  `temper_projection_backfill_hydrated_fallback_total`
- Add metrics:
  `temper_actor_passivations_total`
- Add metrics:
  `temper_actor_respawns_total`
- Add metrics:
  `temper_projection_coverage_ratio`
- Add monitor and dashboard slices for actor count, projected count, and RSS by host.

### Workstream B: Durable query plane

- Add `entity_catalog` schema in Turso.
- Define a projection writer API in `temper-server` so transitions update:
  `entity_catalog`
  `entity_field_index`
- Ensure delete transitions remove rows from both structures.
- Add a backfill job that rebuilds projections from persistence only.

### Workstream C: Read-path migration

- Add shadow-read support for collection queries:
  current path vs query-plane path
- Log mismatches with enough context to debug:
  tenant, entity type, filter, count delta, missing IDs
- Cut over supported `$filter` reads to the query plane once mismatch rates are acceptable.

### Workstream D: Actor residency control

- Keep startup passivation loop enabled.
- Add explicit residency controls:
  actor idle timeout
  passivation check interval
  optional actor-count budget
  optional memory-pressure-triggered passivation
- Limit startup restore to the live set only.

## Phases

## Phase 0: Immediate Stabilization

### Purpose

Stop misleading telemetry and ensure actors can decay after boot.

### Scope

- Fix Datadog entity queries so they report the real total.
- Start the OpenPaw actor passivation loop.
- Add low-cost regression tests around startup config and Datadog config.

### Exit Criteria

- Dashboard and monitor queries report the correct entity total.
- OpenPaw build and targeted tests pass.
- Runtime has an active passivation loop on boot.

### Proof

- `cargo build -p openpaw`
- `cargo test -p openpaw startup::tests:: -- --nocapture`
- local startup smoke log showing successful boot

## Phase 1: Make Query-Plane State Explicit

### Purpose

Create the durable structures needed to stop using actors as the collection catalog.

### Scope

- Add `entity_catalog` table to Turso.
- Define projection semantics:
  one row per entity
  current status
  updated timestamp
  projection version
  snapshot sequence used, if applicable
- Add shared projection update helpers in `temper-server`.

### TDD

- Red:
  schema tests for `entity_catalog`
- Red:
  projection writer tests for insert, update, delete
- Green:
  minimal implementation

### Exit Criteria

- Every successful transition can update both query-plane tables transactionally enough for acceptable consistency.
- Deletions clean up both structures.

## Phase 2: Shadow Projections

### Purpose

Build confidence in the new query plane without changing user-facing behavior yet.

### Scope

- Write both `entity_catalog` and `entity_field_index` during normal transitions.
- Add a persistence-only backfill job:
  snapshots first
  direct event replay if needed
  no actor hydration allowed
- Record projection coverage metrics.

### TDD

- Red:
  backfill tests proving no actor hydration path is used
- Red:
  projection coverage tests
- Green:
  persistence-only rebuild implementation

### Exit Criteria

- Backfill can populate query-plane state from persistence alone.
- Metric `temper_projection_backfill_hydrated_fallback_total` remains zero.

## Phase 3: Shadow Reads

### Purpose

Compare the new collection-read path against the current path under real workloads.

### Scope

- For supported OData collection reads, execute:
  current path
  query-plane path
- Return current-path results to users.
- Log and metric mismatches.

### TDD

- Red:
  read-path equivalence tests for common `$filter` cases
- Red:
  mismatch instrumentation tests
- Green:
  shadow-read wrapper

### Exit Criteria

- Mismatch rate is low enough to explain and fix all known divergences.
- Common collection filters match between both paths.

## Phase 4: Query-Plane Cutover

### Purpose

Make durable projections the default substrate for collection reads.

### Scope

- Supported collection OData reads use query-plane lookup first.
- Unsupported filters still fall back explicitly.
- The fallback path is treated as exceptional and observable.

### TDD

- Red:
  cutover tests for supported filters
- Red:
  fallback tests for unsupported expressions
- Green:
  query-plane default read path

### Exit Criteria

- Supported collection reads no longer depend on actor hydration.
- Collection performance remains acceptable.

## Phase 5: Startup Live-Set Restore Only

### Purpose

Ensure boot restores only runnable state, not the full historical corpus.

### Scope

- Define live-set restoration categories:
  sessions in non-terminal states
  timers and schedules
  heartbeat-driven entities
  other entities with pending executable work
- Remove broad startup field-index rebuild behavior that hydrates or touches cold entities unnecessarily.

### TDD

- Red:
  startup tests proving cold entities remain unhydrated
- Red:
  recovery tests for hot entities
- Green:
  live-set-only restore implementation

### Exit Criteria

- Boot actor count is proportional to active workflows, not total entity count.
- Restart RSS on small instances stays within target budget.

## Phase 6: Cleanup and Hardening

### Purpose

Remove temporary migration machinery and harden operational guardrails.

### Scope

- Remove shadow-read comparison where no longer needed.
- Remove actor-hydrating field-index rebuild fallback entirely.
- Rename or supersede misleading metrics:
  `temper_active_entities`
- Add memory-budget-triggered passivation if still needed.

### TDD

- Red:
  regression tests ensuring collection reads do not hydrate actors
- Red:
  actor-budget/passivation tests
- Green:
  cleanup implementation

### Exit Criteria

- No remaining production path uses actors as a collection-query substrate.
- Metrics and dashboards clearly distinguish:
  indexed entities
  projected entities
  hydrated actors

## Design Constraints

- The source of truth remains the event log plus snapshots.
- The query plane is a projection, not an authority.
- The execution plane must be disposable and reconstructable.
- No startup path may hydrate actors solely to support collection filtering.
- All changes must remain Temper-native and avoid introducing imperative orchestration in OpenPaw.

## Operational Rollout

### Canary Sequence

1. Ship projection metrics only.
2. Ship dual-write projection updates.
3. Ship persistence-only backfill behind a flag.
4. Ship shadow reads.
5. Cut over read path for one entity type at a time.
6. Restrict startup restore to the live set.

### Rollback Strategy

- If projection writes regress:
  disable cutover and continue using current reads.
- If query-plane reads mismatch:
  keep shadow mode and block cutover.
- If live-set restore misses critical entities:
  restore the previous startup scope while preserving the new projection tables.

## Recommended First Implementation Slice

The highest-leverage next slice is:

1. Add metrics for projection coverage and actor hydration fallback.
2. Add `entity_catalog`.
3. Refactor projection writes into a shared helper used by transition effects.
4. Replace startup field-index rebuild fallback so it never hydrates actors.

That slice is small enough to prove the architecture without attempting the entire migration at once.
