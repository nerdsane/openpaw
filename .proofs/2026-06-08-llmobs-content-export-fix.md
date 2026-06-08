# LLMObs Content Export Fix Proof

Date: 2026-06-08
Branch: codex/llmobs-content-export-fix

## Issue

Datadog LLMObs provider spans rendered as `No content` even though TemperPaw
was setting GenAI metadata. Live spans showed short attributes, but content
fields and content events did not appear reliably.

## Root Cause

The platform guest-span host API did not declare the GenAI LLMObs content
fields as Datadog-visible static tracing fields, so the content attributes were
not exported in the shape Datadog LLMObs consumes. TemperPaw also ignored every
guest-span export result in `provider_caller`, which hid attribute/event export
failures from logs.

## Fix

- Temper commit `7f6c029d034200599b8d03688229ad4a316b3303` declares GenAI
  content fields on guest spans and host span-hint spans.
- TemperPaw pins all Temper and `temper-wasm-sdk` dependencies to that revision.
- `provider_caller` bounds legacy `gen_ai.completion` before it crosses the
  guest-span host boundary.
- `provider_caller` logs warn-level failures for content event, attribute, and
  span-end export operations.
- App ADR: `os-apps/paw-agent/adrs/032-provider-llmobs-export-boundary.md`.
- Platform ADR: `docs/adrs/0136-guest-span-llmobs-content-fields.md` in the
  Temper repository.

## Red-Green Evidence

- Red: expanded Temper `common_session_tool_and_llm_span_hints_are_datadog_visible_fields`
  to require GenAI content fields; it failed on missing `gen_ai.system`.
- Green: `cargo test -p temper-wasm common_session_tool_and_llm_span_hints_are_datadog_visible_fields`
  passed after declaring the content fields.
- Red: added TemperPaw provider test
  `llm_success_span_attrs_bound_legacy_completion`; initial compile failed
  before the completion cap constant/helper existed.
- Green: `cargo test --manifest-path os-apps/paw-agent/wasm/provider_caller/Cargo.toml llm_success_span_attrs_bound_legacy_completion`
  passed after adding bounded completion formatting.

## Local Verification

- Temper: `cargo test -p temper-wasm span_hint` passed.
- TemperPaw provider module:
  `cargo test --manifest-path os-apps/paw-agent/wasm/provider_caller/Cargo.toml`
  passed 28 tests.
- TemperPaw hot-path guard:
  `cargo test -p temperpaw --test paw_fs_hot_path` passed 12 tests.
- TemperPaw Datadog contract:
  `cargo test -p temperpaw --test datadog_observability_contract` passed 32 tests
  after all nested WASM lockfiles were pinned to the same Temper revision.
- Server build:
  `cargo build -p temperpaw --release --bin temperpaw-server` passed.
- WASM release build:
  `bash build.sh` from `os-apps/paw-agent/wasm` passed and rebuilt all modules,
  including `provider_caller`.
- Hygiene:
  `git diff --check` passed and `rg` found no remaining references to the old
  Temper revision `5ee4429f45d8f2bcf48f1269e377ef79b2c5544c`.

## Production Verification

Pending merge and Railway deployment. Production proof should include:

- `/paw/version` response showing the deployed TemperPaw commit.
- Railway deployment status for the app service.
- A new post-deploy `tool.llm_call` span in Datadog with
  `gen_ai.output.messages` and/or `gen_ai.completion`.
- A Datadog log query for `provider_caller: LLM span export failed` showing no
  new export failures after deploy.
