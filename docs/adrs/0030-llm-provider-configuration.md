# ADR-0030: LLM Provider Configuration Architecture

**Status:** Accepted
**Date:** 2026-04-14
**Related:** ADR-0029 (deployment architecture), ADR-0028 (bounded startup surface)

## Context

OpenPaw supports multiple LLM providers: Anthropic, OpenAI (Codex), and OpenRouter. Users configure their preferred provider and API key through the dashboard settings page, which stores them in the Temper secrets vault. However, the codebase had `"anthropic"` and `"claude-sonnet-4-6"` hardcoded as defaults in 15+ locations across Rust startup code, WASM modules, entity specs, and API handlers.

This caused a critical runtime failure: when a user configured a non-Anthropic provider (e.g., OpenAI Codex), agent sessions would crash with `provider=anthropic api key is unresolved secret template: '{secret:anthropic_api_key}'`. The error occurred because WASM modules and bootstrap code defaulted to Anthropic regardless of what the user had configured.

The root cause was the absence of a single source of truth for LLM configuration. Each component independently assumed Anthropic as the default, creating a scattered dependency that was invisible until a non-Anthropic user hit the failure path.

## Decision

### 1. Vault as single source of truth

The secrets vault stores two authoritative values: `llm_provider` (the provider name) and `llm_model` (the default model for that provider). All runtime code resolves these from the vault rather than hardcoding defaults.

The resolution chain for any component needing the provider is:

1. **Per-entity state** (agent/session `provider` field) -- highest priority, allows per-agent override
2. **Vault secret** (`llm_provider`) -- the platform default set by the user via dashboard
3. **Environment variable** (`LLM_PROVIDER`) -- for deployment-time configuration
4. **Hardcoded fallback** (`"anthropic"`) -- last resort only, never the first choice

### 2. Provider-aware model defaults

When `llm_model` is not explicitly set, the default is derived from the provider:

| Provider    | Default model              |
|-------------|----------------------------|
| `anthropic` | `claude-sonnet-4-6`        |
| `openai`    | `o3-mini`                  |
| `openrouter`| `anthropic/claude-sonnet-4`|

The `LLM_MODEL` environment variable overrides these defaults for all providers.

### 3. WASM auto-fallback for API keys

WASM modules (`llm_caller`, `context_compactor`) receive all provider API keys via their integration config:

```toml
[integration.config]
api_key = "{secret:anthropic_api_key}"
anthropic_api_key = "{secret:anthropic_api_key}"
openai_codex_access_token = "{secret:openai_codex_access_token}"
openai_codex_account_id = "{secret:openai_codex_account_id}"
openai_codex_token = "{secret:openai_codex_token}"
openrouter_api_key = "{secret:openrouter_api_key}"
```

At runtime, the WASM module:
1. Reads the configured provider from entity state
2. Looks up that provider's API key from config
3. If the key is an unresolved secret template (contains `{secret:`), tries alternative providers
4. Logs a warning when falling back, so the operator knows the configuration is incomplete
5. Only errors if no provider has a valid key

This design ensures that a session never hard-fails when any valid API key exists, while still logging the misconfiguration for the operator to fix.

### 4. Provider-aware API endpoints

Each WASM module selects the correct API endpoint based on the resolved provider:

| Provider    | Endpoint                                          |
|-------------|---------------------------------------------------|
| `anthropic` | `https://api.anthropic.com/v1/messages`           |
| `openai`    | `https://api.openai.com/v1/responses`             |
| `openai_codex` | `https://chatgpt.com/backend-api/codex/responses` |
| `openrouter`| `https://openrouter.ai/api/v1/chat/completions`  |

The endpoint URL is not configurable per-entity; it is derived from the provider name. This prevents mismatches between provider and endpoint.

`openai_codex` is a ChatGPT/Codex subscription route, not a public OpenAI
Platform API route. See ADR-0044.

### 5. Startup seeding

On server boot, `startup.rs` seeds both `llm_provider` and `llm_model` to the vault from environment variables. This ensures the vault has values even on first boot before the user reaches the dashboard.

The `create_agent` API handler reads the vault-resolved provider when no explicit provider is specified in the request, ensuring new agents inherit the platform default.

## Consequences

- Users can switch providers entirely through the dashboard without restarting the server. The vault update propagates to all new sessions.
- Existing agent configurations that explicitly set `provider` and `model` are unaffected by vault changes, preserving per-agent overrides.
- The auto-fallback in WASM modules means a partially-configured deployment (e.g., user set provider to OpenAI but didn't clear the Anthropic default) will still work, with warnings.
- Adding a new provider requires: (1) adding its key to `allowed_secret_keys()`, (2) adding its default model to the match arms, (3) adding its API endpoint and request format to the WASM modules, (4) adding its key to integration configs in `.ioa.toml` files.
