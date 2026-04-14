# ADR-0026: Durable Query Plane and Bounded Actor Residency

**Status:** Accepted
**Date:** 2026-04-13
**Related:** ADR-0001 (Open Paw architecture), ADR-0005 (Temper-native orchestration), ADR-0025 (session recovery and reset)

## Context

OpenPaw and Temper currently blur three distinct concerns:

- **Truth plane** — event journals and snapshots in the persistence layer
- **Query plane** — collection reads and OData `$filter` execution
- **Execution plane** — in-memory actors that process commands and reactions

That coupling is now causing operational pain.

### Problem 1: Query maintenance hydrates the execution cache

Startup correctly performs cheap entity discovery by loading IDs into the in-memory `entity_index`. But it then launches `populate_field_index_from_snapshots()` as a background backfill task for OData filter push-down.

That backfill has a two-phase behavior:

1. Read a persisted snapshot if one exists.
2. If no snapshot exists, call `get_tenant_entity_state(...)`.

The fallback in step 2 hydrates an actor just to compute query metadata. This means a read-model maintenance task can allocate the full execution model in memory.

This violates the intended cache boundary. Actors should exist because work is active, not because collection filtering needs help.

### Problem 2: The field index is a projection, but it is treated like a cache rebuild

The `entity_field_index` table is an Entity-Attribute-Value projection used for OData `$filter` push-down. It mirrors top-level scalar fields plus `Status` so collection queries can be answered with SQL instead of by materializing every entity.

That is the right idea, but the rebuild path is wrong. A durable projection should be maintained incrementally and rebuilt from durable state. It should never require actor hydration to become correct.

### Problem 3: Actor residency is effectively unbounded at boot

Observed production-local telemetry from Datadog during investigation showed:

- ~19.9k active actors
- ~19.8k indexed entities in the default tenant plus ~92 in `temper-system`
- ~1.39–1.42 GB resident memory on the current `Mac` host

This indicates that nearly the entire discovered corpus was hydrated as actors. On smaller instances this is likely to cause instability or OOM pressure even while the system is mostly idle.

### Problem 4: Metrics and naming obscure the real behavior

`temper_indexed_entities` must represent the discovered query-plane corpus, not actor residency. The earlier metric naming blurred those concepts, and the old dashboard query averaged a global total with tenant-tagged series to produce a misleading value.

We need a model where:

- the durable query corpus is explicit and measurable
- actor residency is explicit and bounded
- startup only restores entities that are truly live

## Decision

OpenPaw and Temper will separate the query plane from the execution plane.

### 1. Actors are an execution cache, not the entity catalog

Actors exist to:

- process commands
- run reactions and scheduled work
- serve single-entity reads that need current executable state

Actors do **not** define the queryable corpus of entities. Collection discovery and filtering must not depend on actor hydration.

### 2. The query plane becomes a first-class durable projection

We will maintain two durable query-plane structures:

- `entity_catalog`
  Stores one row per entity with stable lookup fields such as tenant, entity type, entity ID, current status, update timestamp, and projection version.
- `entity_field_index`
  Stores the existing scalar field projection used for OData filter push-down.

Together these become the canonical collection-read substrate for Turso-backed deployments.

### 3. Startup restores only the live working set

Boot will restore only entities that must be executable immediately, such as:

- sessions in non-terminal states
- scheduled or heartbeat-driven entities
- monitors and workflows with pending timers or checks
- any other explicitly "hot" entity categories

The broader entity corpus remains cold until:

- it receives a command
- it is explicitly requested for a single-entity read
- a background maintenance job chooses to project it without hydration

### 4. Projection rebuilds must operate from persistence, never from actors

Backfill and repair jobs may use:

- event journals
- persisted snapshots
- durable projection tables

They must **not** use `get_tenant_entity_state(...)` or any equivalent actor-hydrating API as a fallback path for collection-query correctness.

If a projection cannot be rebuilt cheaply from persistence, that is a projection design bug and must be fixed in the projection layer.

### 5. Actor residency is explicitly bounded and observable

Actor passivation remains mandatory, but it is not sufficient on its own.

The runtime will evolve toward explicit residency controls:

- idle-time passivation
- actor-count and memory-pressure guardrails
- metrics for passivation rate, respawn rate, projection coverage, and entities missing snapshots

The system should be explainable in terms of:

- discovered entities
- queryable projected entities
- hydrated actors

Those counts should diverge naturally without being treated as anomalous.

### 6. OData collection reads are query-plane reads by default

For collection endpoints:

- resolve entity type
- translate supported `$filter` expressions into SQL against the query plane
- materialize only the matching entity IDs
- hydrate actors only if a later step explicitly requires executable state

Unsupported filters may still fall back, but the default path is the durable query plane.

## Consequences

### Positive

- Boot-time memory usage drops because collection-query maintenance no longer hydrates the full corpus.
- Actor counts become a meaningful signal of live work instead of total discovered state.
- Smaller instances become viable because startup no longer allocates the entire execution graph.
- OData filter performance remains strong because the query plane is preserved and clarified, not removed.
- The architecture becomes easier to reason about: truth plane, query plane, and execution plane each have a single responsibility.

### Negative

- We are introducing another durable structure (`entity_catalog`) that must be kept consistent with event application.
- Backfill and repair tooling becomes more sophisticated because it must reason from persisted state instead of simply asking actors.
- During migration, we will temporarily maintain overlapping paths: current behavior, shadow projections, and dual-read verification.

### Risks

- Projection drift could silently corrupt collection reads if incremental updates are incomplete.
- Some existing endpoints may implicitly depend on actor hydration side effects and will need to be audited carefully.
- Backends other than Turso may not support the full query-plane approach immediately, so compatibility behavior must stay explicit.

## Non-Goals

- Replacing event sourcing with row-oriented state storage
- Removing actors from Temper or OpenPaw
- Optimizing every OData function up front
- Solving cross-backend query parity in the first migration phase

## Follow-Up

Implementation sequencing is captured in:

- [Runtime Query Plane Migration Plan](../runtime-query-plane-migration-plan.md)
