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
| koto-wiki | New | LLM Wiki (Karpathy pattern) — WikiSource, WikiPage, WikiJob |

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
- **koto-wiki is spec-only** — the CSDL, IOA, Cedar, and agent definitions exist but the app hasn't been end-to-end tested with real ingest/compile/lint cycles (blocked by temper.write above).

## Next Steps

1. **Enable `temper.write` in agent sessions** — this is the single highest-leverage fix. Without it, agents can't persist any file artifacts (wiki pages, reports, snapshots). Likely a Cedar policy or session capability grant.
2. **Enable spec introspection for agents** — grant read access to `/observe/specs` so agents can discover installed entity sets and self-validate before creating records.
3. **End-to-end test koto-wiki** — once temper.write works, run a full ingest→compile→lint cycle through the Curator agent on a real topic.
4. **Generalize koto-wiki → llm-wiki** — koto-wiki is Kotowari-scoped (persona + learning graph). A general-purpose `llm-wiki` app would drop the persona/concept coupling and work for any topic. The entity model is 90% there; needs a KnowledgeBase wrapper entity and topic-derived KB creation.
5. **Wire single-command invocation** — make `llm-wiki "<topic>"` a recognized command pattern that Paw routes to the wiki Curator agent automatically (AgentRoute or skill-based dispatch).
