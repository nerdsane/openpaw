# Proof 058: Digest-Aware Startup Reconcile And Readiness

Date: 2026-04-25

## Scope

Validated the startup/reconcile/readiness change on the merged latest-main TemperPaw tree using a local live server with a fresh file-backed Turso database, then restarted the same database to prove the warm digest skip path.

Discord was intentionally out of scope.

## Code Gates

```sh
cargo check --manifest-path /Users/seshendranalla/Development/openpaw/.worktrees/startup-readiness-latest-main/Cargo.toml -p temperpaw --locked
cargo clippy --manifest-path /Users/seshendranalla/Development/openpaw/.worktrees/startup-readiness-latest-main/Cargo.toml -p temperpaw --all-targets -- -D warnings
cargo test --manifest-path /Users/seshendranalla/Development/openpaw/.worktrees/startup-readiness-latest-main/Cargo.toml -p temperpaw startup_ --locked
cargo test --manifest-path /Users/seshendranalla/Development/openpaw/.worktrees/startup-readiness-latest-main/Cargo.toml -p temperpaw wasm_failures --locked
cargo test --manifest-path /Users/seshendranalla/Development/openpaw/.worktrees/startup-readiness-latest-main/Cargo.toml -p temperpaw-cli prebuilt_manifest_uses_extended_railway_health_window --locked
git diff --check
```

Result: all passed on the merged PR tree. The `startup_` suite contained 7 tests after merging the latest Discord startup status test from `main`.

## Cold Boot

```sh
HOME=/tmp/openpaw-startup-e2e-merged-home \
RUSTUP_HOME=/Users/seshendranalla/.rustup \
CARGO_HOME=/Users/seshendranalla/.cargo \
PORT=4492 \
TURSO_URL=file:/tmp/openpaw-startup-e2e-merged.db \
TEMPER_API_KEY=startup-e2e-key \
TEMPERPAW_WASM_STARTUP_POLICY=load-only \
OTEL_ENABLED=false \
RUST_LOG=info,temperpaw_server::startup=debug \
./target/debug/temperpaw-server
```

Observed:

```text
phase_6b_os_app_reconcile complete elapsed_ms=10135
Temper Paw listening on port 4492
startup: time to ready elapsed_ms=10797 tenant=default
healthz=200
readyz=200
readyz_body={"status":"ready","healthz":"/healthz","discord":{"status":"disconnected","configured":false,"connected":false}}
```

The final cold boot did not log required WASM module failures. After merging latest `main`, `paw-channels` gained the required `transport_reconcile` module; a prior failed attempt without that artifact correctly kept `/readyz` at 503 and exited with:

```text
Startup OS app reconcile failed for 1 app(s): paw-channels: required WASM module(s) failed to load or validate: transport_reconcile
```

## Warm Restart

```sh
HOME=/tmp/openpaw-startup-e2e-merged-home \
RUSTUP_HOME=/Users/seshendranalla/.rustup \
CARGO_HOME=/Users/seshendranalla/.cargo \
PORT=4492 \
TURSO_URL=file:/tmp/openpaw-startup-e2e-merged.db \
TEMPER_API_KEY=startup-e2e-key \
TEMPERPAW_WASM_STARTUP_POLICY=load-only \
OTEL_ENABLED=false \
RUST_LOG=info,temperpaw_server::startup=debug \
./target/debug/temperpaw-server
```

Observed all startup apps skipped unchanged reconcile:

```text
Skipped unchanged OS app app=paw-fs
Skipped unchanged OS app app=katagami-commons
Skipped unchanged OS app app=paw-agent
Skipped unchanged OS app app=paw-research
Skipped unchanged OS app app=katagami-curation
Skipped unchanged OS app app=paw-channels
phase_6b_os_app_reconcile complete elapsed_ms=653
startup: time to ready elapsed_ms=1314 tenant=default
healthz=200
readyz=200
readyz_body={"status":"ready","healthz":"/healthz","discord":{"status":"disconnected","configured":false,"connected":false}}
```

## Result

The local live E2E confirms:

- `/readyz` does not go green before required app WASM is usable.
- cold startup reconciles each startup app once in dependency order.
- warm restart uses durable bundle digests to skip unchanged startup apps.
- Railway health checks can safely point at `/readyz` instead of `/healthz`.
