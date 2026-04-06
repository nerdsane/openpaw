# Open Paw

Agent daemon built on [Temper](https://github.com/nerdsane/temper). Talk to agents via Discord or Slack. They manage software projects by spawning developer agents with persistent cloud computers.

## Quick Setup

Give this section to your agent (Claude Code, Cursor, etc.) and it will set everything up.

### For Your Agent

You need: Rust toolchain, the human's Anthropic API key, and optionally a Discord bot token.

```bash
# Clone and enter the repo
git clone https://github.com/nerdsane/openpaw.git
cd openpaw

# Write the human's API key to .env (ask them for it)
echo 'ANTHROPIC_API_KEY=sk-ant-...' > .env

# Optional: add Discord bot token if the human has one
echo 'DISCORD_BOT_TOKEN=...' >> .env

# Build and start
cargo run
```

The server starts on **http://localhost:3467**. Dashboard at **/dashboard**. All secrets are automatically encrypted and persisted — the `.env` file can be deleted afterward without losing anything.

Once running, you can manage everything via the REST API:

```bash
# Check status
curl http://localhost:3467/paw/setup/status \
  -H 'x-tenant-id: default' -H 'x-temper-principal-kind: admin'

# Create an agent
curl -X POST http://localhost:3467/paw/agents/create \
  -H 'x-tenant-id: default' -H 'x-temper-principal-kind: admin' \
  -H 'content-type: application/json' \
  -d '{"name": "dev-agent", "role": "Software Developer", "model": "claude-sonnet-4-6"}'

# Connect Discord at runtime (no restart needed)
curl -X POST http://localhost:3467/paw/transports/discord/connect \
  -H 'x-tenant-id: default' -H 'x-temper-principal-kind: admin' \
  -H 'content-type: application/json' \
  -d '{"bot_token": "MTI3..."}'

# Save any secret
curl -X POST http://localhost:3467/paw/setup/secrets \
  -H 'x-tenant-id: default' -H 'x-temper-principal-kind: admin' \
  -H 'content-type: application/json' \
  -d '{"key": "github_token", "value": "ghp_..."}'
```

All endpoints require headers `x-tenant-id: default` and `x-temper-principal-kind: admin`.

<details>
<summary>Full API reference</summary>

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

</details>

### For Humans (Without an Agent)

```bash
git clone https://github.com/nerdsane/openpaw.git
cd openpaw
cargo run
```

First run asks two questions — your Anthropic API key and whether to connect Discord. Then it starts. Open **http://localhost:3467/dashboard** to create agents and manage connections from the UI.

---

## What Persists

Everything survives restarts. Secrets are AES-256-GCM encrypted in a local SQLite database. The encryption key lives at `~/.local/share/openpaw/vault.key`. Mid-flight sessions are automatically recovered on restart.

## Architecture

All agent logic is modeled as IOA state machines, WASM integrations, and Cedar authorization policies. The binary embeds the Temper platform engine.

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

### Agents

Bootstrapped from `os-apps/paw-agent/agents/` at startup:

| Agent | Role |
|-------|------|
| `paw` | Chief of staff — manages projects, spawns teams |
| `swe` | Software developer — receives tasks, writes code |
| `sre` | Site reliability — monitoring, deployment |
| `probe` | Foresight probe — observes projected futures |

### Environment Variables (Optional)

Env vars override vault-stored secrets. Only needed for Docker/CI.

| Variable | Purpose |
|----------|---------|
| `ANTHROPIC_API_KEY` | Claude API key |
| `DISCORD_BOT_TOKEN` | Discord bot token |
| `DISCORD_GUILD_ID` | Server ID for observability |
| `SLACK_APP_TOKEN` / `SLACK_BOT_TOKEN` | Slack tokens |
| `TL_API_KEY` | Tensorlake sandbox provisioning |
| `TURSO_URL` | Database URL (default: local SQLite) |
| `TEMPER_VAULT_KEY` | Vault key override for production |
| `PORT` | HTTP port (default: 3467) |
