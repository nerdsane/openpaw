# Discord DM Routing And Compaction Fix

Date: 2026-05-07

## Datadog Evidence

- Window investigated: 2026-05-07 14:25-15:45 UTC, matching 10:25-11:45 AM Eastern.
- `service:openpaw` received and dispatched the reported Discord DMs from `arni0x9053`.
- Successful replies at 14:34:10 and 14:35:25 UTC used thread `1018228973869727785`.
- Six failed reply attempts from 14:36:21 through 15:39:02 UTC logged `discord reply webhook has no DM channel mapping` for stale thread `codex-live-proof-thread-codex-live-proof-1778010612`.
- Route logs still showed each incoming DM routed to fresh sessions on thread `1018228973869727785`, so ingress was healthy and reply lookup selected the wrong ChannelSession.
- APM showed `context_compactor` failures at 14:34:05 and 14:35:21 UTC with `Compaction LLM call failed (HTTP 400): {"detail":"Store must be set to false"}`.
- Two `message.txt` attachment downloads returned `415 Unsupported Media Type`, so the attached brand context was not inlined on those turns.

## Changes Verified

- `agent_reply` now looks up the ChannelSession by the current `Session` entity id first, then parent session id, then legacy agent binding fallback.
- `context_compactor` now includes `store: false` in OpenAI/Codex Responses API compaction requests.
- Discord text attachment downloads now try the proxy URL first and fall back to the canonical CDN URL before giving up.

## Commands

```sh
cargo test --manifest-path os-apps/paw-agent/wasm/agent_reply/Cargo.toml
cargo test --manifest-path os-apps/paw-agent/wasm/context_compactor/Cargo.toml
cargo build --manifest-path os-apps/paw-agent/wasm/agent_reply/Cargo.toml --target wasm32-unknown-unknown --release
cargo build --manifest-path os-apps/paw-agent/wasm/context_compactor/Cargo.toml --target wasm32-unknown-unknown --release
cargo test -p paw-transport
cargo check --workspace
cargo run -p temperpaw
curl -sS -o /tmp/temperpaw-root.out -w '%{http_code}\n' http://localhost:3467/
curl -sS -o /tmp/temperpaw-dashboard.out -w '%{http_code}\n' http://localhost:3467/dashboard
```

## Results

- `agent_reply` tests: 2 passed.
- `context_compactor` tests: 12 passed.
- `paw-transport` tests: 23 passed.
- Both changed WASM modules built for `wasm32-unknown-unknown`.
- Full Rust workspace check passed.
- Local server booted and printed API/dashboard URLs.
- Local `/` returned `401`; local `/dashboard` returned `307`.
- Local Discord transport was not exercised because the local vault has no `discord_bot_token`; the production DM flow was verified through Datadog telemetry instead.
