# Open Paw — Agent Setup Instructions

You are setting up the Open Paw agent platform for a human. Read this entire file before starting.

## Prerequisites

- Rust toolchain (stable) must be installed
- You need the human's Anthropic API key — ask them for it if you don't have it
- Optional: Discord bot token if they want Discord integration

## Setup

### 1. Clone and enter the repo

```bash
git clone https://github.com/nerdsane/openpaw.git
cd openpaw
```

### 2. Write a `.env` file

```bash
echo 'ANTHROPIC_API_KEY=sk-ant-...' > .env
```

Replace with the human's actual key. Add optional tokens on separate lines:

```bash
echo 'DISCORD_BOT_TOKEN=...' >> .env
echo 'DISCORD_GUILD_ID=...' >> .env
echo 'DISCORD_FEED_CHANNEL_ID=...' >> .env
echo 'DISCORD_FORUM_CHANNEL_ID=...' >> .env
```

### 3. Start the server

```bash
cargo run
```

First boot takes 20-30 seconds (compiling WASM modules, verifying specs). The server is ready when you see:

```
Open Paw listening on port 3467
```

Dashboard: http://localhost:3467/dashboard

### 4. Create an agent (optional)

Once the server is running, create agents via the REST API:

```bash
curl -X POST http://localhost:3467/paw/agents/create \
  -H 'x-tenant-id: default' \
  -H 'x-temper-principal-kind: admin' \
  -H 'content-type: application/json' \
  -d '{"name": "dev-agent", "role": "Software Developer", "model": "claude-sonnet-4-6"}'
```

### 5. Connect Discord at runtime (optional)

No restart needed:

```bash
curl -X POST http://localhost:3467/paw/transports/discord/connect \
  -H 'x-tenant-id: default' \
  -H 'x-temper-principal-kind: admin' \
  -H 'content-type: application/json' \
  -d '{"bot_token": "MTI3...", "guild_id": "123456789"}'
```

## REST API Reference

All endpoints require headers: `x-tenant-id: default` and `x-temper-principal-kind: admin`.

| Method | Endpoint | Body |
|--------|----------|------|
| GET | `/paw/setup/status` | — |
| POST | `/paw/setup/secrets` | `{"key": "...", "value": "..."}` |
| GET | `/paw/setup/secrets` | — (returns key names, not values) |
| DELETE | `/paw/setup/secrets/{key}` | — |
| GET | `/paw/souls/templates` | — |
| POST | `/paw/agents/create` | `{"name", "role?", "model?", "tools_enabled?", "max_turns?"}` |
| GET | `/paw/transports/status` | — |
| POST | `/paw/transports/discord/connect` | `{"bot_token", "public_key?", "guild_id?", "feed_channel_id?", "forum_channel_id?"}` |
| POST | `/paw/transports/discord/disconnect` | — |
| POST | `/paw/transports/slack/connect` | `{"app_token", "bot_token", "signing_secret?"}` |
| POST | `/paw/transports/slack/disconnect` | — |

Allowed secret keys: `anthropic_api_key`, `discord_bot_token`, `discord_public_key`, `discord_guild_id`, `discord_feed_channel_id`, `discord_forum_channel_id`, `slack_app_token`, `slack_bot_token`, `slack_signing_secret`, `github_token`, `exa_api_key`, `tensorlake_api_key`

## Notes

- All secrets from `.env` are encrypted and persisted to the local database on first boot. The `.env` can be deleted afterward.
- The encryption key is at `~/.local/share/openpaw/vault.key`. Don't delete it.
- Subsequent `cargo run` starts immediately with no prompts — secrets load from the database.
- If the server crashes, orphaned sessions are automatically cleaned up on restart.
