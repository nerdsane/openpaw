# ADR-0045: Bounded Warm Restart and Digest-Aware Startup

**Status:** Accepted
**Date:** 2026-04-25
**Related:** ADR-0001, ADR-0005, ADR-0028, Temper ADR-0060

## Context

OpenPaw startup had drifted into an "alive before fully usable" shape: the
daemon could bind and report basic liveness while still paying a broad OS-app
bootstrap tax. Warm restarts replayed durable runtime state, but they also
walked the startup app DAG and could re-run APP.md, agent, skill, system-file,
ADR, seed-entity, and WASM reconcile work even when the installed app bundle had
not changed.

That made deploys unpredictable. A restart of an already-installed production
system could behave too much like a cold bootstrap, consuming CPU on idempotent
content work and delaying true readiness.

## Decision

OpenPaw startup uses Temper's bounded warm-restart contract:

1. Phase 6a performs runtime-only installed-app recovery from durable app
   metadata, plus persisted WASM and Cedar recovery.
2. Phase 6a.5 recovers runtime indexes before app reconcile only when runtime
   app recovery says a cold or changed-bundle reconcile is required. If the
   installed app set is already runtime-ready, full event/index replay is
   deferred until after readiness.
3. Phase 6b runs digest-aware app reconcile. If the installed bundle digest
   matches the bundled app and specs are available, the app is skipped; if spec
   readiness is stale but the digest matches, Temper heals runtime readiness
   metadata instead of reinstalling content.
4. Phase 7 only refreshes the reaction dispatcher after app reconcile decisions
   are known.
5. Orphan-session recovery runs before readiness only when runtime indexes were
   already recovered for a required reconcile. Otherwise it runs after readiness
   behind the deferred runtime-index recovery task.
6. Readiness remains gated on the server being truly usable; configured
   transport connection status is reported separately from mere configuration.

Production must consume prebuilt app artifacts. Local startup builds remain a
developer convenience behind `TEMPERPAW_WASM_STARTUP_POLICY=build`; they are not
part of the production boot contract.

## Consequences

Warm restart becomes bounded by runtime app recovery, WASM restore, and digest
checks. It no longer pays bulky content-bootstrap or full event/index replay
costs when app digests match.

Cold bootstrap and changed-bundle deploys still reconcile content once. That is
intentional: digest-aware startup skips only when durable metadata proves the
installed app already matches the bundled app.

The remaining startup costs are now visible by phase and by app reconcile
result. If production is still slow after this change, the next bottlenecks are
WASM restore, query/projection repair, or real changed-bundle reconcile work
rather than unconditional content bootstrap or same-bundle index replay.

## Verification

Verification for this decision requires:

- unit tests in Temper for runtime-only recovery and digest-match healing
- OpenPaw startup tests for the new runtime-recovery summary reporting
- a local cold boot followed by a warm restart on the same DB
- proof that warm restart reports installed apps as runtime-ready and skips all
  unchanged startup app reconciles by digest
- proof that same-bundle warm restart defers full runtime index and orphan
  session recovery until after readiness

The 2026-04-25 local proof is recorded in
`.proofs/2026-04-25-warm-restart-digest-reconcile-e2e.md`.

## Non-Goals

- Redesigning Discord transport bootstrap in this ADR
- Removing all runtime index replay cost
- Moving every future content reconcile to background entities in one change
