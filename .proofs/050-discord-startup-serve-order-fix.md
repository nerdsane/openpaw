# 050 Discord Startup Serve-Order Fix

## Summary

Fixed a startup ordering bug that could leave Discord offline on Railway even while the OpenPaw service itself was healthy.

Before this change, startup reserved the HTTP listener early but did not start serving the local API until after Discord transport bootstrap completed. The Discord transport calls back into the local OpenPaw API during bootstrap to create or reuse its `Channel` entity. On a bound-but-not-yet-serving port, those requests could stall until the transport manager's 30-second startup timeout elapsed, causing OpenPaw to continue booting without Discord.

After this change, the runtime HTTP server is spawned and probed for readiness before any transport bootstrap begins.

## Code Change

- Started the Axum server before transport bootstrap in [crates/openpaw/src/startup.rs](/private/tmp/openpaw-discord-fix/crates/openpaw/src/startup.rs).
- Added:
  - `spawn_runtime_server(...)`
  - `wait_for_runtime_server(...)`
- Reused the same early-started server handle for the normal runtime path and the soul-setup path.

## Regression Test

Added:

- `startup::tests::spawn_runtime_server_accepts_requests_before_transport_boot`

This test proves the runtime HTTP server can accept requests before transport boot proceeds.

## Verification

### Automated

Command:

```bash
cargo test -p openpaw -- --nocapture
```

Result:

- 26 unit tests passed
- 3 `session_turn_architecture` tests passed

### Runtime Proof

Ran OpenPaw locally with:

- throwaway `HOME`
- `OTEL_ENABLED=false`
- `PUBLIC_BASE_URL=https://example.com`
- fake `DISCORD_BOT_TOKEN`
- fake `DISCORD_PUBLIC_KEY`

Expected behavior after fix:

- Discord transport should reach the local API immediately
- create its `Channel` entity
- then fail fast with a concrete Discord auth/bootstrap error
- not hang for 30 seconds on `Timed out waiting for Discord to reach READY`

Observed log excerpt:

```text
2026-04-16T17:32:10.199304Z  INFO openpaw_server::startup: Phase 9: Starting server...
2026-04-16T17:32:10.209897Z  INFO openpaw_server::transport_manager: Discord transport: connecting (tenant=default)...
  [discord] Webhook listener on port 3488
  [discord] Created Channel entity: en-019d9759-ae67-76f3-8725-ffa1083e19f6
  [discord] Failed to fetch application ID: GET /applications/@me returned 401 Unauthorized: {"message": "401: Unauthorized", "code": 0}
2026-04-16T17:32:10.398564Z ERROR openpaw_server::transport_manager: Discord transport error: Gateway bot endpoint returned 401 Unauthorized: {"message": "401: Unauthorized", "code": 0}
2026-04-16T17:32:10.398620Z ERROR openpaw_server::startup: Discord transport failed during startup; continuing without Discord error=Gateway bot endpoint returned 401 Unauthorized: {"message": "401: Unauthorized", "code": 0}
2026-04-16T17:32:10.398747Z  INFO openpaw_server::startup: Open Paw listening on port 61461
2026-04-16T17:32:10.398752Z  INFO openpaw_server::startup: startup: time to healthy elapsed_ms=3001 tenant=default
```

Interpretation:

- The local API was already serving during Discord startup.
- The transport reached Channel bootstrap successfully.
- The failure was a real Discord auth/bootstrap error from the fake token.
- The old 30-second self-deadlock path did not occur.

## Outcome

This removes the startup-ordering failure mode that could leave Railway deployments healthy at `/healthz` while Discord stayed offline due to transport bootstrap timing out against OpenPaw's own not-yet-serving API.
