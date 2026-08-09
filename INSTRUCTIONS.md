# Temper Paw — Agent Setup Instructions

You are setting up the Temper Paw agent platform for a human. Read this entire file before starting.

## Prerequisites

- Rust toolchain (stable)
- An API key from Anthropic, OpenRouter, or OpenAI (ask the human)
- Optional: a Discord bot token (see Discord setup below)

## Setup

### 1. Clone and build

```bash
git clone https://github.com/nerdsane/temperpaw.git
cd temperpaw
```

Before setup or implementation work, sync with Linear:

- Search Linear for an existing matching issue before creating anything new.
- If an issue exists, append progress there instead of creating a duplicate.
- When starting work, move the issue to In Progress and add a start comment.
- When completing work, attach commits, PRs, proof reports, deployment links, and verification notes before moving the issue to Done.
- If Linear tools are unavailable, say so explicitly and do not claim sync happened.

### 2. Configure

Write a `.env` file with the human's API key. The key goes in `ANTHROPIC_API_KEY` regardless of provider — the platform detects the provider from the key prefix.

```bash
echo 'ANTHROPIC_API_KEY=sk-ant-...' > .env
```

If the human uses OpenRouter or OpenAI, also set the provider:

```bash
echo 'LLM_PROVIDER=openrouter' >> .env   # for sk-or-... keys
echo 'LLM_PROVIDER=openai' >> .env       # for sk-proj-... keys
```

### 3. Discord (optional)

If the human wants Discord, they need a bot token. Here's how to create one:

1. Go to discord.com/developers/applications
2. Click "New Application" → name it
3. Click "Bot" in the left sidebar
4. Click "Reset Token" → copy the token
5. Turn on "Message Content Intent" under Privileged Gateway Intents
6. Go to OAuth2 → URL Generator → select scope "bot" → select "Send Messages" + "Read Message History"
7. Copy the generated URL, open it, pick the server to add the bot to

Then add to `.env`:

```bash
echo 'DISCORD_BOT_TOKEN=...' >> .env
```

### 4. Start

```bash
cargo run
```

First boot takes 20-30 seconds. The server is ready when you see:

```
  Temper Paw is running.

  API:       http://localhost:3467/tdata
  Dashboard: http://localhost:3467/dashboard

  ✓ Anthropic API key
  ✓ Discord
```

If port 3467 is taken, it picks a free one automatically.

### 5. Personalize (optional)

If the human is at the terminal, they can run the interactive setup to personalize Paw — it asks a few questions and generates a soul tailored to them:

```bash
cargo run -- setup
```

### 6. Diagnose

```bash
cargo run -- doctor
```

## REST API

Once running, everything is available via HTTP. All endpoints need headers `x-tenant-id: default` and `x-temper-principal-kind: admin`.

| Method | Endpoint | Body |
|--------|----------|------|
| GET | `/paw/setup/status` | — |
| POST | `/paw/setup/secrets` | `{"key": "...", "value": "..."}` |
| GET | `/paw/setup/secrets` | — (key names only) |
| DELETE | `/paw/setup/secrets/{key}` | — |
| GET | `/paw/souls/templates` | — |
| POST | `/paw/agents/create` | `{"name", "role?", "model?", "tools_enabled?", "max_turns?"}` |
| GET | `/paw/transports/status` | — |
| POST | `/paw/transports/discord/connect` | `{"bot_token", "public_key?", "guild_id?", "feed_channel_id?", "forum_channel_id?"}` |
| POST | `/paw/transports/discord/disconnect` | — |
| POST | `/paw/transports/slack/connect` | `{"app_token", "bot_token", "signing_secret?"}` |
| POST | `/paw/transports/slack/disconnect` | — |

## Notes

- All secrets are encrypted and persisted. The `.env` can be deleted after first boot.
- Encryption key at `~/.local/share/temperpaw/vault.key` — don't delete it.
- Orphaned sessions are cleaned up automatically on restart.
