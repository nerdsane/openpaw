# Codex Tool History As Context

Date: 2026-06-17
Branch: `codex/codex-tool-history-as-context`

## Incident

After ADR-035 deployed, Discord DM `status` still returned:

```text
OpenAI Codex API returned 400:
No tool call found for function call output with call_id call_FGEV2z33q2Wz9T03YAVRwx2E.
```

The first guard handled orphan `tool_result` blocks only. A matched historical
assistant `tool_use` plus user `tool_result` can still be rejected by Codex when
replayed as fresh Responses API protocol items.

## Fix

`provider_caller` now sends all historical TemperPaw tool history to Codex as
ordinary conversation context:

- no historical `function_call` items
- no historical `function_call_output` items
- tool call/result content remains visible to the model as text
- image content in tool results remains attached as user image input

ADR: `os-apps/paw-agent/adrs/036-codex-tool-history-as-context.md`

## Red

```text
cargo test --manifest-path os-apps/paw-agent/wasm/provider_caller/Cargo.toml openai_responses_input --quiet
```

Failed after adding
`openai_responses_input_downgrades_matched_tool_history_to_user_context`
because matched tool history still produced Responses API tool protocol items.

## Green

```text
cargo test --manifest-path os-apps/paw-agent/wasm/provider_caller/Cargo.toml openai_responses_input --quiet
2 passed

cargo test --manifest-path os-apps/paw-agent/wasm/provider_caller/Cargo.toml --quiet
30 passed

cargo test -p temperpaw --test session_turn_architecture --locked
24 passed

cargo test -p temperpaw --test paw_media_image_generation --locked
10 passed

bash os-apps/paw-agent/wasm/build.sh
All WASM modules built, including provider_caller.wasm
```

## Deployment Status

Not deployed yet in this proof. Next steps: merge, publish Docker, deploy
Railway, and verify production.
