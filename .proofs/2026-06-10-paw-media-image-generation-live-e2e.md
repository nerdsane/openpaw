# Paw Media Codex Image Generation Live E2E

Date: 2026-06-10
Branch: `codex/paw-media-image-gen`

## Scope

Verified the new Temper-native `paw-media` image generation flow using Codex subscription auth. No OpenAI API key was used. The local test daemon used an isolated HOME and tenant.

## Commands

- `cargo test -p temperpaw --test paw_media_image_generation`
- `cargo test -p temperpaw --test datadog_observability_contract wasm_sdk_dependencies_pin_same_temper_runtime_revision_as_server`
- `cargo test --manifest-path os-apps/paw-media/wasm/openai_codex_image_generate/Cargo.toml`
- `cargo test --manifest-path os-apps/paw-agent/wasm/provider_caller/Cargo.toml openai_codex_headers_include_chatgpt_account_and_sse_contract`
- `cargo test --manifest-path os-apps/paw-agent/wasm/tool-catalog/Cargo.toml`
- `cargo test --manifest-path os-apps/paw-agent/wasm/monty_repl/Cargo.toml`
- `cargo build --workspace`
- `bash os-apps/paw-agent/wasm/build.sh`
- `bash os-apps/paw-media/wasm/build.sh`
- Live daemon:
  - `PORT=54265 TEMPER_API_KEY=media-e2e PAW_TENANT=media_e2e TEMPERPAW_WASM_STARTUP_POLICY=warn ... target/debug/temperpaw-server`

## Live Setup

- Server readiness: `GET /readyz` returned `{"status":"ready",...}`.
- Seeded Codex subscription OAuth fields from local Codex auth into the isolated Temper vault without logging token values.
- `POST /paw/setup/openai-codex/ensure-fresh` returned `status = Ready`.
- `openai_codex_image_generate` module invocation count changed from `0` to `1`.

## Live Result

`MediaGeneration`:

- ID: `ad2a6e0a-7a14-45ea-914d-f7028361ba26`
- Workspace ID: `178ffb05-6d50-4ebb-8dfd-042d1653780f`
- Status: `Complete`
- Result path: `/generated/e2e/codex-image-20260610014259.png`
- MIME type: `image/png`
- Provider response ID: present
- Inline base64 length: `2008244`
- Error fields: `null`

PawFS `File`:

- File ID: `fl-019eaf33-521c-76c3-a9b1-f6e20388e0a2`
- Status: `Ready`
- Size: `1506181` bytes
- Has content: `true`
- Last version ID: `019eaf33-526a-75c1-8f09-7e0848dbda1f`
- `GET /tdata/Files('{file_id}')/$value` returned `1506181` bytes, `content_type = image/png`, and valid PNG magic.

State histories:

- `MediaGeneration`: `Created -> Authorizing -> Generating -> Storing -> Complete`
- Actions: `Created`, `Generate`, `RecordAuthReady`, `RecordStoring`, `RecordResult`
- `OpenAICodexAuth`: `Idle -> Refreshing -> Ready`
- Actions: `Created`, `EnsureFresh`, `LoginComplete`

## Notes

The Codex image stream can exceed the current buffered WASM HTTP response limit at default/medium quality because it includes partial and final image payloads. The provider now defaults to `quality = "low"` for the buffered path, still producing a durable PNG, and keeps parser fallbacks for raw buffered and partial-image event bodies.
