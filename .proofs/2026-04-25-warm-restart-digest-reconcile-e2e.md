# Warm Restart Digest Reconcile Local E2E Proof

Date: 2026-04-25

## Scope

Proves OpenPaw uses the bounded Temper warm-restart path after the digest-aware
app reconcile implementation landed in Temper and was consumed by OpenPaw.

Discord was intentionally unconfigured for this local proof because Discord
transport validation is being handled separately. The important readiness check
is that the server reports Discord as unconfigured/disconnected rather than
implying a live transport from configuration alone.

## Setup

The server binary was built from this worktree:

```sh
cargo build -p temperpaw
```

Required local startup WASM artifacts were built before the E2E:

```sh
rustup +nightly-2026-02-08 target add wasm32-wasip1
bash os-apps/paw-fs/wasm/blob_adapter/build.sh
bash os-apps/paw-fs/wasm/workspace_fs/build.sh
bash os-apps/paw-research/wasm/build.sh
bash os-apps/paw-channels/wasm/build.sh
bash os-apps/paw-agent/wasm/build.sh
```

Both runs used the same clean temp DB:

```sh
HOME=/tmp/openpaw-warm-e2e-home
PORT=4490
TURSO_URL=file:/tmp/openpaw-warm-e2e.db
TEMPER_API_KEY=local-e2e-key
OTEL_ENABLED=false
TEMPERPAW_WASM_STARTUP_POLICY=load-only
RUST_LOG=info,temperpaw=debug
target/debug/temperpaw-server
```

## Cold Boot

Ready response:

```json
{"status":"ready","healthz":"/healthz","discord":{"status":"disconnected","configured":false,"connected":false}}
```

Cold boot installed the startup app surface and reconciled content as expected:

```text
Installed OS app runtime recovery complete ready=0 healed=0 needs_reconcile=0 missing_bundle=0 store_error=0 result=ready
phase_6a5_runtime_index_recovery complete elapsed_ms=2
Reconciled paw-fs ... wasm_modules: ["blob_adapter", "workspace_fs"], wasm_failures: []
Reconciled paw-agent ... wasm_modules: ["agent_reply", "capability_installer", "coding_agent_runner", "context_compactor", "context_preparer", "cron_compute_next", "emit_ots_trajectory", "monty_repl", "openai_codex_auth", "plan_approval_handler", "plan_review_feedback_handler", "provider_caller", "provider_response_applier", "request_approval", "request_plan_review", "sandbox_provisioner", "session_link_monitor", "steering_checker", "workspace_provisioner", "workspace_restorer"], wasm_failures: []
Reconciled paw-research ... wasm_modules: ["web_fetch", "web_search"], wasm_failures: []
Reconciled paw-channels ... wasm_modules: ["channel_connect", "route_message", "send_reply", "transport_reconcile"], wasm_failures: []
phase_6b_os_app_reconcile complete elapsed_ms=10076
startup: time to ready elapsed_ms=10599 tenant=default
```

## Warm Restart

The server was stopped and restarted against the same DB.

Ready response:

```json
{"status":"ready","healthz":"/healthz","discord":{"status":"disconnected","configured":false,"connected":false}}
```

Warm restart recovered installed apps as runtime-ready and skipped every
unchanged startup app by digest:

```text
Installed OS app runtime recovery complete ready=6 healed=0 needs_reconcile=0 missing_bundle=0 store_error=0 result=ready
phase_6a5_runtime_index_recovery complete elapsed_ms=4
OS app unchanged; skipping hot reconcile app=paw-fs
OS app unchanged; skipping hot reconcile app=katagami-commons
OS app unchanged; skipping hot reconcile app=paw-agent
OS app unchanged; skipping hot reconcile app=paw-research
OS app unchanged; skipping hot reconcile app=katagami-curation
OS app unchanged; skipping hot reconcile app=paw-channels
phase_6b_os_app_reconcile complete elapsed_ms=675
startup: time to ready elapsed_ms=1189 tenant=default
```

## Assertions

- Cold boot reached `/readyz`.
- Warm restart reached `/readyz` on the same durable DB.
- Runtime app recovery reported all 6 installed startup apps as `ready`.
- Runtime index recovery completed before app reconcile.
- All 6 startup apps skipped hot reconcile by matching bundle digest.
- Warm restart avoided APP.md, skill, system-file, ADR, and seed-entity
  bootstrap churn for unchanged app bundles.
