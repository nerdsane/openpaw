# Media Runtime Reconcile Proof

Date: 2026-06-17
Branch: `codex/media-runtime-reconcile`

## Scope

Reconciled the production packaging path for `paw-media` image generation and the core startup WASM surface that blocked a clean local boot. The fix keeps media generation Temper-native: `MediaGeneration.Generate` still advances through entity actions and WASM integrations.

## Red Tests

- `cargo test -p temperpaw --test paw_media_image_generation --locked`
  - Initial result: 3 failures.
  - Missing Docker/CI `paw-media` WASM build.
  - Broad `MediaGeneration` callback Cedar permit.
  - DM image result renderer accepted `Complete` with no file/path/base64 artifact.
- `cargo test -p temperpaw --test corridor_engine_contract corridor_wasm_modules_are_packaged_for_core_startup --locked`
  - Initial result: failed because `os-apps/paw-foresight/wasm/build.sh` did not exist.

## Build Artifacts

- `bash os-apps/paw-media/wasm/build.sh`
  - Built `openai_codex_image_generate`.
  - Published `os-apps/paw-media/wasm/openai_codex_image_generate/openai_codex_image_generate.wasm` (336KB).
- `bash os-apps/paw-foresight/wasm/build.sh`
  - Built and published 13 corridor module-local `.wasm` artifacts:
    `seed_world`, `sample_endpoints`, `decompose_endpoint`, `spawn_repairers`, `spawn_adversaries`, `aggregate_costs`, `evidence_ingest`, `register_forecasts`, `render_artifacts`, `consistency_gate`, `grade_hindcast`, `animate_dwellers`, `adjudicate_nodes`.

## Green Tests

- `cargo test -p temperpaw --test paw_media_image_generation --locked`
  - 6 passed.
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
- `git diff --check`
  - Passed.

## Local Runtime E2E

First fresh boot with `TEMPERPAW_WASM_STARTUP_POLICY=build-if-missing` proved the unrelated blocker: startup failed before readiness because `paw-foresight` was a core app with required modules but no app-level build script.

After adding `paw-foresight/wasm/build.sh` and publishing the artifacts, a fresh `load-only` boot succeeded:

```text
HOME=/tmp/temperpaw-media-runtime-reconcile-loadonly-home
PORT=54267
TURSO_URL=file:/tmp/temperpaw-media-runtime-reconcile-loadonly.db
TEMPER_API_KEY=media-runtime-e2e-key
PAW_TENANT=media_runtime_loadonly
TEMPERPAW_WASM_STARTUP_POLICY=load-only
OTEL_ENABLED=false
./target/debug/temperpaw-server
```

Observed:

- `/readyz` returned HTTP 200:
  `{"status":"ready","healthz":"/healthz","discord":{"status":"disconnected","configured":false,"connected":false}}`
- OData `$metadata` included the live `MediaGeneration` surface:
  `EntityType Name="MediaGeneration"`, `Generate`, `RecordAuthReady`, `RecordStoring`, `RecordResult`, and `EntitySet Name="MediaGenerations"`.
- `GET /tdata/MediaGenerations?$top=1` returned HTTP 200 with an empty collection.
- Created a `MediaGeneration` and dispatched `Temper.Generate?await_integration=true` without Codex credentials in the isolated HOME.
- State history was `Created -> Authorizing -> Failed` with a missing Codex refresh token error from `provider_auth_gate`.

That proves this build serves the real media state machine and auth gate, not the stale `Requested -> Succeeded` stub that could complete without an artifact.

## Notes

This proof did not generate a real cat image because the isolated local HOME intentionally had no Codex OAuth token. Production still needs the GHCR image build and Railway redeploy to make the fix effective in DMs.
