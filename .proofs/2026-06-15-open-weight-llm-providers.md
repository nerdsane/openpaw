# Open-Weight LLM Provider Support Proof

Date: 2026-06-16
Branch: `codex/open-weight-llm-providers-20260616`

## Scope

Add OpenRouter plus OpenAI-compatible hosted/local providers for TemperPaw Sessions without adding a Rust orchestration loop. Runtime LLM calls remain Session-state-driven WASM integrations.

## Verified Before Merge Attempt

Previously observed on deployed TemperPaw:

- OpenRouter completed with model `qwen/qwen3.5-9b`; result marker `OPENROUTER_PROVIDER_OK`.
- Hugging Face requested GGUF model `yuxinlu1/gemma-4-12B-coder-fable5-composer2.5-v1-GGUF` failed with provider `model_not_supported`, proving the requested GGUF repo is not directly runnable through Hugging Face’s hosted router.
- Hugging Face completed with model `openai/gpt-oss-120b:cerebras`; result marker `HF_PROVIDER_OK`.
- Fireworks stale model `accounts/fireworks/models/llama-v3p1-8b-instruct` failed with 404.
- Fireworks completed with model `accounts/fireworks/models/qwen3p7-plus`; result marker `FIREWORKS_PROVIDER_OK`.
- OpenRouter Fusion with `openrouter/fusion` failed upstream with HTTP 500 in both TemperPaw and a direct OpenRouter call.

## Clean Branch Verification

All commands below were run from `/Users/seshendranalla/Development/temperpaw-worktrees/open-weight-llm-providers-20260616`.

- `cargo fmt --all` passed.
- `cargo test --manifest-path os-apps/paw-agent/wasm/openai-chat-wire/Cargo.toml` passed: 5 tests.
- `cargo test --manifest-path os-apps/paw-agent/wasm/provider_caller/Cargo.toml` passed: 28 tests. Existing dead-code warnings remain for the old OpenRouter-specific helpers that are now superseded by the shared adapter path.
- `cargo test --manifest-path os-apps/paw-agent/wasm/context_compactor/Cargo.toml` passed: 15 tests.
- `cargo test -p temperpaw --tests -- --nocapture` passed: all TemperPaw integration suites, including the new provider secret/schema, provider options, and shared OpenAI-compatible adapter architecture tests.
- `./build.sh` in `os-apps/paw-agent/wasm` completed successfully.
- `./build.sh` in `os-apps/paw-channels/wasm` completed successfully.
- `sh build.sh` in `os-apps/paw-managed-agents/wasm` completed successfully.
- `cargo build --manifest-path os-apps/paw-wiki/wasm/build_session_message/Cargo.toml --target wasm32-unknown-unknown --release` completed successfully.
- `cargo build --manifest-path os-apps/paw-wiki/wasm/finalize_spawned_session/Cargo.toml --target wasm32-unknown-unknown --release` completed successfully.
- `cargo build` completed successfully.

## Fresh Boot Check

Fresh isolated boot command:

```sh
env HOME="$(mktemp -d)" PORT=32138 OTEL_ENABLED=false RUST_LOG=warn \
  TEMPERPAW_WASM_STARTUP_POLICY=build \
  TEMPERPAW_ORPHANED_SESSION_RECOVERY=false \
  TEMPERPAW_QUERY_PROJECTION_BACKFILL_ON_STARTUP=false \
  target/debug/temperpaw-server
```

Observed result:

- `healthz` reached `200`.
- `readyz` remained `503`.
- The process exited with startup reconcile errors for missing app-required WASM artifacts in existing startup apps outside this provider change: `paw-fs`, `paw-foresight`, `paw-media`, `paw-ingest`, `paw-patrol`, `paw-research`, and `paw-skills`.
- The touched provider-path apps were built/tested directly as listed above. This branch did not attempt to repair the unrelated global startup app artifact gap.

## Pending Live Provider Proof

- Run one live Session per configured provider after merge/deploy and record final OData Session ids, statuses, counters, and logs.
- Verify Sakana Fugu once beta URL/key/model are available.
- Verify local Ollama from a network location reachable by deployed TemperPaw or from local TemperPaw.
- Re-run OpenRouter Fusion when upstream `openrouter/fusion` stops returning HTTP 500.

## Notes

Hosted providers are not free open-weight execution. No-pay proof requires local/self-hosted OpenAI-compatible inference such as Ollama or vLLM.
