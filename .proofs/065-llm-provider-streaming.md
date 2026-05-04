# Proof 065: LLM Provider Streaming Through Temper

Date: 2026-05-04

## Scope

- Routed `provider_caller` LLM calls through ADR-0057 streaming HTTP instead of buffered `ctx.http_call`.
- Covered OpenAI/Codex Responses, Anthropic Messages, and OpenRouter chat completions.
- Added live `llm_delta` progress events and throttled `ProgressMade` dispatches while leaving final `ProviderResponseArtifact` persistence unchanged.
- Updated Temper `AuthorizedWasmHost` to authorize/delegate streaming begin/read/write/close/head operations.
- Updated Temper streaming host to consume `X-Temper-Span-*` hint headers instead of forwarding them upstream.

## Verification

```text
cargo test --manifest-path os-apps/paw-agent/wasm/provider_caller/Cargo.toml -- --nocapture
=> 22 passed

cargo build --manifest-path os-apps/paw-agent/wasm/provider_caller/Cargo.toml --target wasm32-unknown-unknown
=> passed

cargo build --manifest-path os-apps/paw-agent/wasm/provider_caller/Cargo.toml --target wasm32-unknown-unknown --release
=> passed

cargo test -p temper-wasm http_stream -- --nocapture
=> 14 passed in filtered streaming/auth run

cargo test -p temper-wasm --test http_stream_outbound -- --nocapture
=> 3 passed

cargo build -p temperpaw
=> passed
```

## Long-Stream E2E

Ran a throwaway proof runner from `/tmp/temperpaw-stream-proof` that:

- started a local OpenAI-compatible SSE endpoint;
- streamed deltas for 70 seconds;
- invoked the real compiled `provider_caller.wasm` through Temper `WasmEngine`;
- wrapped the host with `AuthorizedWasmHost`;
- intercepted only local Temper API calls for `Heartbeat` / `ProgressMade`;
- verified final `ProviderResponseReady` artifact content.

Result:

```text
ok elapsed_secs=70 llm_delta_events=8 progress_made_dispatches=4 final_text_chars=54 response_bytes=610
```

This confirms the provider call completed past the 60s buffered host-call cap, emitted live deltas, dispatched `ProgressMade`, and produced the final provider response artifact.
