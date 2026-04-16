# 051: Discord Transport Loopback Auth Fix

Date: 2026-04-16

## Problem

The Railway deployment was healthy at HTTP level but Discord startup still timed out with:

```text
Timed out waiting for Discord to reach READY
```

The live container could reach both the local OpenPaw server and Discord's public endpoints, so this was not a generic network outage.

## Root Cause

`PawApiClient` uses header-based internal auth when `TEMPER_API_KEY` is absent:

- `x-temper-principal-kind: admin`

But OpenPaw's auth middleware only treats internal header-authenticated requests as pre-authenticated when **both** headers are present:

- `x-temper-principal-kind`
- `x-temper-principal-id`

That meant production transport bootstrap requests to `http://127.0.0.1:$PORT/tdata/...` were unauthorized when `TEMPER_API_KEY` was not set. The transport client also masked one of those failures by treating non-success `query_entities()` responses as an empty list.

## Red-Green Tests

Added regression tests in `crates/paw-transport/src/lib.rs`:

- `paw_api_client_without_api_key_includes_internal_admin_identity`
- `paw_api_query_entities_surfaces_non_success_responses`

Red:

```text
thread 'tests::paw_api_client_without_api_key_includes_internal_admin_identity' panicked
left: None
right: Some("openpaw-transport")
```

Green after the fix:

```text
cargo test -p paw-transport -- --nocapture
```

Result:

```text
test result: ok. 16 passed; 0 failed
```

Focused OpenPaw startup check:

```text
cargo test -p openpaw startup_discord_connect_result -- --nocapture
```

Result:

```text
test result: ok. 2 passed; 0 failed
```

## Live Production Evidence

From inside the live Railway container before the fix, loopback OData with only `x-temper-principal-kind` failed:

```text
path=/tdata/Channels status=401
path=/tdata/Channels?$filter=ChannelType%20eq%20%27discord%27%20and%20Status%20ne%20%27Archived%27 status=401
status=401   # POST /tdata/Channels
```

From inside the same live Railway container, adding `x-temper-principal-id: openpaw-transport` made those same loopback requests succeed:

```text
path=/tdata/Channels status=200
path=/tdata/Channels?$top=1 status=200
```

Independent reachability probes from the live Railway container also confirmed the broader network was healthy:

```text
GET http://127.0.0.1:8080/healthz -> HTTP/1.1 200 OK
GET https://discord.com/api/v10/gateway -> HTTP/1.1 200 OK
GET https://discord.com/api/v10/gateway/bot -> HTTP/1.1 401 Unauthorized (without bot token)
TLS connect gateway.discord.gg:443 -> success
```

## Fix

In `crates/paw-transport/src/lib.rs`:

1. Added `x-temper-principal-id: openpaw-transport` for internal header-authenticated transport calls when no bearer token is configured.
2. Changed `query_entities()` to return an error on non-success responses instead of silently returning an empty list.
3. Added modest request timeouts to the shared transport `reqwest::Client` so future hangs surface as concrete errors faster.

## Expected Outcome

With this fix deployed, the Discord transport should be able to bootstrap its `Channel` entity and proceed into the authenticated Discord startup path in production instead of failing local loopback auth silently.
