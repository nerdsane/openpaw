# 055 - LLM Observability Model Metadata Fix

Date: 2026-04-23

## Problem

Datadog LLM Observability still showed a trace with `input No content` and `output No content` for trace `029ffe78db0542be8ffd5ef358df9fcf`. The trace was rooted at `Session.ProviderResponseReady.integrations` and included `wasm:provider_response_applier`, not the actual provider call span. In the LLM caller trace, model/provider metadata was also missing or normalized incorrectly.

## Fix

- `provider_caller` now reports `_gen_ai_model` with `_gen_ai_provider` and content payloads on `ProviderResponseReady`.
- The direct legacy `call_llm` integration is marked `llm = true`.
- `provider_response_applier` no longer emits private `_gen_ai_*` content params, so it does not appear as the LLM content span.
- Temper only records `gen_ai.*` span attributes for integrations declared as LLM integrations.
- Temper prefers callback-reported model/provider for observability, normalizes `openai_codex` to `openai`, and submits Datadog LLMObs API spans using the actual module span name plus model metadata.

## Red/Green Evidence

Added failing regression coverage before implementation:

```text
cargo test --lib provider_response_applier_base_params_do_not_emit_llm_observability_content
cwd: os-apps/paw-agent/wasm/llm_caller
red result: failed before build_provider_response_applier_base_params existed
green result: passed
```

```text
cargo test -p temper-observe llm_span_payload_uses_span_name_and_supported_model_metadata
red result: failed before span_name/build_llm_span_payload existed
green result: passed
```

```text
cargo test -p temper-server gen_ai_span_attrs_are_recorded_only_for_llm_integrations
cargo test -p temper-server llm_model_for_observability_prefers_callback_model
red result: failed before the helper behavior existed
green result: passed
```

## Verification Results

| Step | Expected | Actual | Status |
|------|----------|--------|--------|
| OpenPaw LLM caller unit tests | LLM callback params include real model and applier omits private content params | `cargo test --lib`: 32 passed | Pass |
| Temper LLMObs payload tests | Direct Datadog payload uses `wasm:provider_caller`, model name, and normalized provider | `cargo test -p temper-observe`: 49 passed | Pass |
| Temper dispatch regression tests | GenAI attrs are LLM-only and callback model wins | `cargo test -p temper-server state::dispatch::wasm::tests`: 3 passed | Pass |
| Temper compile check | Workspace compiles | `cargo check --workspace`: finished | Pass |
| OpenPaw compile check | TemperPaw compiles against local Temper patch | `cargo check -p temperpaw`: finished | Pass |
| OpenPaw format gate | Root workspace formatting clean | `cargo fmt --all -- --check`: passed | Pass |
| Temper lock update | OpenPaw image build resolves the merged Temper fix | `Cargo.lock` Temper packages advanced to `40122623eaea897ca2e53266dc6ce93d62c7321a` | Pass |
| Local server boot | Server starts with temp DB and health endpoint responds | `/healthz`: `HTTP/1.1 200 OK` | Pass |

## Runtime Evidence

Started a temporary local server with OTEL disabled:

```text
TEMPER_APP_SOURCES= \
GIT_TERMINAL_PROMPT=0 \
HOME=/tmp/openpaw-llmobs-metadata-proof \
RUSTUP_HOME=/Users/seshendranalla/.rustup \
CARGO_HOME=/Users/seshendranalla/.cargo \
OTEL_ENABLED=false \
PORT=45679 \
TURSO_URL=file:/tmp/openpaw-llmobs-metadata-proof/paw.db \
TEMPERPAW_WASM_STARTUP_POLICY=load-only \
RUST_LOG=info,temperpaw=debug \
cargo run -p temperpaw --bin temperpaw-server
```

Health check:

```text
curl -fsS -i http://127.0.0.1:45679/healthz
HTTP/1.1 200 OK
```

## Limits

The original Datadog trace will continue to show old captured data; LLMObs traces are not retroactively rewritten. A live post-deploy trace is still required to prove the production Datadog UI reflects the new provider-call span shape.

The first local boot attempt tried to clone the private git app source from `.env` and prompted for GitHub credentials. The successful proof run set `TEMPER_APP_SOURCES=` explicitly, which avoids that unrelated startup source.

## Architecture Diagram

```text
Session.CallProvider
  -> wasm:provider_caller [llm span, content + provider + model]
  -> ProviderResponseReady
  -> wasm:provider_response_applier [workflow only, no private LLM content]
  -> ProcessToolCalls / CheckSteering / RecordResult
```
