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

### Temper Apps

| App | Status | Purpose |
|-----|--------|---------|
| paw-agent | Working | Agent execution — Agent, Soul, Skill, Memory, Session, Team, Project |
| paw-channels | Working | Multi-platform messaging — Channel, AgentRoute, ChannelSession |
| paw-fs | Working | Governed file storage — File, Workspace, Directory |
| paw-pm | Working | Project management — Issues, Plans |
| paw-ingest | Working | Webhook ingestion — WebhookEvent, WebhookRoute |
| paw-research | Working | Web search and fetch |
| paw-foresight | Working | Probe projections and entropy simulation |
| koto-learn | New | Language learning knowledge graph — Concept, Encounter, Persona |
| koto-tutor | New | AI tutor agent — encounter design + adaptive teaching |
| koto-wiki | New | LLM Wiki ([Karpathy pattern](https://gist.github.com/karpathy/442a6bf555914893e9891c11519de94f)) — WikiSource, WikiPage, WikiJob |

## What Works

- **Core platform**: Paw boots, connects to Discord, runs conversations, spawns agents with cloud computers
- **Entity CRUD + state machines**: temper_create/get/list/action all work; IOA state machines enforce valid transitions
- **Web research**: temper_web_search + temper_web_fetch available to agents
- **OS app loading**: apps under `os-apps/` are discovered and loaded at startup
- **Agent sessions**: spawn_agent, temper_spawn_session, temper_steer_session work
- **Cedar authorization**: policies enforce who can do what

## What Doesn't Work (Known Blockers)

- **`temper.write` not available in agent sessions** — agents cannot write to TemperFS from within their tool sandbox. This blocks any workflow that needs to persist files (wiki pages, raw source snapshots, reports). The `write` tool in the sandbox is the local filesystem writer, not TemperFS.
- **`/observe/specs` returns 403** — agents cannot inspect or register new entity specs at runtime. This blocks dynamic app installation (the `temper_install_app` path).
- **Dynamic app install is incomplete** — Paw can design an OS app spec perfectly but cannot self-install it. The `temper_install_app` + `temper_submit_specs` flow requires permissions the agent session doesn't have.
- **No way to surface auth requests to the user** — when an agent hits a permission wall (403, disabled tool), it has no mechanism to send an approval request to Discord for the human to act on. Agents get stuck in loops asking clarifying questions instead of requesting the specific grant they need.
- **New OS apps are spec-only** — CSDL, IOA, Cedar, and agent definitions exist on disk but haven't been end-to-end tested (blocked by temper.write above).

## Next Steps

1. **Agent awareness of available tools** — agents need to know what Temper tools they have (`temper.write`, `temper.specs`, etc.) and which are granted vs denied in their session. Without this self-awareness, agents waste turns attempting blocked operations.
2. **Agent access to build new capabilities** — enable agents to create, install, and evolve Temper apps at runtime: submit specs (`temper_submit_specs`), write to TemperFS (`temper.write`), introspect installed specs (`/observe/specs`). This is what makes the platform self-extending.
3. **End-to-end app lifecycle** — once agents can submit specs + write files, test the full cycle: agent designs an app → submits specs → governance approves → app installs → agent uses it. The koto-wiki app is the first candidate.
4. **Surface auth requests to Discord** — when an agent needs a permission it doesn't have, it should be able to post a structured approval request to the user's Discord channel (tool needed, reason, scope). The user approves or denies; the agent polls and retries. No more stalling in chat loops.
5. **Evolve apps over time** — agents should be able to extend an installed app (add entity types, new actions, updated policies) without reinstalling from scratch. Incremental spec evolution, not tear-down-and-rebuild.
