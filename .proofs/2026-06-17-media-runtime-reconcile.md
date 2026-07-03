# Media Runtime Reconcile Proof

Date: 2026-06-17
Branch: `codex/media-runtime-reconcile`

## Scope

Reconciled the production path for DM image generation. The fixed flow is Temper-native: `temper.image_generate` creates a `MediaGenerationRequest`, dispatches `Generate`, `provider_auth_gate` checks Codex subscription auth, `openai_codex_image_generate` renders through the Codex backend, and the result is written to PawFS before success is reported.

This also explains the "search corridor" confusion: `paw-foresight` is the corridor/search app and is separate from `paw-media`. It was relevant only because production bootstraps from Genesis refs, so all pinned refs had to be reconciled together.

## Red Tests

- `cargo test -p temperpaw --test paw_media_image_generation --locked`
  - Initial failures covered missing Docker/CI `paw-media` WASM packaging, broad callback Cedar permissions, and DM result rendering that allowed `Complete` without file/path/base64 output.
- `cargo test -p temperpaw --test corridor_engine_contract corridor_wasm_modules_are_packaged_for_core_startup --locked`
  - Initial failure showed `paw-foresight` had required core modules but no app-level build script.
- Added failing coverage for the production collision:
  - `image_generation_uses_app_scoped_entity_set_route` requires `MediaGenerationRequests` and rejects legacy `/tdata/MediaGenerations` callers.
  - `codex_image_renderer_streams_large_provider_responses` rejects the fixed-buffer `ctx.http_call` provider path and requires streaming response reads.

## Green Tests

- `cargo test -p temperpaw --test paw_media_image_generation --locked`
  - 8 passed.
- `cargo test -p temperpaw --test corridor_engine_contract --locked`
  - 13 passed.
- `cargo test -p temperpaw --test temperpaw_identity_contract os_app_wasm_build_scripts_preserve_temper_host_imports --locked`
  - 1 passed.
- `cargo test --manifest-path os-apps/paw-media/wasm/openai_codex_image_generate/Cargo.toml --quiet`
  - 9 passed.
- `cargo test --manifest-path os-apps/paw-agent/wasm/monty_repl/Cargo.toml --quiet`
  - 69 passed.
- `cargo test -p temperpaw --locked --quiet`
  - Passed all `temperpaw` test targets.

## Build Artifacts

- `bash os-apps/paw-media/wasm/build.sh`
  - Built `openai_codex_image_generate.wasm`.
  - SHA-256: `9c5520c2bacf3600380760b2e07c5794949a2779476cef964ac1fa197ff1bb3c`.
- `bash os-apps/paw-foresight/wasm/build.sh`
  - Built 13 corridor module-local `.wasm` artifacts for core startup.
- `cargo build --target wasm32-wasip1 --release` in `os-apps/paw-agent/wasm/monty_repl`
  - Rebuilt `monty_repl.wasm`.
  - SHA-256: `a09f51290b777da7839919507c836b2c71deb1b315000696808c53c8568a8049`.

## Local Runtime E2E

Fresh boot with a clean local database, `TEMPERPAW_WASM_STARTUP_POLICY=load-only`, and no Codex OAuth token:

```text
HOME=/tmp/temperpaw-media-request-route-home
PORT=54279
TURSO_URL=file:/tmp/temperpaw-media-request-route.db
TEMPER_API_KEY=media-request-route-key
PAW_TENANT=media_request_route
TEMPERPAW_WASM_STARTUP_POLICY=load-only
OTEL_ENABLED=false
./target/debug/temperpaw-server
```

Observed:

- `/readyz` returned HTTP 200.
- OData `$metadata` exposed `EntitySet Name="MediaGenerationRequests"` with `EntityType="TemperPaw.Media.MediaGenerationRequest"`.
- `GET /tdata/MediaGenerations` returned 404 on the clean install.
- Created a `MediaGenerationRequest` and dispatched `Temper.Generate?await_integration=true`.
- The request failed at the expected auth boundary with a missing Codex refresh token, proving the real media state machine and auth gate were attached instead of the old `Requested -> Succeeded` stub.

## Production Deploy Evidence

Production `TEMPERPAW_GENESIS_BOOTSTRAP_REFS` includes:

```text
temperpaw/paw-agent@dc6a81fd65ebef9514fd7e91a6b4fae92477c2b7
temperpaw/paw-media@7098fc6c3ba726880af3d2a9a005429dd90f7df0
temperpaw/paw-foresight@01ac826b9604ef1828eee146724a44953375ebfb
```

The deployed app still uses the GHCR image built from commit `12e8e8aff942b620ea300c412474fba4ce112d21`, but Genesis refs are the source of installed app specs/modules when `TEMPERPAW_GENESIS_BOOTSTRAP_REFS` is set.

Observed on `https://openpaw-production.up.railway.app`:

- `/readyz` returned `status=ready` and Discord `connected=true`.
- `/observe/wasm/modules/openai_codex_image_generate` returned `sha256_hash=9c5520c2bacf3600380760b2e07c5794949a2779476cef964ac1fa197ff1bb3c`, `cached=true`.
- `/observe/wasm/modules/monty_repl` returned `sha256_hash=a09f51290b777da7839919507c836b2c71deb1b315000696808c53c8568a8049`, `cached=true`.
- Production metadata includes `MediaGenerationRequests` for `TemperPaw.Media.MediaGenerationRequest`.
- Legacy `MediaGenerations` metadata still exists from the old root spec, but DM/runtime code no longer targets it.

## Production Cat E2E

Created workspace `ws-production-media-smoke`, then generated:

```text
prompt = "a tiny orange cat on a blue mat"
workspace_id = "ws-production-media-smoke"
output_path = "/images/cat-smoke-final.png"
```

Result entity:

```text
entity_id = en-019ed6f8-39e5-7873-bb9b-0d9bedcb934c
entity_type = MediaGenerationRequest
status = Complete
result_file_id = fl-019ed6f8-ad6a-74c2-9a87-5769eac4e5fb
result_path = /images/cat-smoke-final.png
mime_type = image/png
provider_response_id = resp_046e0905c8015e6d016a32efb875388196bc0102e3aaa5ff04
result_image_base64 length = 2542704
```

PawFS readback:

```text
GET /tdata/Files('fl-019ed6f8-ad6a-74c2-9a87-5769eac4e5fb')
entity_type = File
status = Ready
workspace_id = ws-production-media-smoke
path = /images/cat-smoke-final.png
mime_type = image/png
size_bytes = 1907026
```

Downloaded `/tdata/Files('fl-019ed6f8-ad6a-74c2-9a87-5769eac4e5fb')/$value` and compared it to the generated image bytes:

```text
/tmp/cat-smoke-final.png: PNG image data, 1402 x 1122, 8-bit/color RGB, non-interlaced
10ac12218b4f580170ffb4490a7effc89d40f0be59eb8ed64e0d5c68537330bd  /tmp/cat-smoke-final.png
10ac12218b4f580170ffb4490a7effc89d40f0be59eb8ed64e0d5c68537330bd  /tmp/cat-smoke-pawfs.png
```

This proves production rendered an actual Codex image, stored it in PawFS, and returned retrievable image bytes.

## Remaining Caveat

The clean long-term path is still a pinned Genesis `paw-agent` ref containing the updated `monty_repl.wasm`. Genesis Git currently fails or stalls on the large `paw-agent` pack, including a reduced hotfix branch containing only Monty. Until that is fixed, production requires a post-redeploy `monty_repl` hot-upload and hash verification as captured in `os-apps/paw-agent/adrs/033-temporary-media-route-hot-upload.md`.
