# ADR-0024: Sandbox Provider Abstraction

**Status:** Accepted
**Date:** 2026-04-09
**Supersedes:** Portions of ADR-0007 (single-provider assumption)

## Context

All sandbox operations across 4 WASM modules (`sandbox_provisioner`, `monty_repl`, `coding_agent_runner`, `workspace_restorer`) were hardcoded to Tensorlake's Firecracker MicroVM API. This prevented switching to alternative sandbox providers at runtime. The `llm_caller` module already demonstrated the pattern for multi-provider dispatch (Anthropic, OpenRouter, OpenAI) — sandbox needed the same treatment.

Modal is the primary alternative provider. However, Modal's Python SDK has **no REST API** for sandbox lifecycle (`Sandbox.create()`, `exec()`, `read_file()`, `write_file()` all require the Python SDK). This is the same constraint documented in ADR-0007.

## Decision

### 1. Centralized sandbox abstraction in `wasm-helpers/src/sandbox.rs`

All sandbox operations are consolidated into a single module with match-based dispatch (no `dyn Trait` — WASM-compatible):

- `resolve_sandbox_provider(ctx, fields) -> String` — reads provider from entity field, config, or defaults to "tensorlake"
- `resolve_sandbox_api_key(ctx, provider) -> Result<String>` — per-provider credential lookup
- `sandbox_create`, `sandbox_health_check`, `sandbox_file_read/write/delete`, `sandbox_exec`, `sandbox_setup`

Each function dispatches to provider-specific implementations:
```rust
match provider {
    "tensorlake" => tensorlake_create(ctx, config),
    "modal"      => modal_create(ctx, config),
    _            => Err(format!("unsupported sandbox provider: {provider}")),
}
```

### 2. Modal REST bridge

Since Modal has no HTTP API, a thin Python FastAPI web endpoint deployed ON Modal wraps the SDK as REST. Lives at `os-apps/paw-agent/modal-bridge/`. The WASM modules call this bridge via standard HTTP — no Python dependency in the Rust/WASM stack.

Endpoints:
- `POST /sandboxes` — create sandbox
- `GET /sandboxes/{id}/health` — readiness check
- `GET/PUT/DELETE /sandboxes/{id}/files` — file operations
- `POST /sandboxes/{id}/exec` — synchronous command execution
- `DELETE /sandboxes/{id}` — terminate sandbox

Auth: Bearer token (`MODAL_API_TOKEN`), checked by the bridge.

### 3. Provider selection via config

```
.env (SANDBOX_PROVIDER, TL_API_KEY, MODAL_API_TOKEN, MODAL_API_URL)
  → config.rs → startup.rs secrets vault
  → session.ioa.toml {secret:*} → ctx.config in WASM
  → sandbox::resolve_sandbox_provider() reads entity field or config
```

### 4. Deduplication

| Previously Duplicated | Files | Now |
|---|---|---|
| Sandbox creation | `sandbox_provisioner`, `dispatch.rs` | `sandbox.rs::sandbox_create()` |
| Health check | `sandbox_provisioner`, `dispatch.rs` | `sandbox.rs::sandbox_health_check()` |
| File I/O | `dispatch.rs`, `coding_agent_runner`, `workspace_restorer` | `sandbox.rs::sandbox_file_*()` |
| Exec (output-redirection) | `dispatch.rs`, `coding_agent_runner` | `sandbox.rs::sandbox_exec()` |
| `url_encode()` | 3 files | `sandbox.rs::url_encode()` |
| gh CLI setup | `sandbox_provisioner`, `dispatch.rs` | `sandbox.rs::sandbox_setup()` |

## Consequences

- Adding a new sandbox provider requires implementing ~8 functions in `sandbox.rs` and adding credentials to config/secrets
- Modal support requires deploying the REST bridge (`modal deploy modal_bridge.py`)
- The `sandbox_provider` field propagates through entity state, so different sessions can use different providers
- Tensorlake remains the default when `SANDBOX_PROVIDER` is not set
