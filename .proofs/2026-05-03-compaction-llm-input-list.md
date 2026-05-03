# Proof Report: 2026-05-03 — Fix compaction LLM input format

## Date
2026-05-03

## Branch / Commit
- Branch: `claude/fix-compaction-llm-input-nNMv6`
- Crate: `os-apps/paw-agent/wasm/context_compactor`

## What Was Done
The compaction LLM call was failing with:

    Compaction LLM call failed (HTTP 400): {"detail":"Input must be a list"}

Root cause: in `call_compaction_llm` (`os-apps/paw-agent/wasm/context_compactor/src/lib.rs`), the body for the `openai` and `openai_codex` providers serialized `input` as a string. The OpenAI Responses API — and especially the Codex backend at `https://chatgpt.com/backend-api/codex/responses` — requires `input` to be a list of input items, so the API rejected the request before any model work happened.

Changes:
1. Extracted body construction into a pure helper `build_compaction_request_body(provider, model, system_prompt, conversation_text)`. For `openai` / `openai_codex` the body now sends `input: [{"role":"user","content":...}]` (matching the format used by `provider_caller`). Anthropic and OpenRouter shapes are unchanged.
2. Extracted response parsing into `parse_compaction_response_text(provider, body)` plus a new `collect_codex_sse_output` helper. Previously, the `openai_codex` provider fell through to the Anthropic JSON branch and `serde_json::from_str` would fail on the SSE response (`accept: text/event-stream` is set in `build_openai_headers`). The Codex path now collects `response.output_text.delta` and `response.output_item.done` events from the SSE stream and reuses the same Responses-API extractor.
3. Collapsed the per-provider header construction into `call_compaction_llm` and added a small `anthropic_compaction_headers` helper for symmetry.
4. Added 8 new unit tests covering body shape (3 providers + Anthropic) and response parsing (5 paths including Codex SSE delta-only and `response.completed`-overrides-deltas).

## Verification Flow
1. Wrote regression test `openai_compaction_body_sends_input_as_list_not_string` that asserts `body["input"].as_array()` is `Some` for both `openai` and `openai_codex` — the prior code stored a string here, so this fails against pre-fix behavior.
2. Refactored the implementation to use the new pure helpers so the test passes (red → green).
3. Added Codex SSE parsing tests to cover the latent issue that would surface as soon as the input-format bug was fixed.
4. `cargo test --lib` (host target).
5. `cargo build --target wasm32-unknown-unknown --release` to confirm the actual deployable artifact still compiles.
6. `cargo fmt -- --check` to confirm formatting.

## Verification Results
| Step | Expected | Actual | Status |
|------|----------|--------|--------|
| `cargo test --lib` | 11 passed, 0 failed | 11 passed, 0 failed | OK |
| `cargo build --target wasm32-unknown-unknown --release` | Builds clean | Finished `release` profile target(s) | OK |
| `cargo fmt -- --check` | No diff | No diff | OK |
| OpenAI / Codex body shape | `input` is array of `{role,content}` items | Confirmed by `openai_compaction_body_sends_input_as_list_not_string` | OK |
| Codex SSE response parsing | Concatenates `response.output_text.delta` events; `response.completed.output` wins when present | Confirmed by two SSE parsing tests | OK |

## What Worked
- Single-source body construction makes the provider-specific shapes auditable and testable.
- Reusing the Responses-API output-item extractor for both raw JSON and SSE-derived synthetic JSON keeps the parser logic deduplicated.

## What Didn't Work
- None observed in unit tests. End-to-end verification against a live Codex endpoint requires real credentials and is not run here.

## Limitations
- No live LLM call was made — verification is via unit tests plus a wasm release build. A live end-to-end run against `chatgpt.com/backend-api/codex/responses` would require the user's OpenAI Codex token and is outside the scope of this fix.
- The Codex SSE parser ignores reasoning summary events; only output text is extracted. That matches the existing behavior in `provider_caller` and is sufficient for compaction summaries.

## What Still Doesn't Work
- None known. If a future Codex schema change introduces a new event type carrying the user-visible summary, the parser will need to learn that event name.

## Artifacts
- `os-apps/paw-agent/wasm/context_compactor/src/lib.rs` (refactor + new helpers + new tests)
- Wasm release artifact: `os-apps/paw-agent/wasm/context_compactor/target/wasm32-unknown-unknown/release/context_compactor.wasm`

## Architecture Diagram
```text
context_compactor::run
        |
        v
call_compaction_llm(provider, api_key, model, conversation_text)
        |
        +-- build_compaction_request_body(provider, ...)
        |       |
        |       +-- openai / openai_codex -> { instructions, input: [ {role:user, content} ] }
        |       +-- openrouter            -> { messages: [system, user] }
        |       +-- anthropic (default)   -> { system, messages: [user] }
        |
        +-- headers: build_openai_headers | openrouter bearer | anthropic_compaction_headers
        |
        +-- ctx.http_call POST url, headers, body_str
        |
        +-- parse_compaction_response_text(provider, resp.body)
                |
                +-- openai_codex          -> collect_codex_sse_output (SSE) -> parse_openai_responses_text
                +-- openai                -> JSON                            -> parse_openai_responses_text
                +-- openrouter            -> JSON.choices[0].message.content
                +-- anthropic (default)   -> JSON.content[type=text].text
```
