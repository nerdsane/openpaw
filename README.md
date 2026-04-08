# Open Paw

Agent platform built on [Temper](https://github.com/nerdsane/temper). Talk to Paw via Discord — it manages software projects by spawning specialized agents with persistent cloud computers.

## Get Started

```bash
git clone https://github.com/nerdsane/openpaw.git
cd openpaw
cargo run
```

On first run, the CLI walks you through setup:
- Pick your AI provider (Anthropic, OpenRouter, or OpenAI) and paste your API key
- Connect Discord (with step-by-step bot creation instructions)
- A short interview about you and what kind of Paw you want
- Paw generates a personalized soul using the LLM and shows you a preview

Everything is encrypted and saved. Next time you run it, no prompts — just boots.

```bash
cargo run -- setup    # reconfigure or personalize Paw
cargo run -- doctor   # check what's configured
```

### Have an agent? Give it this:

> Set up Open Paw for me. Read the instructions at https://raw.githubusercontent.com/nerdsane/openpaw/main/INSTRUCTIONS.md and follow them.

## Architecture

All agent logic is Temper state machines (IOA specs), WASM integrations, and Cedar authorization policies.

### Temper Apps

| App | Purpose |
|-----|---------|
| paw-agent | Agent execution — Agent, Soul, Skill, Memory, Session, Team, Project |
| paw-channels | Multi-platform messaging — Channel, AgentRoute, ChannelSession |
| paw-fs | Governed file storage — File, Workspace, Directory |
| paw-pm | Project management — Issues, Plans |
| paw-ingest | Webhook ingestion — WebhookEvent, WebhookRoute |
| paw-research | Web search and fetch |
| paw-foresight | Probe projections and entropy simulation |
