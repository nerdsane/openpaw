# ADR-0022: Lazy Sandbox Provisioning

**Status:** Accepted
**Date:** 2026-04-09
**Supersedes:** Partial supersession of ADR-0007 (Tensorlake sandbox migration) — sandbox is still Tensorlake, but provisioning timing changes.

## Context

Every agent session eagerly provisions a Tensorlake Firecracker MicroVM sandbox (2 CPUs, 4 GB RAM) before the agent can begin thinking. The provisioning flow is:

```
Configure → auto-schedule Provision(delay=0) → Provisioning
  → provision_sandbox WASM (create sandbox + poll readiness + create TemperFS workspace)
  → SandboxReady → Thinking → call_llm
```

This design has three critical problems:

1. **Latency**: Every DM response waits 60–120 seconds for sandbox boot, even when the agent only needs to call `temper.*` API tools or respond with text. The vast majority of agent interactions don't require code execution.

2. **Session death on infrastructure failure**: The `provision_sandbox` integration uses `on_failure = "Fail"`, making sandbox provisioning failure terminal. Once a session enters `Failed`, it cannot be resumed. When Tensorlake returns HTTP 400 (e.g., sandboxes stuck in "pending" status), the session dies silently.

3. **Doom loop**: When a user sends a follow-up DM after a failed session, `route_message` creates a new agent that immediately hits the same Tensorlake failure. The user sees raw infrastructure errors or silence, with no way to break the cycle.

Additionally, the `provision_sandbox` WASM module conflates two independent concerns: Tensorlake sandbox creation (slow, sometimes fails) and TemperFS workspace creation (fast, always needed for conversation storage).

## Decision

### 1. Decouple workspace from sandbox provisioning

Extract TemperFS workspace/conversation/session-tree creation into a new `workspace_provisioner` WASM module. This is fast (sub-second, local API calls) and always needed. The `Configure` action schedules `ProvisionWorkspace` instead of `Provision`.

### 2. Make sandbox provisioning lazy

The session goes directly from `Provisioning` (workspace only) → `Thinking` without a sandbox. When the agent calls a tool that requires code execution (`sandbox.bash`, `sandbox.read`, `sandbox.write`, `sandbox.edit`, `temper.run_coding_agent`), `monty_repl` provisions the sandbox synchronously via `ctx.http_call()` to the Tensorlake API.

The provisioned `sandbox_url` and `sandbox_id` are:
- Cached in a thread-local for the current WASM invocation (handles multiple sandbox tool calls in one turn)
- Persisted to entity state via `HandleToolResults` params (survives across turns, compaction, steering)

### 3. Sandbox failure = tool error, not session death

If lazy sandbox provisioning fails, the error is returned as a tool result to the LLM:
```
"This tool requires a code execution sandbox, but sandbox provisioning failed: {error}.
You can still use non-sandbox tools (temper.create, temper.list, temper.web_search, etc.)
to help the user."
```

The session stays alive. The agent can explain the failure, suggest alternatives, or retry later.

### 4. Keep existing Provision flow for Resume

The `Provision` → `SandboxReady` state machine path is preserved for the `Resume` action, which restores a session with a known sandbox URL.

## Consequences

### Positive

- **Sub-5-second response for text-only interactions** — no sandbox boot for the common case
- **No more session death from infrastructure failures** — sandbox failures are recoverable
- **No more doom loop** — failed sandbox provisioning doesn't cascade across DMs
- **Cost reduction** — sandboxes only created when actually needed
- **Clear separation of concerns** — workspace provisioner (fast, reliable) vs sandbox (slow, fallible)

### Negative

- **First sandbox tool call has ~60s latency** — user waits on first code execution (mitigated by heartbeat/typing indicator)
- **Sandbox provisioning logic duplicated** — exists in both `sandbox_provisioner` (Resume flow) and `monty_repl/dispatch.rs` (lazy flow). Could be extracted to `wasm-helpers` if duplication becomes a maintenance burden.
- **More complex tool dispatch** — `dispatch()` now has lazy provisioning logic interleaved with tool routing

### Tool categorization

Tools that trigger lazy sandbox provisioning:
- `sandbox.bash()`, `sandbox.read()`, `sandbox.write()`, `sandbox.edit()`
- `temper.run_coding_agent()`

Tools that work without a sandbox (all `temper.*` API tools):
- `temper.create()`, `temper.get()`, `temper.list()`, `temper.action()`, `temper.patch()`
- `temper.web_search()`, `temper.web_fetch()`
- `temper.save_memory()`, `temper.recall_memory()`
- `temper.spawn_session()`, `temper.steer_session()`
- All other `temper.*` tools
