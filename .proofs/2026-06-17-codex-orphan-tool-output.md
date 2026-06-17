# Codex Orphan Tool Output Recovery

Date: 2026-06-17
Branch: `codex/codex-orphan-tool-output`

## Incident

Discord DM `status` failed in production with:

```text
OpenAI Codex API returned 400:
No tool call found for function call output with call_id call_FGEV2z33q2Wz9T03YAVRwx2E.
```

The failing request reached Codex with a Responses API `function_call_output` item whose `call_id` was not present as a `function_call` item in the same input transcript.

## Fix

`provider_caller` now builds Codex/OpenAI Responses input with a pairing guard:

- matched assistant `tool_use.id` + user `tool_result.tool_use_id` remains a Responses API `function_call` / `function_call_output` pair
- orphan `tool_result` blocks are downgraded into ordinary user context
- image content on downgraded tool results is preserved as user image input
- the provider logs how many orphan tool outputs were downgraded

ADR: `os-apps/paw-agent/adrs/035-codex-orphan-tool-output-guard.md`

## Red

```text
cargo test --manifest-path os-apps/paw-agent/wasm/provider_caller/Cargo.toml openai_responses_input --quiet
```

Failed before implementation because `build_openai_responses_input` did not exist.

## Green

```text
cargo test --manifest-path os-apps/paw-agent/wasm/provider_caller/Cargo.toml openai_responses_input --quiet
2 passed

cargo test --manifest-path os-apps/paw-agent/wasm/provider_caller/Cargo.toml --quiet
30 passed

cargo test -p temperpaw --test session_turn_architecture --locked
24 passed

bash os-apps/paw-agent/wasm/build.sh
All WASM modules built, including provider_caller.wasm

cargo test -p temperpaw --test paw_media_image_generation --locked
10 passed
```

## Deployment Status

Not deployed yet in this proof. The next steps are to publish a new image, deploy Railway, and verify production `/paw/version` plus a live DM reply.
