# paw-research

Governed web search and URL fetching for agent research. Every query is an auditable entity with Cedar policy enforcement.

## Entity Types

### WebQuery
Single web search or URL fetch operation.

- **States**: Created -> Executing -> Complete / Failed
- **Key actions**:
  - `ExecuteSearch` (query) — search via Exa API
  - `ExecuteFetch` (url) — fetch URL and extract text content
  - `RecordResults` — WASM stores results on success
  - `RecordError` — WASM stores error on failure
- **WASM**: `web_search` (Exa API), `web_fetch` (HTTP fetch + HTML stripping)

Agent usage via dispatch wrappers:
```
temper.web_search("rust async patterns")
temper.web_fetch("https://docs.rs/serde_json/latest/serde_json/")
```

## Setup

Depends on `paw-agent` for agent context. Requires Exa API key secret (`exa_api_key`).
