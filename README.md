# Open Paw

Agent daemon built on the [Temper](https://github.com/nerdsane/temper) platform. A human talks to a Paw agent via Discord, and Paw manages software projects by spawning developer agents with persistent cloud computers.

## Quick Start

```bash
# Set required env vars
export DISCORD_BOT_TOKEN=...
export ANTHROPIC_API_KEY=...

# Optional
export TEMPER_API_KEY=...        # Bearer token for API auth
export TEMPER_VAULT_KEY=...      # 32-byte base64 key for secrets encryption
export FLY_API_TOKEN=...         # For Sprites computer provisioning
export TURSO_URL=...             # Database URL (default: local SQLite)

# Run
cargo run
```

## Architecture

Open Paw follows the Temper OS app pattern — all agent logic is modeled as IOA specs (state machines), WASM integrations, and Cedar policies. The daemon binary is a thin bootstrap layer that embeds the Temper platform engine.

### OS Apps

| App | Namespace | Purpose |
|-----|-----------|---------|
| paw-agent | OpenPaw | Agent execution (Agent, Soul, Memory) |
| paw-channels | Paw.Channel | Multi-platform messaging (Channel, AgentRoute, ChannelSession) |
| paw-fs | Paw.FS | File storage (File, Workspace) |
| paw-pm | Paw.PM | Project management (Issues, Plans) |
| paw-compute | Paw.Compute | Cloud computer provisioning (Computer) |
| paw-harness | Paw.Harness | Development workflow enforcement (ProjectHarness, WorkCycle) |
| paw-heal | Paw.Heal | Self-healing monitoring (Monitor, AlertCycle, MonitorScan) |

### Souls

Agent personalities are defined in `souls/` and seeded into the entity system at boot:
- `paw.md` — Project manager agent
- `developer.md` — Software developer agent
- `sre.md` — SRE monitoring and triage agent
