# 047: Discord Reconnect, Startup Fallback, And Setup Secret Lockdown

Date: 2026-04-15

## Goal

Fix the production-facing Discord setup path end-to-end:

- setup secret reads must not be anonymously exposed
- saving Discord credentials in Settings must be enough to apply them
- Discord reconnects must not poison future reconnect attempts
- startup must continue even if Discord cannot reach `READY`
- Railway deploys must actually ship the code in this workspace

## Root Cause Summary

Three separate issues were interacting:

1. Discord save/connect behavior
   - the dashboard saved `discord_*` secrets without reconnecting the live transport
   - users had to know to click a separate `Connect` button

2. Discord reconnect lifecycle
   - failed connect attempts leaked the internal webhook listener on port `3488`
   - later reconnect attempts then failed with `Address already in use`

3. Deployment mismatch
   - Railway was configured with `Dockerfile.deploy`
   - `Dockerfile.deploy` only pulled `ghcr.io/nerdsane/openpaw:edge`
   - `railway up` therefore redeployed the upstream image instead of the local repo
   - local fixes could pass verification without ever reaching production

## Code Changes

### Auth and secret exposure

`crates/openpaw/src/auth.rs`

- narrowed anonymous bootstrap access to:
  - `GET /paw/setup/status`
  - `GET /paw/setup/secrets/schema`
- setup secret reads now require auth, including before first account creation

### Discord save-time reconnect

`crates/openpaw/src/setup_api.rs`

- saving `discord_bot_token`, `discord_public_key`, `discord_guild_id`, `discord_feed_channel_id`, or `discord_forum_channel_id` now computes the effective config and reconnects immediately when the config is complete
- invalid reconnect attempts return `400`
- failed reconnects do not persist the bad secret

### Honest Discord readiness

`crates/openpaw/src/transport_manager.rs`

`crates/paw-transport/src/discord/transport.rs`

- Discord status stays `Connecting` until the gateway emits `READY`
- the spawned transport task handle is now retained and awaited on disconnect/reconnect
- reconnect/disconnect now fully tears down prior state instead of leaving a detached task behind

### Webhook listener cleanup

`crates/paw-transport/src/discord/transport.rs`

`crates/paw-transport/src/slack/transport.rs`

- introduced a `WebhookListenerGuard` that owns:
  - the listener port
  - the shutdown channel
  - the listener task handle
- when the transport exits, the webhook listener is shut down gracefully and the port is released

### Startup fallback

`crates/openpaw/src/startup.rs`

- Discord startup connect failures are now logged and dropped instead of aborting server startup
- the server continues booting without Discord if the stored token is stale, invalid, or the gateway never reaches `READY`

### Dashboard UX

`dashboard/src/routes/settings/+page.svelte`

- save success copy now reflects automatic Discord reconnect
- Settings now explains that saving Discord credentials applies them immediately
- `Connect` is positioned as manual retry only

## Red-Green Verification

### Backend focused tests

Passed:

```bash
cargo test -p openpaw --bin openpaw-server discord_secret_update_
cargo test -p openpaw --bin openpaw-server startup_discord_connect_result_
cargo test -p openpaw --bin openpaw-server safe_setup_metadata_routes_are_public_only_before_first_account
cargo test -p openpaw --bin openpaw-server setup_secret_routes_require_auth_even_before_first_account
cargo test -p paw-transport webhook_listener_guard_releases_port_on_drop
```

Observed:

- `discord_secret_update_builds_reconnect_params_when_config_is_complete ... ok`
- `discord_secret_update_skips_reconnect_when_required_values_are_missing ... ok`
- `startup_discord_connect_result_keeps_success ... ok`
- `startup_discord_connect_result_drops_failure ... ok`
- `safe_setup_metadata_routes_are_public_only_before_first_account ... ok`
- `setup_secret_routes_require_auth_even_before_first_account ... ok`
- `webhook_listener_guard_releases_port_on_drop ... ok`

### Frontend verification

Passed:

```bash
cd dashboard
npm run check
```

Observed:

- `svelte-check found 0 errors and 0 warnings`

### Full binary test note

`cargo test -p openpaw --bin openpaw-server` still has one unrelated pre-existing failure:

- `startup::tests::startup_os_apps_only_include_core_apps`
- current repo includes additional startup apps beyond the historical expected set

## End-To-End Verification

### Local bootstrap auth on a fresh server

Started a fresh isolated server on port `4315` with a clean temp `HOME` and no Discord credentials.

Verified:

```http
GET /paw/setup/secrets
```

Response:

- `HTTP/1.1 401 Unauthorized`

This confirms the full server now blocks anonymous setup secret reads even before the first account exists.

### Local save-time reconnect validation

Started an isolated server on port `4312`.

Verified:

1. Save only `discord_public_key`
   - response: `200`
2. Save a bogus `discord_bot_token`
   - response: `400`
   - error included: `Saved value would not produce a working Discord connection`
3. Confirm the bad token was not persisted
   - authenticated `GET /paw/setup/secrets/discord_bot_token` returned `404`
4. Confirm the earlier good value remained
   - authenticated `GET /paw/setup/secrets/discord_public_key` returned `200`

### Local startup fallback

Started an isolated server on port `4313` with:

- `DISCORD_BOT_TOKEN=definitely-not-a-real-token`
- `DISCORD_PUBLIC_KEY=public-key-for-test`

Observed log:

```text
ERROR openpaw_server::startup: Discord transport failed during startup; continuing without Discord error=Timed out waiting for Discord to reach READY
```

Verified:

```http
GET /healthz
```

Response:

- `HTTP/1.1 200 OK`

### Local reconnect lifecycle regression test

Started another isolated server on port `4314`, registered a local account, and hit the live Discord connect endpoint twice in a row with a fake token.

First request:

```http
POST /paw/transports/discord/connect
```

Response:

- `HTTP/1.1 400 Bad Request`
- `Gateway bot endpoint returned 401 Unauthorized`

Server log:

```text
[discord] Webhook listener on port 3488
[discord] Failed to fetch application ID: GET /applications/@me returned 401 Unauthorized
```

Second request, same invalid payload:

- `HTTP/1.1 400 Bad Request`
- same `401 Unauthorized` error

Critical observation:

- the second attempt did **not** fail with `Address already in use`
- the listener port was successfully reused
- reconnect failure is now isolated to the actual Discord auth error

## Production Investigation

### Original production Discord failure

Datadog / Railway logs previously showed repeated reconnect failures:

```text
2026-04-15T20:09:04Z Discord transport error: Failed to bind webhook listener: Address already in use (os error 98)
2026-04-15T20:09:10Z Discord transport error: Failed to bind webhook listener: Address already in use (os error 98)
2026-04-15T20:09:17Z Discord transport error: Failed to bind webhook listener: Address already in use (os error 98)
2026-04-15T20:09:40Z Discord transport error: Failed to bind webhook listener: Address already in use (os error 98)
2026-04-15T20:09:56Z Discord transport error: Failed to bind webhook listener: Address already in use (os error 98)
```

This was the concrete reason the bot could remain offline even when reconnects were attempted.

### Post-fix Discord readiness

Datadog later showed clean connects:

```text
2026-04-15T20:43:35Z Discord transport: connecting (tenant=default)...
2026-04-15T20:43:35Z Discord transport ready

2026-04-15T20:50:07Z Discord transport: connecting (tenant=default)...
2026-04-15T20:50:07Z Discord transport ready
```

This confirms the webhook-listener leak was fixed in the running production image.

### Production auth mismatch discovery

Even after local auth fixes, production still returned:

```http
GET /paw/setup/secrets -> 200
GET /paw/version -> 401
```

That inconsistency led to the deploy-path audit.

## Deployment Findings

### Earlier deployments

Triggered:

```bash
railway up -d -m "Fix Discord reconnect-on-save and lock setup secrets"
```

Deployment id:

- `d295e7d8-6d6c-4a5c-b77f-55a040262d35`

Triggered:

```bash
railway up -d -m "Lock setup secret reads and keep Discord startup non-fatal"
```

Deployment id:

- `a5344cae-a12a-417f-882d-54f7c92698ec`

### Why those deploys did not ship the local fixes

At the time, Railway was configured with:

```dockerfile
FROM ghcr.io/nerdsane/openpaw:edge
```

So `railway up` only redeployed the upstream edge image.

### Deployment architecture clarification

`DEPLOYMENT.md` and `docs/adrs/0029-deployment-architecture.md` make the intended deployment path explicit:

- Railway should pull pre-built GHCR images
- Railway should not build the Rust workspace from source
- the correct way to ship code fixes is to update the GitHub image pipeline, not to change Railway into a source builder

A temporary local change to `Dockerfile.deploy` was made during investigation and then reverted after reviewing the deployment docs.

## Remaining Verification

The final production verification to re-run after deployment `198ed9aa-16c0-4a94-9bd1-75eaf8e05f51` finishes:

1. `GET /paw/setup/secrets` should return `401`
2. Datadog / Railway logs should show Discord reaching `ready`
3. Discord app presence should reflect the connected bot identity

## Artifacts

- Code:
  - `crates/openpaw/src/auth.rs`
  - `crates/openpaw/src/setup_api.rs`
  - `crates/openpaw/src/startup.rs`
  - `crates/openpaw/src/transport_manager.rs`
  - `crates/paw-transport/src/discord/transport.rs`
  - `crates/paw-transport/src/slack/transport.rs`
  - `dashboard/src/routes/settings/+page.svelte`
- Proof:
  - `.proofs/047-discord-reconnect-and-setup-secrets-lockdown.md`
