# Open Paw

Agent daemon built on the [Temper](https://github.com/nerdsane/temper) platform. A human talks to a Paw agent via Discord or Slack, and Paw manages software projects by spawning developer agents with persistent cloud computers.

Everything persists across restarts. Secrets are encrypted and stored in the local database. Sessions that crash are automatically recovered.

---

## Setup for Humans

### Prerequisites

- Rust toolchain (`rustup` — stable)
- An Anthropic API key ([console.anthropic.com](https://console.anthropic.com))
- Optional: a Discord bot token ([discord.com/developers](https://discord.com/developers/applications))

### First Run

```bash
git clone https://github.com/nerdsane/openpaw.git
cd openpaw
cargo run
```

On first run with no prior configuration, Open Paw walks you through setup:

```
  Welcome to Open Paw!
  First time? Let's get you set up.

  Anthropic API Key: sk-ant-...
  ✓ Key saved

  Connect Discord? (y/N): y
  Bot Token: MTI3...
  ✓ Token saved

  Starting Open Paw...
  Open Paw Data API: http://localhost:3467/tdata
  Open Paw Dashboard: http://localhost:3467/dashboard
```

That's it. Your API key and tokens are encrypted and saved — they survive restarts without needing environment variables.

### Dashboard

Open the dashboard at **http://localhost:3467/dashboard** to:

- **Floor** — watch active agent sessions in real time
- **Agents** — view agents, click **+ NEW AGENT** to create one
- **Connections** — connect/disconnect Discord and Slack with live status
- **Permissions** — view Cedar authorization policies

### Creating an Agent

From the dashboard: **Agents** > **+ NEW AGENT**

Or via API:
```bash
curl -X POST http://localhost:3467/paw/agents/create \
  -H 'content-type: application/json' \
  -H 'x-tenant-id: default' \
  -H 'x-temper-principal-kind: admin' \
  -d '{"name": "my-assistant", "role": "Software Developer", "model": "claude-sonnet-4-6"}'
```

### Connecting Discord

From the dashboard: **Connections** > **CONNECT** on the Discord card.

Or via API:
```bash
curl -X POST http://localhost:3467/paw/transports/discord/connect \
  -H 'content-type: application/json' \
  -H 'x-tenant-id: default' \
  -H 'x-temper-principal-kind: admin' \
  -d '{"bot_token": "MTI3...", "guild_id": "optional", "feed_channel_id": "optional"}'
```

No restart needed — the transport connects immediately.

### Subsequent Runs

```bash
cargo run
```

No prompts. Secrets load from the encrypted vault. Transports reconnect from saved tokens. Any sessions that were mid-flight when the process last stopped are automatically marked as failed and cleaned up.

---

## Setup for Agents

If you're an AI agent (Claude Code, Cursor, or any LLM with shell access), here's how to set up Open Paw for your human.

### Option A: Environment Variables (Non-Interactive)

Write a `.env` file and start the binary. This skips the interactive prompts entirely.

```bash
cd /path/to/openpaw

# Create .env with the human's API key
cat > .env << 'EOF'
ANTHROPIC_API_KEY=sk-ant-...
EOF

# Build and start
cargo build --release
cargo run --release
```

The server starts on port 3467. All secrets from `.env` are automatically persisted to the encrypted vault, so the `.env` can be removed later without losing configuration.

### Option B: Use the REST API (Server Already Running)

If the Open Paw server is already running, configure everything via HTTP:

```bash
BASE="http://localhost:3467"
HEADERS='-H "x-tenant-id: default" -H "x-temper-principal-kind: admin" -H "content-type: application/json"'

# 1. Save API key
curl -X POST "$BASE/paw/setup/secrets" $HEADERS \
  -d '{"key": "anthropic_api_key", "value": "sk-ant-..."}'

# 2. Connect Discord (optional)
curl -X POST "$BASE/paw/transports/discord/connect" $HEADERS \
  -d '{"bot_token": "MTI3...", "guild_id": "123456789"}'

# 3. Create an agent
curl -X POST "$BASE/paw/agents/create" $HEADERS \
  -d '{"name": "dev-agent", "role": "Software Developer", "model": "claude-sonnet-4-6"}'

# 4. Check status
curl "$BASE/paw/setup/status" -H "x-tenant-id: default" -H "x-temper-principal-kind: admin"
```

### Option C: Pipe Answers to Interactive Setup

If starting fresh with no `.env` and no prior vault:

```bash
printf "sk-ant-...\nn\n" | cargo run
#        ^API key  ^skip Discord
```

### API Reference

All endpoints use headers: `x-tenant-id: default` and `x-temper-principal-kind: admin`.

| Method | Endpoint | Purpose |
|--------|----------|---------|
| GET | `/paw/setup/status` | Onboarding status (has API key, has agents, transport status) |
| POST | `/paw/setup/secrets` | Save a secret: `{"key": "...", "value": "..."}` |
| GET | `/paw/setup/secrets` | List secret key names (not values) |
| DELETE | `/paw/setup/secrets/{key}` | Remove a secret |
| GET | `/paw/souls/templates` | List available agent templates |
| POST | `/paw/agents/create` | Create agent: `{"name", "role", "model", "tools_enabled", "max_turns"}` |
| GET | `/paw/transports/status` | Discord/Slack connection status |
| POST | `/paw/transports/discord/connect` | `{"bot_token", "public_key?", "guild_id?", "feed_channel_id?", "forum_channel_id?"}` |
| POST | `/paw/transports/discord/disconnect` | Stop Discord transport |
| POST | `/paw/transports/slack/connect` | `{"app_token", "bot_token", "signing_secret?"}` |
| POST | `/paw/transports/slack/disconnect` | Stop Slack transport |

Allowed secret keys: `anthropic_api_key`, `discord_bot_token`, `discord_public_key`, `discord_guild_id`, `discord_feed_channel_id`, `discord_forum_channel_id`, `slack_app_token`, `slack_bot_token`, `slack_signing_secret`, `github_token`, `exa_api_key`, `tensorlake_api_key`

---

## What Persists

| What | Where | Survives Restart |
|------|-------|-----------------|
| Entity state (agents, sessions, memories) | Turso/SQLite at `~/.local/share/openpaw/paw.db` | Yes |
| Secrets (API keys, tokens) | Encrypted in Turso, decrypted via vault key | Yes |
| Vault encryption key | `~/.local/share/openpaw/vault.key` (0600) | Yes |
| Conversation history | Turso blob store | Yes |
| Cedar policies | Turso event store | Yes |
| WASM modules | Turso (verified + cached) | Yes |
| Transport connections | Tokens in vault; transports reconnect on boot | Yes |
| Mid-flight sessions | Recovered to Failed state on restart | Cleaned up |

## Architecture

Open Paw follows the Temper OS app pattern — all agent logic is modeled as IOA specs (state machines), WASM integrations, and Cedar policies. The daemon binary is a thin bootstrap layer that embeds the Temper platform engine.

### OS Apps

| App | Purpose |
|-----|---------|
| paw-agent | Agent execution — Agent, Soul, Skill, Memory, Session, Team |
| paw-channels | Multi-platform messaging — Channel, AgentRoute, ChannelSession |
| paw-fs | Governed file storage — File, Workspace, Directory |
| paw-pm | Project management — Issues, Plans |
| paw-compute | Cloud computer provisioning |
| paw-harness | Development workflow — ProjectHarness, WorkCycle |
| paw-heal | Self-healing monitoring — Monitor, AlertCycle |
| paw-ingest | Webhook ingestion — WebhookEvent, WebhookRoute |
| paw-research | Web search and fetch |
| paw-foresight | Probe projections and entropy simulation |

### Agents & Skills

Agent definitions live in `os-apps/paw-agent/agents/` and are bootstrapped at startup:

| Agent | Role |
|-------|------|
| `paw` | Chief of staff — manages projects, spawns teams (SOUL.md + STYLE.md + AGENT.md) |
| `swe` | Software developer — receives tasks, writes code (AGENT.md) |
| `sre` | Site reliability — monitoring, deployment (AGENT.md) |
| `probe` | Foresight probe — observes projected futures (AGENT.md) |

Reusable skills live in `os-apps/paw-agent/skills/`.

## Environment Variables (Optional)

Environment variables override vault-stored secrets. They're only needed for Docker/CI or if you prefer env-based config over the interactive setup.

| Variable | Purpose |
|----------|---------|
| `ANTHROPIC_API_KEY` | Claude API key |
| `DISCORD_BOT_TOKEN` | Discord bot token |
| `DISCORD_GUILD_ID` | Server ID for observability |
| `DISCORD_FEED_CHANNEL_ID` | One-liner feed channel |
| `DISCORD_FORUM_CHANNEL_ID` | Per-agent forum channel |
| `SLACK_APP_TOKEN` | Slack Socket Mode token (xapp-...) |
| `SLACK_BOT_TOKEN` | Slack Web API token (xoxb-...) |
| `TL_API_KEY` | Tensorlake sandbox provisioning |
| `TURSO_URL` | Database URL (default: local SQLite) |
| `TEMPER_VAULT_KEY` | 32-byte base64 vault key (production override) |
| `PORT` | HTTP port (default: 3467) |
| `PAW_TENANT` | Tenant ID (default: "default") |
