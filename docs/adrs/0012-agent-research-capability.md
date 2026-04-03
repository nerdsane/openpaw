# ADR-0012: Agent Research Capability

## Status

Accepted

## Context

OpenPaw agents can execute code in sandboxes, manage entities, and call external services (Datadog, Railway, Vercel), but they cannot research online. When given a non-trivial task, agents jump straight to implementation without exploring documentation, prior art, or best practices. This leads to suboptimal solutions and repeated mistakes.

Two complementary insights inform the design:

1. **Slate's episodic memory** (Random Labs, March 2026) — when context is compacted, preserve the *trajectory* (what was tried, what worked, what failed) rather than just the current state. This prevents agents from repeating failed approaches after compaction.

2. **The bitter lesson** (Browser Use, January 2026) — agent frameworks should expand the action space and get out of the way. Don't build deterministic orchestrators or complex planning infrastructure. The model is the intelligence; it just needs the right tools.

## Decision

### 1. Web search and fetch as standalone WASM modules

Create a new `paw-research` os-app with a `WebQuery` entity and two standalone WASM modules:

- **`web_search`** — calls the Exa API for semantic web search, returns `[{title, url, text}]`
- **`web_fetch`** — fetches a URL, strips HTML tags, returns clean text (truncated to 100KB)

Each is an independently compilable, deployable WASM crate — not a hardcoded function in the monty_repl dispatch layer. This follows the entity-first principle: every web query is an auditable entity with a governed lifecycle (Created → Executing → Complete/Failed), Cedar authorization, and a trajectory record.

Thin dispatch wrappers in monty_repl provide an ergonomic Python API:
```python
results = temper.web_search("rust async patterns")
text = temper.web_fetch("https://tokio.rs/tokio/tutorial/async")
```

Under the hood, each call creates a WebQuery entity, dispatches ExecuteSearch/ExecuteFetch, and reads back the results.

**Why standalone modules, not dispatch functions:** The Datadog/Railway/Vercel integrations are dispatch functions — direct HTTP calls from within monty_repl. For web research, standalone WASM modules are preferred because:
- Research queries are auditable entities (who searched what, when)
- Cedar policies govern which agents can search
- Modules are independently deployable and testable
- Follows the existing pattern for all other capabilities

### 2. Episodic context compaction

Replace the generic summarization prompt (Goal/Progress/Next Steps) with an episode-based format:

```
## Active Goal
## Episodes
### Episode: <title>
- Worked: <what succeeded>
- Failed: <what was tried and abandoned>
- Discoveries: <facts learned>
- Artifacts: <files changed, entities created>
## Current State
```

This preserves the trajectory. A future model reading compacted context knows which approaches were already tried and failed, avoiding repeated work.

### 3. Research-first planning as a Skill, not infrastructure

Per the bitter lesson: don't build plan mode as entity state machines or WASM coordination logic. Instead, create a Skill entity with markdown content that instructs agents to:

1. Research (codebase + web)
2. Write a plan
3. Wait for human approval
4. Implement

The model does the planning. We just tell it to. The skill includes skip conditions for trivial tasks.

## Consequences

### Positive

- Agents can now search the web and read documentation before implementing
- Context compaction preserves trajectory, preventing repeated failed approaches
- Research-first planning behavior is injectable per-agent via Skills (no code changes to enable/disable)
- All web queries are auditable entities with Cedar governance
- No new orchestration complexity — the model decides when and how to research

### Negative

- Each `temper.web_search()` call creates a WebQuery entity — slight overhead vs. a direct HTTP call
- Exa API dependency for search (requires `EXA_API_KEY`)
- HTML stripping is basic (character-by-character, no regex) — may produce noisy output on malformed pages

## Files

| File | Change |
|------|--------|
| `os-apps/paw-research/specs/web_query.ioa.toml` | WebQuery entity spec |
| `os-apps/paw-research/specs/model.csdl.xml` | OData CSDL model |
| `os-apps/paw-research/wasm/web_search/src/lib.rs` | Exa search WASM module |
| `os-apps/paw-research/wasm/web_fetch/src/lib.rs` | URL fetch WASM module |
| `os-apps/paw-research/policies/research.cedar` | Cedar authorization |
| `os-apps/paw-agent/wasm/monty_repl/src/dispatch.rs` | Dispatch wrappers |
| `os-apps/paw-agent/wasm/llm_caller/src/lib.rs` | Tool descriptions |
| `os-apps/paw-agent/wasm/context_compactor/src/lib.rs` | Episodic compaction prompt |
| `crates/openpaw/src/config.rs` | EXA_API_KEY config |
| `crates/openpaw/src/startup.rs` | paw-research registration + secret seeding |
| `souls/skills/research-first-planning.md` | Planning skill content |
