# 054 - LLM Observability Content Fix

Date: 2026-04-22

## Problem

Datadog LLM Observability showed `input No content` and `output No content` for `openpaw` LLM spans. The long spans matched the `provider_caller` WASM integration, but that integration only returned provider response file metadata. The `_gen_ai_*` message payloads were created later by `provider_response_applier`, which is not the LLM span Datadog records.

## Fix

- OpenPaw `llm_caller` now includes `_gen_ai_system_instructions`, `_gen_ai_input_messages`, `_gen_ai_output_messages`, `_gen_ai_provider`, `_gen_ai_finish_reason`, `input_tokens`, and `output_tokens` in the `ProviderResponseReady` callback params emitted by `provider_caller`.
- Local Temper strips private observability-only params (`_gen_ai_*`, `_dd_llmobs_tool_spans`) before dispatching callback actions so entity state is not polluted by telemetry payloads.
- Rebuilt the local `provider_caller` WASM artifact used by the OS-app loader.

## Red/Green Evidence

1. Added the OpenPaw regression test first:
   `provider_response_ready_params_include_llm_observability_content`
   It failed before implementation because `build_provider_response_ready_params` did not exist.

2. Implemented the OpenPaw callback payload helper and reran:

   ```text
   cargo test --lib
   result: 31 passed; 0 failed
   ```

3. Added the Temper regression test first:
   `strips_private_llm_observability_params_before_callback_dispatch`
   It failed before implementation because `strip_private_observability_params` did not exist.

4. Implemented callback sanitization and reran:

   ```text
   cargo test -p temper-server strips_private_llm_observability_params_before_callback_dispatch
   result: passed
   ```

## Build Evidence

```text
cargo build --target wasm32-unknown-unknown --release
cwd: os-apps/paw-agent/wasm/provider_caller
result: finished release build
rebuilt hash: a6037e3db82cf498437cd3665e6317ee7ad61c21880ac2e69efe498b5c63d071
```

```text
cargo test -p temperpaw --no-run
result: finished test profile
```

```text
cargo test -p temperpaw
result: 36 unit tests passed; 4 integration tests passed
```

## Runtime Evidence

Started a temporary local server:

```text
HOME=/tmp/openpaw-llmobs-proof \
RUSTUP_HOME=/Users/seshendranalla/.rustup \
CARGO_HOME=/Users/seshendranalla/.cargo \
OTEL_ENABLED=false \
PORT=45678 \
TURSO_URL=file:/tmp/openpaw-llmobs-proof/paw.db \
TEMPERPAW_WASM_STARTUP_POLICY=load-only \
RUST_LOG=info,temperpaw=debug \
cargo run -p temperpaw --bin temperpaw-server
```

Health check:

```text
curl -fsS -i http://127.0.0.1:45678/healthz
HTTP/1.1 200 OK
```

Authenticated OData probe:

```text
curl -fsS -H "Authorization: Bearer $API_KEY" \
  "http://127.0.0.1:45678/tdata/Agents?$top=1"

result: returned an Active Agent
```

WASM registry probe:

```text
curl -fsS -H "Authorization: Bearer $API_KEY" \
  "http://127.0.0.1:45678/observe/wasm/modules"

result: provider_caller registered with sha256_hash a6037e3db82cf498437cd3665e6317ee7ad61c21880ac2e69efe498b5c63d071
```

## Limits

I did not send a live external LLM request to Datadog from this local proof run. OTEL was disabled for the boot probe to avoid exporting local test telemetry, and a real provider call would consume configured provider credentials. The behavior that fixes the blank Datadog content is covered by the WASM payload regression test and the Temper callback sanitization regression test.
