# Type-Scoped Runtime Index Recovery Proof

Date: 2026-04-27
Branch: `codex/runtime-index-catalog`

## Temper validation

Merged prerequisite Temper PR: https://github.com/nerdsane/temper/pull/190

Commands:

```sh
cargo test -p temper-store-turso list_entity_ids_by_type -- --nocapture
cargo test -p temper-server --test ensure_entity_loaded list_entity_ids_lazy_populates_only_requested_type -- --nocapture
```

Results:

- `temper-store-turso`: 2 passed
- `temper-server ensure_entity_loaded`: 1 passed

## OpenPaw tests

Command:

```sh
cargo test -p temperpaw runtime_recovery -- --nocapture
```

Result:

- 2 passed

## Local e2e

Cold disposable DB boot used local WASM build mode to produce the required app artifacts:

```sh
TURSO_URL=file:/tmp/openpaw-runtime-index-catalog-e2e/paw.db \
PORT=4580 \
PUBLIC_BASE_URL=http://127.0.0.1:4580 \
OTEL_ENABLED=false \
TEMPERPAW_WASM_STARTUP_POLICY=build \
TEMPERPAW_QUERY_PROJECTION_BACKFILL_ON_STARTUP=false \
TEMPERPAW_ORPHANED_SESSION_RECOVERY=false \
cargo run -p temperpaw
```

Cold boot result:

- `/readyz`: 200
- `/healthz`: 200
- `/tdata/AgentRoutes?$top=5`: 200, client time `0.003563s`
- server log: `GET /tdata/AgentRoutes` latency `1ms`
- server log: `entity.populate_index_from_store_by_type` for `AgentRoute`, count `0`
- server log: no whole-tenant `entity.populate_index_from_store`
- server log: no `Deferred runtime index recovery`

Warm restart against the same DB:

```sh
TURSO_URL=file:/tmp/openpaw-runtime-index-catalog-e2e/paw.db \
PORT=4580 \
PUBLIC_BASE_URL=http://127.0.0.1:4580 \
OTEL_ENABLED=false \
TEMPERPAW_WASM_STARTUP_POLICY=load-only \
TEMPERPAW_QUERY_PROJECTION_BACKFILL_ON_STARTUP=false \
TEMPERPAW_ORPHANED_SESSION_RECOVERY=false \
cargo run -p temperpaw
```

Warm boot result:

- startup time to ready: `3193ms`
- `phase_6b_os_app_reconcile`: `2436ms`
- `/readyz`: 200 in `0.001655s`
- `/healthz`: 200 in `0.001435s`
- `/tdata/AgentRoutes?$top=5`: 200 in `0.004745s`
- server log: `GET /tdata/AgentRoutes` latency `2ms`
- server log: `entity.populate_index_from_store_by_type` for `AgentRoute`, count `0`
- server log full-index matches: `0`
- server log typed-index matches: `3`

Conclusion: startup no longer schedules whole-tenant post-ready runtime index replay, and a cold `AgentRoutes` collection read hydrates only the `AgentRoute` runtime index instead of blocking behind a full tenant event scan.
