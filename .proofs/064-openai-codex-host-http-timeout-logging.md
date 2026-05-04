# Proof Report: 064 - OpenAI Codex Host HTTP Timeout Logging

## Date
2026-05-04

## Branch / Commit
- Branch: codex/llm-provider-streaming
- Base commit: 7a76b496

## What Was Done
- Updated `provider_caller` so Codex host HTTP failures are reported as host-boundary failures, not as generic OpenAI API failures.
- Added per-attempt warning text that includes `host_http_timeout_or_transport_error=true`.
- Added a regression test for the final exhausted retry error wording.

## Verification Flow
- Red: ran the new focused test before implementation and confirmed it failed because `format_openai_codex_exhausted_error` did not exist.
- Green: implemented the formatter and wired it into the Codex retry path.
- Built and tested the provider caller as both a native test crate and a WASM module.
- Built the TemperPaw server and performed a short local boot check.

## Verification Results
| Step | Expected | Actual | Status |
|------|----------|--------|--------|
| `cargo test --manifest-path os-apps/paw-agent/wasm/provider_caller/Cargo.toml openai_codex_host_http_failure_message_names_host_boundary -- --nocapture` before implementation | Failing test | Failed with missing formatter | Pass |
| `cargo test --manifest-path os-apps/paw-agent/wasm/provider_caller/Cargo.toml openai_codex_host_http_failure_message_names_host_boundary -- --nocapture` after implementation | Focused test passes | 1 passed | Pass |
| `cargo test --manifest-path os-apps/paw-agent/wasm/provider_caller/Cargo.toml` | Provider caller suite passes | 17 passed | Pass |
| `cargo build --manifest-path os-apps/paw-agent/wasm/provider_caller/Cargo.toml --target wasm32-unknown-unknown` | WASM module builds | Finished successfully | Pass |
| `cargo build -p temperpaw` | Server crate builds | Finished successfully | Pass |
| `cargo run -p temperpaw -- doctor` | Local config can be diagnosed | Data, vault key, database, and API key ok; Discord absent; Slack partial | Pass |
| `cargo run -p temperpaw -- run` | Server boots | Rebound from busy port 3467 to `http://localhost:63376/tdata`; printed `Paw is ready` | Pass |

## What Worked
- The final Codex retry-exhausted error now names the host HTTP boundary and says no provider HTTP response was returned.
- The attempt warning log is explicit enough to query in Datadog by `host_http_timeout_or_transport_error=true`.
- The local server booted cleanly after the change.

## What Didn't Work
- Discord and Slack transports did not start in the local boot because their tokens are not configured in the local vault.

## Limitations
- I did not intentionally trigger five live 60-second Codex host timeouts. That would spend several minutes and an external LLM call path just to verify wording already covered by the retry-path formatting test.

## What Still Doesn't Work
- The underlying 60-second `host_http_call` outer deadline still exists in deployed Temper. This change only makes the error diagnosis honest.

## Artifacts
- `os-apps/paw-agent/wasm/provider_caller/src/lib.rs`
- `.proofs/064-openai-codex-host-http-timeout-logging.md`

## Architecture Diagram
```text
provider_caller WASM
  -> ctx.http_call(POST chatgpt Codex SSE)
    -> Temper host_http_call boundary
      -> provider response returned: parse SSE
      -> host timeout/transport failure: log host-boundary warning and retry
```
