# Proof Report: 036 — Sandbox Provider Abstraction (ADR-0024)

## Date
2026-04-11

## Branch / Commit
`feat/sandbox-provider-abstraction` @ `6a80dec8`

## What Was Done

Introduced a centralized sandbox provider abstraction (`wasm-helpers/src/sandbox.rs`) that enables runtime selection between Tensorlake and Modal sandbox providers. Follows the existing `llm_caller` match-based dispatch pattern. Eliminates ~400 lines of duplicated sandbox code across 4 WASM modules.

### Files Changed
- **NEW** `os-apps/paw-agent/wasm/wasm-helpers/src/sandbox.rs` — Core abstraction (8 public functions, match-based dispatch)
- **NEW** `os-apps/paw-agent/modal-bridge/modal_bridge.py` — Python FastAPI bridge deployed on Modal (wraps SDK as REST)
- **NEW** `docs/adrs/0024-sandbox-provider-abstraction.md` — Architecture decision record
- **MOD** `os-apps/paw-agent/wasm/wasm-helpers/src/lib.rs` — `pub mod sandbox;`
- **MOD** `os-apps/paw-agent/wasm/sandbox_provisioner/src/lib.rs` — Uses `sandbox::*` instead of hardcoded Tensorlake
- **MOD** `os-apps/paw-agent/wasm/monty_repl/src/dispatch.rs` — Lazy provisioning + tool dispatch via abstraction
- **MOD** `os-apps/paw-agent/wasm/monty_repl/src/lib.rs` — 3-tuple `(url, id, provider)` in lazy sandbox state
- **MOD** `os-apps/paw-agent/wasm/monty_repl/src/entity_ops.rs` — `run_coding_agent` via `sandbox::sandbox_exec`
- **MOD** `os-apps/paw-agent/wasm/coding_agent_runner/src/lib.rs` — Uses `sandbox::sandbox_exec`
- **MOD** `os-apps/paw-agent/wasm/coding_agent_runner/Cargo.toml` — Added `wasm-helpers` dep
- **MOD** `crates/openpaw/src/config.rs` — `sandbox_provider`, `modal_api_token`, `modal_api_url` fields
- **MOD** `crates/openpaw/src/startup.rs` — Provider-aware secret seeding + validation
- **MOD** `os-apps/paw-agent/specs/session.ioa.toml` — `sandbox_provider` state + config in integrations

## Verification Flow

Full E2E via curl against running daemon — no Discord transport required.

### Tensorlake E2E
1. Set `SANDBOX_PROVIDER=tensorlake` in `.env`
2. Build WASM (`./build.sh`), start daemon (`cargo run`)
3. Create session via `POST /tdata/Sessions`
4. Configure via `POST /tdata/Sessions('{id}')/OpenPaw.Configure` with `provider: "openai", model: "gpt-5.2"`
5. Approve Cedar decision via `/api/tenants/default/decisions/{PD}/approve`
6. Poll session state until completion

### Modal E2E
1. Deploy Modal bridge (`modal deploy modal_bridge.py`)
2. Verify bridge directly: create, health, file write, file read, exec, file delete, terminate (all via curl)
3. Set `SANDBOX_PROVIDER=modal`, rebuild WASM, restart daemon
4. Create + configure session, approve Cedar decision
5. Poll session state until completion

## Verification Results

| Step | Expected | Actual | Status |
|------|----------|--------|--------|
| WASM build | All 14 modules compile | All 14 modules compile clean | PASS |
| Tensorlake session lifecycle | Created → Provisioning → Thinking → Executing → Completed | Created → Provisioning → Thinking → Executing → Steering → Completed | PASS |
| Tensorlake sandbox_provider in state | `sandbox_provider=tensorlake` | `sandbox_provider=tensorlake` | PASS |
| Modal bridge deploy | 7 endpoints deployed | 7 endpoints at `n-seshendra--openpaw-sandbox-bridge-*.modal.run` | PASS |
| Modal bridge create | Returns `sandbox_id` | `sandbox_id: sb-kQlCZVRVb4DuGNb0J7C0cL` | PASS |
| Modal bridge health | `{ready: true}` | `{ready: true, status: running}` | PASS |
| Modal bridge file write | `{ok: true}` | `{ok: true, path: /tmp/test.py}` | PASS |
| Modal bridge file read | Returns content | Returns exact content written | PASS |
| Modal bridge exec | Returns stdout/exit_code | `stdout: "hello from modal"`, `exit_code: 0` | PASS |
| Modal bridge file delete | `{ok: true}` | `{ok: true}` | PASS |
| Modal bridge terminate | `{ok: true}` | `{ok: true}` | PASS |
| Modal full session lifecycle | Created → ... → Completed with `sandbox_provider=modal` | Created → Provisioning → Thinking → 6 turns → Steering → Completed | PASS |
| Modal sandbox_id in state | Non-empty sandbox_id | `sandbox_id: sb-pPE9jZgbZ1yLKfYNdAXolH` | PASS |
| Provider switch | Changing .env switches provider | Tensorlake then Modal, both complete | PASS |

## What Worked
- Match-based dispatch cleanly follows the `llm_caller` pattern
- Lazy sandbox provisioning (ADR-0022) works with both providers
- Modal bridge auto-scales to zero on Modal — no infrastructure to maintain
- The sandbox_provider field propagates through entity state correctly
- Both providers complete the full session lifecycle (including multi-turn tool use)

## What Didn't Work
- Initial Modal bridge used deprecated `@modal.web_endpoint` and `allow_concurrent_inputs` — fixed to `@modal.fastapi_endpoint` + `@modal.concurrent`
- Modal SDK `read_file()`/`write_file()` don't exist — had to use `sb.open(path, mode)` pattern
- Modal `result.returncode` requires `result.wait()` first — added
- FastAPI `body: dict` expects direct JSON, not wrapped in `{"body": {...}}`
- Auth must be passed as query param (Modal strips custom headers from web endpoints)

## Limitations
- Modal bridge requires a separate deployment (`modal deploy modal_bridge.py`)
- Modal has no REST API for sandboxes — the Python bridge is required
- Modal bridge auth uses a shared Bearer token (not per-user tokens)
- `ANTHROPIC_API_KEY` was empty; tests used OpenAI (`gpt-5.2`) provider

## What Still Doesn't Work
- No workspace_restorer changes yet (deferred — it already uses wasm-helpers and is lower priority)
- No automated tests for sandbox.rs (unit tests would require mocking `ctx.http_call`)

## Artifacts
- Modal bridge deployed at: `https://n-seshendra--openpaw-sandbox-bridge-*.modal.run`
- Tensorlake session: `ss-019d7d3f-ee3b-7f68-9abe-6f5b8ae19a75` (from previous E2E)
- Modal session: `ss-019d7d48-78a2-7a81-8635-252ba212c106`

## Architecture Diagram
```text
.env (SANDBOX_PROVIDER=tensorlake|modal)
  |
  v
config.rs --> startup.rs --> secrets vault
  |                              |
  v                              v
session.ioa.toml             {secret:sandbox_provider}
  |                          {secret:modal_api_token}
  |                          {secret:modal_api_url}
  v
WASM module (sandbox_provisioner / monty_repl / coding_agent_runner)
  |
  v
sandbox::resolve_sandbox_provider(ctx, fields)
  |
  +-- match "tensorlake" --> tensorlake_create/exec/file_*
  |       |                     |
  |       v                     v
  |   api.tensorlake.ai    Firecracker MicroVM
  |
  +-- match "modal" ---------> modal_create/exec/file_*
          |                     |
          v                     v
      Modal REST Bridge    Modal Sandbox (Python SDK)
      (FastAPI on Modal)   (auto-scales to zero)
```
