# ADR-0047: Type-Scoped Runtime Index Recovery

**Status:** Accepted
**Date:** 2026-04-27
**Related:** ADR-0026 (durable query plane and bounded actor residency), ADR-0028 (bounded startup surface), ADR-0046 (delta OS app reconcile)

## Context

ADR-0046 removed the largest blocking OS-app reconcile work from normal warm restarts, but production traces still showed post-ready `entity.populate_index_from_store` work taking hundreds of seconds. This no longer blocked `/readyz`, but it was still real background load and could still affect requests.

The slow path rebuilt the in-memory runtime entity index by asking Turso for every live `(entity_type, entity_id)` in a tenant. In production this meant a whole-tenant `SELECT DISTINCT` over the event table, commonly returning more than 12,000 entities and spending almost all time idle in the remote database call.

Worse, OData collection reads could race this background recovery. A request for a small collection such as `AgentRoutes` could see an empty in-memory index and trigger another whole-tenant recovery through `list_entity_ids_lazy`.

## Decision

Runtime index recovery is no longer a whole-tenant startup primitive.

Temper provides a type-scoped entity ID listing API. Collection reads and OS-app bootstrap helpers populate only the requested `(tenant, entity_type)` index. Turso first uses the durable entity catalog when it has rows for that type, and otherwise falls back to a type-filtered event-log query.

OpenPaw startup uses that type-scoped primitive:

- pre-reconcile recovery, when needed, warms only the startup entity surface: `App`, `Agent`, and `Soul`
- OData collection reads lazily recover only the requested entity type
- bounded orphaned session recovery warms only `Session`
- normal post-ready startup does not schedule a whole-tenant runtime index replay

## Consequences

Warm deploys should no longer emit multi-minute `entity.populate_index_from_store` or `turso.list_entity_ids` spans as routine background work.

Requests for small collections should not wait behind a full tenant replay. They may still pay a typed recovery cost on a cold process, but that cost is bounded to the requested entity type.

Large collections such as `File` can still be expensive if the durable query/catalog plane is missing or incomplete. That is a separate query-plane completeness problem, not a reason to bring back whole-tenant startup recovery.
