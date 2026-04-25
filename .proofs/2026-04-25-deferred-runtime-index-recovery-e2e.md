# Deferred Runtime Index Recovery Local E2E Proof

Date: 2026-04-25

## Scope

Proves same-bundle warm restart no longer blocks readiness on full runtime
index replay. This is the follow-up to the initial warm-restart proof after the
Railway deploy showed production spending 363s in Phase 6a.5.

## Setup

The server binary was rebuilt after the deferred-index change:

```sh
cargo build -p temperpaw
```

The run used the same local DB from the prior cold/warm E2E, which already had
all six startup apps installed with matching bundle digests:

```sh
HOME=/tmp/openpaw-warm-e2e-home
PORT=4491
TURSO_URL=file:/tmp/openpaw-warm-e2e.db
TEMPER_API_KEY=local-e2e-key
OTEL_ENABLED=false
TEMPERPAW_WASM_STARTUP_POLICY=load-only
RUST_LOG=info,temperpaw=debug
target/debug/temperpaw-server
```

## Result

Ready response:

```json
{"status":"ready","healthz":"/healthz","discord":{"status":"disconnected","configured":false,"connected":false}}
```

Startup evidence:

```text
Installed OS app runtime recovery complete ready=6 healed=0 needs_reconcile=0 missing_bundle=0 store_error=0 result=ready
phase_6a5_runtime_index_recovery skipped; deferring until after readiness elapsed_ms=0
Session recovery deferred until post-ready runtime index recovery
startup: time to ready elapsed_ms=2656 tenant=default
Deferred runtime index recovery scheduled after readiness tenants=2
Deferred runtime index recovery complete elapsed_ms=10
```

## Assertions

- Same-bundle warm restart reached `/readyz`.
- Runtime app recovery reported all six installed startup apps as `ready`.
- Full runtime index recovery did not block readiness.
- Orphan-session recovery did not block readiness when runtime indexes were
  deferred.
- Deferred runtime index recovery still ran after readiness.
