# Open Paw — Agent Setup Instructions

You are setting up the Open Paw agent platform for a human. Read this entire file before starting.

## Prerequisites

- Rust toolchain (stable) must be installed
- The human needs an API key from one of: Anthropic, OpenRouter, or OpenAI
- Optional: Discord bot token if they want Discord integration

## Setup

### 1. Clone and enter the repo

```bash
git clone https://github.com/nerdsane/openpaw.git
cd openpaw
```

### 2. Write a `.env` file

Ask the human which provider they use and get their API key.

**Anthropic** (key from console.anthropic.com):
```bash
echo 'ANTHROPIC_API_KEY=sk-ant-...' > .env
```

**OpenRouter** (key from openrouter.ai/keys):
```bash
echo 'ANTHROPIC_API_KEY=sk-or-...' > .env
echo 'LLM_PROVIDER=openrouter' >> .env
```

**OpenAI** (key from platform.openai.com/api-keys — ChatGPT subscriptions don't include API access):
```bash
echo 'ANTHROPIC_API_KEY=sk-proj-...' > .env
echo 'LLM_PROVIDER=openai' >> .env
```

Optional — add Discord:
```bash
echo 'DISCORD_BOT_TOKEN=...' >> .env
```

### 3. Start the server

```bash
cargo run
```

First boot takes 20-30 seconds (compiling WASM modules, verifying specs). The server is ready when you see:

```
  Open Paw is running.

  API:       http://localhost:3467/tdata
  Dashboard: http://localhost:3467/dashboard

  ✓ Anthropic API key
  ✓ Discord
```

If the preferred port (3467) is taken, it automatically picks a free one.

### 4. Personalize Paw (optional)

If the human is at the terminal, they can run the interactive setup to personalize their agent:

```bash
cargo run -- setup
```

This walks through a short interview and uses the LLM to generate a soul tailored to the human. It can be re-run anytime.

### 5. Diagnose

```bash
cargo run -- doctor
```

Shows what's configured and what's missing.

## REST API

Once the server is running, everything is also available via HTTP. All endpoints need headers: `x-tenant-id: default` and `x-temper-principal-kind: admin`.

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

## Notes

- All secrets are encrypted and persisted to the local database. The `.env` can be deleted after first boot.
- Encryption key: `~/.local/share/openpaw/vault.key` — don't delete it.
- Orphaned sessions are automatically cleaned up on restart.
- If the port is busy, the server picks a free one automatically.
