# ADR-0008: Monty REPL for Agent Tool Execution

## Status

Proposed

## Context

OpenPaw agents interact with Temper through 19 discrete LLM tools (`temper_create`, `temper_get`, `bash`, `read`, `spawn_agent`, etc.) dispatched by a monolithic `tool_runner` WASM module (~2400 lines across 3 files). This architecture has several problems:

1. **Diverges from Temper pattern.** Temper's MCP interface (`mcp__temper__execute`) gives agents a single `execute` tool backed by a Monty Python REPL. Agents write `await temper.create(...)`, `await temper.action(...)` — one interface, all operations. OpenPaw agents instead see 19 separate tool schemas (~410 lines of JSON definitions) loaded into every LLM call, including duplicate entries.

2. **Named tools are redundant.** Most named tools are thin wrappers over generic entity operations:
   - `save_memory` = `temper_create("Memories", {...})`
   - `recall_memory` = `temper_list("Memories", filter=...)`
   - `spawn_agent` = `temper_create("Agents")` + `temper_action("Configure")` + `temper_action("Provision")`
   - legacy `file_upload` = `temper.write(path, content)` = `CreateFile` + native `PUT Files(...)/$value`

3. **No self-provisioning.** Temper supports hot-loading specs (`POST /api/specs/load-inline`), WASM modules (`POST /api/wasm/modules/<name>`), and OS apps at runtime. Agents have no access to these APIs despite being the primary consumers of new capabilities.

4. **No evolution observability.** Temper automatically records failed intents as trajectory entries and generates insights. Agents cannot read this data (`get_trajectories`, `get_insights`) to close the feedback loop.

5. **No persistent REPL.** Each tool call is stateless. Variables from one call cannot be referenced in the next. Multi-step workflows require multiple LLM↔tool round trips instead of a single script execution.

## Decision

Replace the `tool_runner` WASM module with a `monty_repl` module that embeds Pydantic's Monty Python sandbox, compiled to WASM. Agents get a single `execute` tool that accepts Python code — the same interface as `mcp__temper__execute`.

### Architecture

```
LLM ──▶ execute({code: "..."}) ──▶ WASM [Monty Python REPL]
                                     │
                  temper.create()  ──▶ Temper API (local, ~1ms)
                  temper.action()  ──▶ Temper API (local)
                  temper.submit_specs() ──▶ Temper API
                  temper.get_trajectories() ──▶ Temper API
                  │
                  sandbox.bash()  ──HTTP──▶ Tensorlake sandbox
                  sandbox.read()  ──HTTP──▶ Tensorlake sandbox
```

- Monty interpreter runs on the Temper server (in WASM), not in the Tensorlake sandbox
- Temper API calls are local (same process) — no network hop
- Sandbox operations go over HTTP to Tensorlake (same as current tool_runner)
- REPL state persists across LLM turns via Monty's `dump()`/`load()` serialization, stored in an entity field

### Method set

The dispatch layer maps Python method calls to HTTP endpoints, mirroring the Temper SDK:

| Method | HTTP Call |
|--------|----------|
| `temper.create(entity_set, body)` | `POST /tdata/{entity_set}` |
| `temper.get(entity_set, id)` | `GET /tdata/{entity_set}('{id}')` |
| `temper.list(entity_set, ...)` | `GET /tdata/{entity_set}?$filter=...` |
| `temper.action(entity_set, id, action, params)` | `POST /tdata/{entity_set}('{id}')/{action}` |
| `temper.patch(entity_set, id, body)` | `PATCH /tdata/{entity_set}('{id}')` |
| `temper.submit_specs(specs)` | `POST /api/specs/load-inline` |
| `temper.show_spec(entity_type)` | `GET /api/specs/{tenant}/{entity_type}` |
| `temper.upload_wasm(module, bytes)` | `POST /api/wasm/modules/{module}` |
| `temper.compile_wasm(module, source)` | `POST /api/wasm/compile/{module}` |
| `temper.install_app(app_name)` | Creates CapabilityRequest entity (Cedar-governed) |
| `temper.get_trajectories(...)` | `GET /api/evolution/trajectories` |
| `temper.get_insights()` | `GET /api/evolution/insights` |
| `temper.get_decisions()` | `GET /api/decisions` |
| `temper.poll_decision(id)` | Polls until resolved |
| `sandbox.read(path)` | `GET {sandbox_url}/api/v1/files?path=...` |
| `sandbox.write(path, content)` | `PUT {sandbox_url}/api/v1/files?path=...` |
| `sandbox.edit(path, old, new)` | Read + search-replace + write |
| `sandbox.bash(command)` | `POST {sandbox_url}/api/v1/processes` + poll |

### Self-provisioning

A new `CapabilityRequest` entity type governs self-provisioning:

```
Requested ──[Approve]──▶ Installing ──[InstallComplete]──▶ Installed
    │                        │
    └──[Reject]──▶ Rejected  └──[InstallFailed]──▶ Failed
```

When an agent calls `await temper.install_app("paw-github")`, the dispatch layer creates a CapabilityRequest entity. Cedar policies control whether the request auto-approves or requires human approval via the existing Discord/Observe UI flow. A `capability_installer` WASM integration handles the actual installation (calling platform APIs to load specs, WASM, or apps).

## Alternatives Considered

### 1. JSON ops restructure (single `execute` tool with structured JSON)

Replace 19 tools with one `execute` tool accepting `{"op": "create", "entity_set": "Issues", ...}` JSON commands. A WASM router dispatches by `op` field.

**Rejected because:** This is cosmetically different but architecturally identical to the current approach — just routing by `op` instead of tool name. No persistent state, no multi-statement scripts, no reduction in WASM complexity. The dispatch code moves but doesn't shrink.

### 2. Python SDK in Tensorlake sandbox

Write a lightweight `temper_sdk.py` HTTP wrapper, install in the agent's Tensorlake sandbox, send Python code there for execution.

**Rejected because:** Temper API calls would require an extra network hop (sandbox → Temper server). File/shell operations would be native (fast), but the agent's execution context is less governed — full CPython in a container vs restricted Monty interpreter. Also requires maintaining a persistent Python process in the sandbox for state persistence.

### 3. Incremental tool additions

Keep the current 19-tool architecture, just add new tools (`temper_submit_specs`, `temper_get_trajectories`, etc.).

**Rejected because:** This ships the platform capabilities but doesn't address the architectural divergence from Temper's REPL pattern, the context bloat, or the lack of persistent state.

## Consequences

### Positive

- **Temper alignment:** Agent tool interface matches `mcp__temper__execute` exactly. Same method names, same Python code, same dispatch semantics.
- **Self-provisioning:** Agents can submit specs, upload WASM, install apps — governed by Cedar.
- **Evolution loop:** Agents can read trajectories and insights to close the feedback loop.
- **Persistent REPL:** Variables survive across LLM turns. Multi-step workflows execute in a single call.
- **Context savings:** One tool schema (~3 lines) + concise SDK reference in system prompt replaces ~410 lines of JSON schemas.
- **Code reduction:** ~2400 lines of tool_runner dispatch replaced by ~200 lines of method dispatch + Monty.

### Negative

- **Monty WASM patching:** Two standard library features (`std::time::Instant`, `std::sync::LazyLock`) need WASM-compatible shims. Requires a compatibility wrapper crate.
- **New host function:** `host_get_time_millis()` must be added to temper-wasm-sdk and implemented in the WASM engine (Temper repo change).
- **WASM binary size:** Embedding a Python interpreter increases the monty_repl module size significantly compared to the current tool_runner.
- **Monty maintenance:** We depend on Pydantic's Monty crate. Breaking changes in Monty require updating the compatibility wrapper.
- **State serialization overhead:** `dump()`/`load()` adds latency to each tool execution round trip. Must be measured.

## Implementation

See plan file for detailed implementation phases:
1. Fork/patch Monty for WASM compatibility
2. Create `monty_repl` WASM module with dispatch layer
3. Replace `build_tool_definitions()` with single `execute` tool
4. Build CapabilityRequest entity + capability_installer
5. End-to-end verification
