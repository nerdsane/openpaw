# ADR-0056: Directed Evolution Promotion Materializer

## Status

Accepted.

## Context

Directed Evolution variant generation now creates real Genesis app commits and
hot-loads them into variant-scoped tenants. Selection can choose a winner, but
the live proof showed a gap: advancing the canonical Genesis ref, publishing
the app version, and hot-loading the winner into the production tenant still
required manual operator commands.

Promotion materialization is not a reasoning task. The selector worker already
chooses the winner from evaluated evidence. The worker needs to execute bounded
external side effects and report evidence back through Directed Evolution
entities.

## Decision

The `paw-codex-worker` will handle Directed Evolution `promoter` WorkItems as a
deterministic materialization role:

- Resolve the `Promotion` target, winning `Variant`, parent `Generation`, and
  organism repository mapping.
- Use the winning variant branch/worktree and pinned app ref produced by
  variant generation.
- Push the winning commit to the canonical Genesis branch (`main` by default)
  without force-push.
- Dispatch `App.PublishNewVersion` for the canonical app.
- Dispatch `App.Install` for the configured production tenant
  (`DIRECTED_EVOLUTION_PRODUCTION_TENANT`, default `default`).
- Return structured JSON containing `canonical_app_ref`, `production_tenant`,
  `runtime_ref`, evidence, and digest so the Directed Evolution app can record
  materialization on the `Promotion`.

This role runs through the same WorkItem/WorkerRun reporting path as other
Directed Evolution worker roles, but it does not launch a background Codex
session.

## Consequences

- The Directed Evolution pipeline can complete canonical promotion without a
  human running git/Genesis commands.
- Railway is not involved in organism promotion; only Genesis hot-load is used.
- If Genesis rejects the push, publish, or install, the promoter WorkItem fails
  and the Promotion carries materialization failure evidence.
- Re-running the exact same app hash into the same tenant remains limited by
  Genesis AppInstallation idempotency; the worker treats it as a failure unless
  a later proof path can verify the canonical ref and tenant already match.

## Verification

- Unit tests cover production-tenant defaulting, promoter output shape, and
  canonical push argument construction.
- Live proof must show a selected winner automatically advancing Genesis main
  and installing into the production tenant.
