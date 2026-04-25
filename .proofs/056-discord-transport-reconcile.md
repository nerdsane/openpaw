# Proof Report: 056 - Discord Transport Reconcile

## Date

2026-04-24

## Branch / Commit

Branch: `codex/discord-dm-fix`

Commit: PR head for this branch

## What Was Done

- Replaced one-shot Discord startup with an entity-backed `TransportConnection` reconcile flow.
- Added `transport_reconcile` WASM integration for Discord startup retries and state reporting.
- Added transient retry around Discord transport Channel bootstrap local OData calls.
- Split Discord "configured" from "connected" in startup status, `/paw/transports/status`, and `/readyz`.
- Kept `/healthz` as liveness-only so non-Discord traffic can stay up while Discord is degraded.
- Centralized `/readyz` behind the startup readiness gate so the current main router does not register duplicate readiness routes.

## Verification Flow

1. Red test: `cargo test -p paw-transport bootstrap_channel_retries_transient_create_failure`
2. Green transport tests: `cargo test -p paw-transport`
3. Green TemperPaw API/status tests: `cargo test -p temperpaw setup_api::tests`
4. Green startup banner test: `cargo test -p temperpaw startup::tests::startup_discord_summary_distinguishes_configured_from_connected`
5. Green startup readiness gate test: `cargo test -p temperpaw startup::tests::startup_gates_keep_liveness_up_while_readiness_stays_blocked`
6. Green session lifecycle contract tests after latest main staged-turn cleanup: `cargo test -p temperpaw --test session_lifecycle_and_config`
7. Clippy: `cargo clippy -p temperpaw --all-targets -- -D warnings`
8. Rust compile check: `cargo check -p temperpaw`
9. WASM native tests: `cargo test --manifest-path os-apps/paw-channels/wasm/transport_reconcile/Cargo.toml`
10. WASM target build: `cargo build --manifest-path os-apps/paw-channels/wasm/transport_reconcile/Cargo.toml --target wasm32-unknown-unknown --release`
11. Full `paw-channels` WASM build path: `bash os-apps/paw-channels/wasm/build.sh`
12. Whitespace check: `git diff --check`
13. Rustfmt check: `cargo fmt --all -- --check` and `cargo fmt --manifest-path os-apps/paw-channels/wasm/transport_reconcile/Cargo.toml -- --check`
14. Local E2E boot:
   `PORT=45679 TURSO_URL=file:/tmp/temperpaw-discord-e2e-4.db TEMPER_API_KEY=e2e-key OTEL_ENABLED=false TEMPERPAW_WASM_STARTUP_POLICY=build RUST_LOG=info,temperpaw=debug cargo run -p temperpaw --bin temperpaw-server`
15. Local E2E HTTP checks:
   `/healthz`, `/readyz`, `/paw/transports/status`
16. Local E2E entity reconcile:
    create `TransportConnection('transport-discord')`, dispatch `Configure`, dispatch `Start`, query state.
17. Local E2E configured/degraded check:
    set local fake `discord_bot_token`, confirm `/healthz` remains 200 while `/readyz` reports 503 degraded.
18. Post-rebase startup gate boot check:
    after rebasing onto current `origin/main`, booted the server and confirmed `/healthz` and `/readyz` responses after the duplicate-route fix.
19. Railway healthcheck config check:
    `railway.toml` still uses `healthcheckPath = "/healthz"`.

## Verification Results

| Step | Expected | Actual | Status |
|------|----------|--------|--------|
| Red transport retry test | Fails before retry logic | Failed with `create Channels returned 503 Service Unavailable` before implementation | Pass |
| `cargo test -p paw-transport` | All transport tests pass | 22 passed | Pass |
| `cargo test -p temperpaw setup_api::tests` | Status, readiness, secrets, signature tests pass | 11 passed | Pass |
| Startup banner test | Configured token does not imply connected gateway | 1 passed | Pass |
| Startup readiness gate test | Liveness stays public while readiness is blocked | 1 passed | Pass |
| Session lifecycle contract tests | Latest staged-turn cleanup no longer references removed `llm_caller` source | 3 passed | Pass |
| `cargo clippy -p temperpaw --all-targets -- -D warnings` | Clippy passes with warnings denied | Finished successfully | Pass |
| `cargo check -p temperpaw` | TemperPaw compiles | Finished successfully | Pass |
| `transport_reconcile` native tests | Retry classification and URL handling pass | 2 passed | Pass |
| `transport_reconcile` wasm build | wasm32 release build succeeds | Finished successfully | Pass |
| `paw-channels` WASM build | Dockerfile channel build path still succeeds | `channel_connect`, `send_reply`, `transport_reconcile`, and `route_message` built successfully | Pass |
| `git diff --check` | No whitespace errors | No output, exit 0 | Pass |
| Rustfmt check | Root and nested WASM crate formatting pass | Both commands exited 0 | Pass |
| Local boot | Server boots with new app/spec/WASM | `/healthz` returned `HTTP/1.1 200 OK` | Pass |
| Unconfigured readiness | Discord unconfigured should not degrade service readiness | `/readyz` returned `HTTP/1.1 200 OK` and `configured:false` | Pass |
| Entity reconcile missing-token path | Entity records clear non-retryable startup failure | `Configured -> Starting -> Failed`, `last_error:"discord_bot_token is not configured"`, `attempt_count:1` | Pass |
| Status payload naming | Requested target and current entity state are separate | `desired_state:"connected"`, `connection_state:"Failed"` | Pass |
| Configured but disconnected readiness | Liveness stays up; readiness reports Discord degraded | `/healthz` returned 200, `/readyz` returned 503 with `configured:true`, `connected:false`, `connection_state:"Failed"` | Pass |
| Post-rebase `/readyz` boot | No duplicate route panic; readiness delegates to Discord status after startup | `/healthz` returned `HTTP/1.1 200 OK`; `/readyz` returned `HTTP/1.1 200 OK` with `configured:false` | Pass |
| Railway healthcheck path | Railway liveness check stays on `/healthz` | `healthcheckPath = "/healthz"` | Pass |

## What Worked

- The original local OData 503 failure class is now retried during Channel bootstrap.
- Discord startup is no longer a startup-only attempt that silently stays off after failure.
- Runtime state is visible through `TransportConnection` entity events and `/paw/transports/status`.
- `/healthz` remains liveness-only, protecting non-Discord traffic from readiness changes.
- `/readyz` now gives deployment tooling a truthful configured-but-disconnected signal.

## What Didn't Work

- A temporary E2E run exposed a liveness warning for `TransportConnection.Starting`.
- Fixed by adding a 90 second `[[state_timeout]]` that sends the entity to `StartRetry`.
- The second boot no longer reported a `TransportConnection` liveness warning; remaining liveness warnings were from unrelated pre-existing specs.
- After rebasing onto current `origin/main`, the first local boot exposed a duplicate `/readyz` route between startup gates and setup API.
- Fixed by removing `/readyz` from the setup router and having the startup-gated readiness route delegate to `setup_api::get_readyz` once startup readiness is true.

## Limitations

- Local E2E did not connect to the real Discord gateway because no production Discord token was used in the test environment.
- The exercised path did verify the internal reconcile, WASM dispatch, state transitions, missing-token failure, configured/degraded readiness, and liveness separation.
- A post-rebase `TEMPERPAW_WASM_STARTUP_POLICY=build` boot also hit an unrelated local checkout mismatch in `paw-agent/wasm/monty_repl`; targeted TemperPaw, Discord transport, and `paw-channels` checks were rerun after the rebase.

## What Still Doesn't Work

- Production deployment and live Discord DM verification are not covered by this local proof yet.
- After merge/deploy, verify Railway `/healthz`, `/readyz`, `/paw/transports/status`, and send a real Discord DM to Paw.

## Artifacts

- `crates/paw-transport/src/discord/transport.rs`
- `crates/temperpaw/src/setup_api.rs`
- `crates/temperpaw/src/startup.rs`
- `crates/temperpaw/src/auth.rs`
- `os-apps/paw-channels/specs/transport_connection.ioa.toml`
- `os-apps/paw-channels/wasm/transport_reconcile/src/lib.rs`
- `os-apps/paw-channels/policies/channels.cedar`

## Architecture Diagram

```text
Discord configured
      |
      v
TransportConnection.Configure
      |
      v
TransportConnection.Start
      |
      v
transport_reconcile WASM
      |
      v
POST /paw/internal/transports/discord/start
      |
      +--> StartSucceeded -> Connected
      |
      +--> StartRetry -----> Retrying --scheduled RetryDue--> Starting
      |
      +--> StartFailed ----> Failed
```
