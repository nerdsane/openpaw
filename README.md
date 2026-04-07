# Open Paw

Agent platform built on [Temper](https://github.com/nerdsane/temper). Talk to agents via Discord or Slack — they manage software projects by spawning developer agents with persistent cloud computers.

## Get Started

### Have an agent? Give it this:

> Set up Open Paw for me. Read the instructions at https://raw.githubusercontent.com/nerdsane/openpaw/main/INSTRUCTIONS.md and follow them.

### Doing it yourself?

```bash
git clone https://github.com/nerdsane/openpaw.git
cd openpaw
cargo run
```

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
