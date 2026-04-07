# Open Paw

Agent platform built on [Temper](https://github.com/nerdsane/temper). Talk to agents via Discord or Slack — they manage software projects by spawning developer agents with persistent cloud computers.

## Setup

### Have an agent? Point it here:

> Read [INSTRUCTIONS.md](INSTRUCTIONS.md) and follow it.

### Doing it yourself?

```bash
git clone https://github.com/nerdsane/openpaw.git
cd openpaw
cargo run
```

First run asks for your Anthropic API key and optionally a Discord bot token. Then it starts. Dashboard at **http://localhost:3467/dashboard**.

Next time you run it, no prompts — everything is saved.

## Architecture

All agent logic is modeled as Temper state machines (IOA specs), WASM integrations, and Cedar authorization policies. The binary embeds the Temper platform engine.

### Temper Apps

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

### Environment Variables (Optional)

Env vars override saved secrets. Only needed for Docker/CI.

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
