# Open Paw

Agent platform built on [Temper](https://github.com/nerdsane/temper). You talk to Paw via Discord. Paw spawns agents, gives them cloud computers, and manages work across projects. Agents can create Temper apps to extend their own capabilities — new entity types, state machines, tools — so the platform grows as they work.

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

## What Works

- **Core platform**: Paw boots, connects to Discord, runs conversations, spawns agents with cloud computers
- **Entity CRUD + state machines**: temper_create/get/list/action all work; IOA state machines enforce valid transitions
- **Web research**: temper_web_search + temper_web_fetch available to agents
- **OS app loading**: apps under `os-apps/` are discovered and loaded at startup
- **Agent sessions**: spawn_agent, temper_spawn_session, temper_steer_session work
- **Cedar authorization**: policies enforce who can do what

### Temper Apps

Paw itself is implemented as Temper apps — the agent isn't separate from the platform, it runs on it.

- **paw-agent** — the agent, its soul, skills, memory, and sessions are all Temper entities with state machines and Cedar policies
- **paw-fs** — governed filesystem; agents read and write files through Temper with authorization checks
- **paw-pm** — project management; issues and plans tracked as Temper entities so agents can manage work
- **paw-foresight** — probes and projections; agents simulate outcomes before acting

## Next Steps

1. **Agent awareness of available tools** — agents need to know what Temper tools they have (`temper.write`, `temper.specs`, etc.) and which are granted vs denied in their session. Without this self-awareness, agents waste turns attempting blocked operations.
2. **Agent access to build new capabilities** — enable agents to create, install, and evolve Temper apps at runtime: submit specs (`temper_submit_specs`), write to TemperFS (`temper.write`), introspect installed specs (`/observe/specs`). This is what makes the platform self-extending.
3. **End-to-end app lifecycle** — once agents can submit specs + write files, test the full cycle: agent designs an app → submits specs → governance approves → app installs → agent uses it. A wiki app (web research → structured knowledge base) is the first candidate.
4. **Surface auth requests to Discord** — when an agent needs a permission it doesn't have, it should be able to post a structured approval request to the user's Discord channel (tool needed, reason, scope). The user approves or denies; the agent polls and retries. No more stalling in chat loops.
5. **Evolve apps over time** — agents should be able to extend an installed app (add entity types, new actions, updated policies) without reinstalling from scratch. Incremental spec evolution, not tear-down-and-rebuild.
