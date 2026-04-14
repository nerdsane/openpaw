# Proof Report: 039 — LLM Provider Configuration Cleanup

## Date
2026-04-14

## Branch / Commit
main (pre-commit, based on 7f520a49)

## What Was Done

Fixed critical runtime error: `provider=anthropic api key is unresolved secret template` affecting users with non-Anthropic LLM providers. The root cause was 15+ locations hardcoding `"anthropic"` as the default provider.

Changes:
1. **Rebuilt `llm_caller.wasm`** with auto-fallback: detects unresolved keys, switches provider, remaps model from vault
2. **Rebuilt `context_compactor.wasm`** with same provider-aware fallback and multi-provider API endpoints
3. **Fixed `create_agent()` in `setup_api.rs`** to read provider/model from vault
4. **Seeded `llm_model` to vault in `startup.rs`** derived from `llm_provider` at boot
5. **Updated `session.ioa.toml`** integration configs with all provider keys + vault-resolved defaults
6. **Hid Projects from sidebar** (non-functional section)
7. **Wrote ADR-0030** — LLM Provider Configuration Architecture

## Verification Flow

### Build Verification
| Step | Expected | Actual | Status |
|------|----------|--------|--------|
| `cargo check -p openpaw` | Compiles clean | `Finished dev profile in 1.87s` | PASS |
| `cargo check -p openpaw-cli` | Compiles clean | `Finished dev profile in 0.13s` | PASS |
| llm_caller WASM build (`wasm32-wasip1`) | Build succeeds | `Finished release in 2.07s` | PASS |
| context_compactor WASM build (`wasm32-wasip1`) | Build succeeds | `Finished release in 2.34s` | PASS |
| `npm run check` (dashboard) | 0 errors | `422 FILES 0 ERRORS 0 WARNINGS` | PASS |
| `npm run build` (dashboard) | Production build succeeds | `Wrote site to "build"` | PASS |

### Runtime Verification (new server on port 3467)
| Step | Expected | Actual | Status |
|------|----------|--------|--------|
| Server boots clean | No panics | All phases complete, specs bootstrapped | PASS |
| WASM binary loaded | New hash (not old 615736-byte binary) | Hash `af63f31f`, size 645475 (wasip1 build) | PASS |
| `GET /paw/version` | Returns version JSON | `{"version":"dev","sha":"unknown"}` | PASS |
| `GET /paw/setup/status` | Returns correct `llm_provider` | `"llm_provider":"openai"` (vault-resolved) | PASS |
| Auth middleware | 401 without auth | `HTTP/1.1 401 Unauthorized` | PASS |

### Paw Chat End-to-End (critical test)
| Step | Expected | Actual | Status |
|------|----------|--------|--------|
| Create Session | entity_id returned | `ss-019d8e50-c2fc-7800-b316-e313ecbd87be` | PASS |
| Configure with Paw agent | Transitions to Created | Status: Created | PASS |
| LLM call succeeds | No `unresolved secret template` error | Session reached Completed state | PASS |
| Response received | Agent produces a reply | Result: `"Hello!"` | PASS |
| Provider fallback logged | WASM warns about fallback | Server logs show fallback from anthropic to openai | PASS |
| Model remapped | Uses vault's `gpt-5.4` not hardcoded `claude-sonnet-4-6` | Confirmed (no "model not supported" error) | PASS |

### Issues Found During Testing

1. **Stale `wasm32-unknown-unknown` binaries**: The Temper platform checks `wasm32-unknown-unknown` before `wasm32-wasip1`. Old binaries in that path were loaded instead of rebuilt ones. Fix: deleted stale binaries.

2. **Model not remapped on provider fallback**: When WASM auto-fallback changed provider from anthropic to openai, the model stayed as `claude-sonnet-4-6`. OpenAI rejected it. Fix: added model remapping using vault's `llm_model` secret.

3. **`deliver_reply` 401 error**: The `agent_reply` WASM module gets HTTP 401 when calling back into the Temper API. This is a pre-existing issue unrelated to LLM provider changes — occurs because the WASM HTTP call uses an internal URL without proper auth headers. Session still completes successfully; only reply delivery to channel sessions fails.

## What Worked
- WASM auto-fallback correctly detects unresolved Anthropic key and switches to OpenAI
- Model remapping reads vault-configured `gpt-5.4` instead of hardcoded defaults
- Vault correctly stores and serves `llm_provider: "openai"` and `llm_model: "gpt-5.4"`
- Full session lifecycle: Created -> Provisioning -> Thinking -> Completed with LLM response
- Dashboard builds cleanly with Projects section removed

## What Didn't Work
- First two WASM rebuild attempts loaded stale binaries (platform checked wrong target first)
- First model remap used hardcoded `o3-mini` instead of vault value

## Limitations
- `deliver_reply` 401 is pre-existing and not addressed in this change
- Existing Paw agent entity still has `provider: "anthropic"` / `model: "claude-sonnet-4-6"` — relies on WASM fallback. New agents created via API will get vault-resolved values.

## What Still Doesn't Work
- `deliver_reply` integration returns HTTP 401 (pre-existing, unrelated to this fix)
- Entity spec `.ioa.toml` initial values still have hardcoded defaults (overridden at Configure time)

## Artifacts

### Files Modified
| File | Change |
|------|--------|
| `os-apps/paw-agent/wasm/llm_caller/src/lib.rs` | Provider fallback with model remapping from vault |
| `os-apps/paw-agent/wasm/llm_caller/target/wasm32-wasip1/release/llm_caller.wasm` | Rebuilt binary |
| `os-apps/paw-agent/wasm/context_compactor/src/lib.rs` | Provider-aware key resolution + multi-provider endpoints |
| `os-apps/paw-agent/wasm/context_compactor/target/wasm32-wasip1/release/context_compactor.wasm` | Rebuilt binary |
| `os-apps/paw-agent/specs/session.ioa.toml` | Added provider keys to compact_context config; added `default_llm_provider`/`default_llm_model` to call_llm config |
| `crates/openpaw/src/setup_api.rs` | `create_agent()` reads provider/model from vault |
| `crates/openpaw/src/startup.rs` | Seeds `llm_model` to vault |
| `dashboard/src/routes/+layout.svelte` | Removed Projects section |
| `docs/adrs/0030-llm-provider-configuration.md` | New ADR |

## Architecture Diagram
```text
                          Vault (single source of truth)
                    ┌──────────────────────────────────┐
                    │ llm_provider = "openai"           │
                    │ llm_model = "gpt-5.4"             │
                    │ openai_codex_token = "sk-..."      │
                    │ anthropic_api_key = (unset)        │
                    └──────┬───────────────┬────────────┘
                           │               │
              startup.rs seeds      integration config
              model from provider   resolves all keys
                           │               │
                    ┌──────▼──────┐  ┌─────▼──────────┐
                    │ create_agent │  │ WASM llm_caller │
                    │ reads vault  │  │ 1. try provider │
                    │ for defaults │  │ 2. key unresolved│
                    └─────────────┘  │ 3. fallback alt  │
                                     │ 4. remap model   │
                                     │ 5. call correct  │
                                     │    API endpoint   │
                                     └──────────────────┘
```
