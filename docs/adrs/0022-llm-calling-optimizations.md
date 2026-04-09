# ADR-0022: LLM Calling Infrastructure Optimizations

## Status

Accepted

## Context

Industry analysis identified four areas where OpenPaw's LLM calling infrastructure leaves significant value on the table.

### System prompt is rebuilt from scratch every turn

The `llm_caller` WASM module rebuilds the full system prompt on every LLM call by making 7-9 HTTP reads to TemperFS (Soul, Agent instructions, Harness, Skills, Plan mode instructions, Active plan, Memory, SDK reference). These components rarely change within a session — Soul, Skills, Harness, and SDK reference are stable for the session's lifetime. Only `session_mode` and `active_plan_id` are volatile.

### No provider-level prefix caching

The Anthropic Messages API supports `cache_control` breakpoints that let the provider cache prefix content across requests, reducing input token costs by approximately 90% on the cached portion. OpenPaw does not use this feature, so every turn pays full input token cost for the entire system prompt and conversation prefix. On a typical 10+ turn conversation with a ~10K token system prompt, this represents substantial unnecessary cost.

### No incremental tool result management

Context compaction only triggers when the full context window is nearly exhausted. There is no intermediate strategy for reducing token load between turns. Old tool results — which are often large and decrease in relevance with each subsequent turn — remain in full until the all-or-nothing compaction threshold is hit.

### Web-fetched content loses document structure

The `web_fetch` WASM module strips HTML using a character-by-character tag remover that produces flat text. All document structure — headings, links, lists, emphasis, code blocks — is lost. A page with `<h2>API Reference</h2><ul><li><a href="/docs">Docs</a></li></ul>` becomes `API Reference Docs`, losing hierarchy and link targets. This reduces the LLM's ability to reason about the content's structure and makes citations unreliable.

## Decision

### 1. Anthropic prompt caching

Send the system prompt as a structured content block with `cache_control: {"type": "ephemeral"}`. Add cache breakpoints to up to 2 recent conversation messages (the Anthropic API supports 4 breakpoints total). Parse and log `cache_read_input_tokens` and `cache_creation_input_tokens` from responses.

This is the highest-impact change: on multi-turn conversations, it reduces re-read costs by approximately 90% on the cached prefix. The change is additive — only the `call_anthropic` code path is modified. OpenRouter and OpenAI Codex paths are untouched.

### 2. System prompt stability via hash cache

Compute a hash of the component inputs (soul_id, agent_id, project_harness_id, session_mode, active_plan_id, tools_enabled) before fetching. Cache the assembled prompt in a TemperFS file and store its hash alongside the file ID in session entity state. On subsequent turns, compare the hash — if unchanged, read the cached prompt (1 HTTP call) instead of re-fetching all components (7-9 HTTP calls).

Since WASM modules are stateless across invocations, the cache must be persisted in entity state fields (`system_prompt_hash`, `system_prompt_file_id`) and passed through action params across the state machine loop.

### 3. Pre-compaction tool result pruning

Before checking the compaction threshold, replace `tool_result` content blocks older than a configurable number of turns (default: 4) with a stub: `"[tool result pruned — N chars]"`. This is a pure text-replacement operation requiring no LLM call. It extends usable context window life significantly in tool-heavy sessions.

Only tool results are pruned — user text and assistant messages are never modified. The pruning threshold is configurable via session state (`prune_tool_results_after_turns`).

### 4. HTML-to-markdown conversion for web fetch

Replace the character-by-character `strip_html()` function with `html_to_markdown()` that preserves document structure. The converter handles common structural tags:

- Headings (`<h1>`–`<h6>`) → `#`–`######` prefixes
- Links (`<a href>`) → `[text](url)` markdown links
- Lists (`<ul>/<ol>/<li>`) → `- ` and `1. ` prefixes with nesting
- Emphasis (`<strong>/<em>`) → `**bold**` / `*italic*`
- Code (`<code>/<pre>`) → inline backticks / fenced code blocks
- Block elements (`<p>/<br>/<blockquote>`) → appropriate whitespace / `> ` prefix
- Script/style blocks → skipped entirely (existing behavior)
- Unknown tags → stripped silently (existing behavior)

The implementation is hand-rolled (no external crate dependency) to ensure `wasm32-unknown-unknown` compatibility. No entity or state machine changes — only the content quality improves.

### Deferred: Web fetch LLM-assisted summarization

Invoking a secondary LLM call from within a WASM module that is itself triggered by a different entity's state machine would violate the ONE-ONE rule (ADR-0005). The correct approach would be a new `Summarizing` state in the `WebQuery` entity with its own WASM integration. This is a separate design effort and is deferred to a future ADR.

## Consequences

### Positive

- Anthropic prompt caching alone can reduce per-turn input token costs by 75-90% for multi-turn sessions
- System prompt stability eliminates 7-9 unnecessary HTTP calls per turn in the common case
- Tool result pruning extends context window life by 2-5x in tool-heavy sessions with no LLM cost
- HTML-to-markdown conversion gives agents structured, citable content instead of flat text walls
- All changes are isolated to WASM modules, requiring no platform changes

### Negative

- `cache_control` is Anthropic-specific; OpenRouter and OpenAI paths get no benefit
- System prompt caching introduces invalidation complexity; a stale prompt could cause incorrect behavior if a component changes mid-session (mitigated by hash check on every turn)
- Tool result pruning loses information; agents cannot re-read pruned results (mitigated by keeping recent turns intact and by the existing compaction summary which captures key findings)
- The hand-rolled markdown converter will not handle all HTML edge cases (malformed markup, deeply nested structures, exotic tags); this is acceptable because the fallback is the same as current behavior — unknown tags are stripped
