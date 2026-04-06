# ADR-0010: Slack Socket Mode Transport

## Status

Accepted

## Context

OpenPaw has a working Discord transport (`crates/paw-transport/src/discord/`) that bridges Discord Gateway WebSocket events to Paw's Channel entity architecture. Users want equivalent Slack support — the ability to chat with an OpenPaw agent via Slack DMs.

The Channel entity (`os-apps/paw-channels/specs/channel.ioa.toml`) is already platform-agnostic: it accepts a `channel_type` field and uses a `webhook_url` for reply delivery. The WASM modules (`route_message`, `send_reply`) are also channel-type-agnostic — they route messages to agents and deliver replies without knowing which platform originated the message. This means adding Slack support requires only a new transport (Rust protocol bridge), with zero changes to entity specs or WASM integrations.

Slack offers two event delivery mechanisms:

1. **Events API** — Slack pushes events to a public HTTP endpoint. Requires a publicly routable URL (or tunnel like ngrok for development). Production-grade but requires ingress configuration.

2. **Socket Mode** — The bot opens an outbound WebSocket to Slack's servers. No public URL needed. Events arrive as JSON envelopes over the WebSocket. Production-ready and officially supported for apps that don't need Slack Marketplace distribution.

## Decision

### Use Socket Mode for Slack event delivery

Socket Mode is chosen because:

- **Matches the Discord transport pattern exactly** — both are outbound WebSocket connections. The reconnection logic, event parsing, and lifecycle management are structurally identical.
- **No ingress configuration** — works in local development, on Railway, on Fly.io, and any deployment without exposing additional ports or configuring reverse proxies.
- **OpenPaw is a private app** — it's installed in the operator's own Slack workspace, not distributed on the Slack Marketplace. Socket Mode is Slack's recommended approach for private apps.
- **Throughput is sufficient** — Socket Mode supports ~37,000 events/hour per connection, far exceeding what agent chat requires.

### Two-token authentication model

Slack requires two tokens (unlike Discord's single bot token):

- **App-Level Token** (`xapp-...`) — used exclusively for the Socket Mode WebSocket connection (`apps.connections.open`). Requires the `connections:write` scope.
- **Bot Token** (`xoxb-...`) — used for all REST API calls (`chat.postMessage`, `chat.update`, `auth.test`). Requires `chat:write`, `im:history`, `im:read`, `im:write` scopes.

### Transport architecture (follows ADR-0001 trigger boundary)

The Slack transport follows the same architecture as Discord — it is a pure OData API client:

1. **Bootstrap** — Archive stale `ChannelType='slack'` entities, create new Channel, Configure with webhook URL, Connect.
2. **Inbound** — Socket Mode receives message events → dispatch `Channel.ReceiveMessage` (ONE entity, ONE action).
3. **Outbound** — WASM `send_reply` posts to webhook → transport delivers via `chat.postMessage`.
4. **Interactions** — Approve/deny button clicks arrive as `interactive` envelopes over Socket Mode (not a separate HTTP webhook like Discord). Transport calls Temper decisions API identically to Discord.

### Key simplifications vs Discord

- **No heartbeat** — Slack manages keepalive server-side.
- **No resume/session** — On disconnect, request a fresh WebSocket URL and reconnect.
- **No DM channel mapping** — Slack provides the channel ID directly in message events. Discord required tracking `user_id → dm_channel_id` because DM channels must be created/discovered separately.
- **Interactions via WebSocket** — Discord delivers button clicks via a separate HTTP webhook endpoint with Ed25519 signature verification. Slack delivers them through the same Socket Mode WebSocket, eliminating the `/interaction` route entirely.

### Files

New:
- `crates/paw-transport/src/slack/` — `mod.rs`, `transport.rs`, `socket.rs`, `api.rs`, `types.rs`

Modified:
- `crates/paw-transport/src/lib.rs` — add `pub mod slack`
- `crates/paw-transport/Cargo.toml` — add `hmac`, `sha2` for signature verification
- `crates/openpaw/src/config.rs` — add `SLACK_APP_TOKEN`, `SLACK_BOT_TOKEN`, `SLACK_SIGNING_SECRET`
- `crates/openpaw/src/startup.rs` — add `spawn_slack_transport()`, seed tokens into vault

Unchanged:
- `os-apps/paw-channels/specs/channel.ioa.toml` — already platform-agnostic
- `os-apps/paw-channels/wasm/route_message/` — already channel-type-agnostic
- `os-apps/paw-channels/wasm/send_reply/` — already channel-type-agnostic

## Consequences

### Positive

- Both Discord and Slack transports can run simultaneously — each creates its own Channel entity with distinct `channel_type`.
- Zero changes to WASM modules or entity specs — the platform-agnostic Channel architecture pays off.
- The same Cedar governance flow (approve/deny buttons) works identically in Slack via Block Kit.
- Socket Mode eliminates ingress requirements, simplifying deployment.

### Negative

- Two tokens per Slack workspace adds operational complexity vs Discord's single token.
- Socket Mode is not available for apps distributed via the Slack Marketplace (not a concern for OpenPaw's use case).

### Risks

- Slack's Socket Mode has a 37,000 events/hour throughput limit. If OpenPaw scales to very high message volumes, this could become a bottleneck. Mitigation: migrate to Events API only if this limit is reached.
- The `hmac` and `sha2` crates are added as new dependencies for Slack signature verification, though they are not yet used in the core flow (signature verification is for future webhook extensions). These are well-maintained crates with no security concerns.
