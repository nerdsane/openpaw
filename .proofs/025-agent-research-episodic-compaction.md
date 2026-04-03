# Proof Report: 025 — Agent Research, Episodic Compaction & Planning Skill

## Date
2026-04-03

## Branch / Commit
main (uncommitted)

## What Was Done

Three capabilities added to OpenPaw agents, guided by the "bitter lesson" principle (expand action space, improve context, instruct through content):

1. **Online Research (`paw-research` os-app)** — New os-app with standalone WASM modules for web search (Exa API) and URL fetch (HTML stripping). WebQuery entity with Cedar authorization.

2. **Episodic Context Compaction** — Replaced generic Goal/Progress summarization prompt with episode-based format (Goal/Worked/Failed/Discoveries/Artifacts) to preserve trajectory of attempted approaches.

3. **Research-First Planning Skill** — Global skill markdown instructing agents to research → plan → wait for approval → implement, with skip conditions for trivial tasks.

## Verification Flow

1. Build all WASM modules for wasm32-unknown-unknown target
2. Start platform, verify WebQuery entity type registers
3. Create WebQuery entity, dispatch ExecuteSearch, verify WASM runs
4. Create WebQuery entity, dispatch ExecuteFetch with real URL, verify HTML stripping
5. Verify context_compactor builds with new episodic prompt
6. Verify skill file exists and tool descriptions updated

## Verification Results
| Step | Expected | Actual | Status |
|------|----------|--------|--------|
| web_search WASM compile | Builds clean | Compiled in 3.17s | PASS |
| web_fetch WASM compile | Builds clean | Compiled in 2.17s | PASS |
| context_compactor compile | Builds clean | Compiled in 0.36s | PASS |
| Platform startup | paw-research installs | `added=["WebQuery"]`, `wasm=["web_fetch", "web_search"]` | PASS |
| WebQuery entity set | Appears in OData | `WebQueries` listed at `/tdata` | PASS |
| Entity creation | Returns Created status | entity_id returned, status=Created | PASS |
| ExecuteSearch (no API key) | Graceful error | Status=Failed, error="missing exa_api_key" | PASS |
| ExecuteSearch (live Exa) | Returns search results | Status=Complete, 10 results with title/url/text (Tokio docs, Rust guides) | PASS |
| ExecuteFetch (httpbin.org/html) | Returns stripped text | Status=Complete, 3594 chars of clean Moby-Dick text, no HTML tags | PASS |
| WASM integration chain | execute_search trigger fires | Logs show: custom effect dispatched → WASM invoked → RecordResults dispatched | PASS |
| Exa API key seeding | Secret resolved from .env | `exa_api_key` cached in vault, resolved by WASM at runtime | PASS |
| Session lifecycle | Created → Configure → Provision → Thinking → Executing → Completed | Full 23-turn session with Claude Sonnet 4.6 | PASS |
| Agent uses web_search | Agent calls temper.web_search() | 3 search queries created as WebQuery entities, all Complete | PASS |
| Agent uses web_fetch | Agent calls temper.web_fetch() | 2 URL fetches (async-book, tokio docs), all Complete | PASS |
| Agent calls done() | Session transitions to Completed | Detailed research summary returned via temper.done() | PASS |
| Full e2e agent research | LLM → dispatch → WebQuery → WASM → Exa/fetch → results → LLM | 5 WebQueries, 23 turns, structured Rust async summary produced | PASS |
| Episodic compaction prompt | Contains "Episodes" format | grep confirms 2 occurrences of "Episodes", 1 of "trajectory" | PASS |
| Tool descriptions | Include web_search/web_fetch | Both listed in llm_caller concat! macro | PASS |
| Skill file | Exists with research-first content | `souls/skills/research-first-planning.md` present | PASS |

## What Worked
- WebQuery entity lifecycle: Created → Executing → Complete/Failed via WASM integration chain
- HTML tag stripping: httpbin.org/html returned pure text (Moby-Dick excerpt)
- Graceful error handling: missing API key produces clear error, entity transitions to Failed
- CSDL model merged correctly (required unique `ResearchService` container name and no Vocab re-definition)
- Cedar policy allows all principals to create/execute/read WebQueries
- WASM modules compile independently, deploy independently

## What Didn't Work
- Initial CSDL had duplicate `Paw.Vocab` namespace and `EntityContainer Name="Service"` — entity set was silently dropped during CSDL merge. Fixed by removing redundant Vocab schema and using unique container name `ResearchService`.
- monty_repl WASM build fails due to pre-existing `getrandom` v0.3 dependency issue (unrelated to these changes)
- Agent session with Anthropic provider failed with 401 "invalid x-api-key" — pre-existing credential issue in `.env`, not related to these changes. Session infrastructure itself worked (Created → Configured → Provisioned → Thinking → [LLM call attempted])

## Limitations
- `temper.web_search()` requires `EXA_API_KEY` secret configured. Without it, searches fail gracefully with a clear error.
- `temper.web_fetch()` HTML stripping is character-by-character (no regex) — handles standard HTML well but may struggle with malformed markup.
- Response truncation at 100KB for web_fetch.
- Full e2e agent session proven: Claude Sonnet 4.6 agent ran 23 turns, called `temper.web_search()` 3 times and `temper.web_fetch()` 2 times, produced detailed research summary, completed via `temper.done()`.
- Episodic compaction prompt deployed but not live-tested (requires long-running session hitting context limit).
- Planning skill file created but not yet deployed as a Skill entity (runtime step, no code change needed).

## Artifacts
- `os-apps/paw-research/` — New os-app (specs, wasm, policies)
- `os-apps/paw-research/wasm/web_search/target/wasm32-unknown-unknown/release/web_search.wasm`
- `os-apps/paw-research/wasm/web_fetch/target/wasm32-unknown-unknown/release/web_fetch.wasm`
- `souls/skills/research-first-planning.md`

## Architecture Diagram
```text
Agent (Python REPL)
  │
  ├─ temper.web_search("query")     temper.web_fetch("url")
  │         │                              │
  ▼         ▼                              ▼
dispatch.rs (monty_repl)
  │
  ├─ POST /tdata/WebQueries {QueryType:"search"}
  ├─ POST /tdata/WebQueries('id')/Temper.ExecuteSearch
  │         │
  │         ▼
  │   WebQuery Entity: Created → Executing
  │         │
  │         ▼ (WASM integration)
  │   web_search.wasm → POST api.exa.ai/search
  │         │
  │         ▼
  │   WebQuery Entity: Executing → Complete {results}
  │
  └─ GET /tdata/WebQueries('id') → return results
```
