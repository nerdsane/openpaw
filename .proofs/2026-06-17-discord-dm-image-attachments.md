# Discord DM Image Attachments Proof

Date: 2026-06-17

## Scope

Fix the DM image path after `MediaGenerationRequest` succeeds:

- avoid broad `SessionEntry` recovery reads that can produce OData HTTP 413
- make `temper.image_generate(...)` resolve or create a PawFS workspace for DM sessions
- carry generated PawFS image files through `Session -> Channel -> Discord`
- upload reply images to Discord DMs as files

## Red Tests

Added failing source-contract tests before implementation:

- `cargo test -p temperpaw --test paw_media_image_generation --locked`
  - failed on missing `resolve_image_workspace_id`
  - failed on missing `reply_attachments_json` / Discord file upload path
- `cargo test -p temperpaw --test session_turn_architecture session_entry_readbacks_stay_within_bounded_query_budget --locked`
  - failed on `$top=10000` SessionEntry readback

## Green Verification

Commands run after implementation:

```text
cargo test -p temperpaw --test paw_media_image_generation --locked
10 passed

cargo test -p temperpaw --test session_turn_architecture --locked
24 passed

cargo test -p paw-transport --lib --locked
34 passed

cargo test --manifest-path os-apps/paw-agent/wasm/monty_repl/Cargo.toml --quiet
69 passed

cargo test --manifest-path os-apps/paw-agent/wasm/provider_response_applier/Cargo.toml --quiet
13 passed

cargo test --manifest-path os-apps/paw-agent/wasm/agent_reply/Cargo.toml --quiet
7 passed

cargo test --manifest-path os-apps/paw-agent/wasm/steering_checker/Cargo.toml --quiet
2 passed

cargo test --manifest-path os-apps/paw-agent/wasm/wasm-helpers/Cargo.toml --quiet
38 passed

cargo test --manifest-path os-apps/paw-channels/wasm/send_reply/Cargo.toml --quiet
2 passed
```

WASM packaging:

```text
bash os-apps/paw-agent/wasm/build.sh
All WASM modules built. monty_repl (wasip1): 6746KB

bash os-apps/paw-channels/wasm/build.sh
All Temper channel WASM modules built. send_reply: 189KB, route_message (WASI): 480KB
```

Local daemon smoke:

```text
PORT=3567 OTEL_ENABLED=false RUST_LOG=warn cargo run -p temperpaw --bin temperpaw-server
curl -fsS -i http://127.0.0.1:3567/healthz
HTTP/1.1 200 OK

curl -fsS -i http://127.0.0.1:3567/readyz
HTTP/1.1 503 Service Unavailable
```

The 503 on `/readyz` is expected in this local smoke because Discord and production credentials are not configured. It does not indicate a liveness failure.

## Production Boundary

This shell did not have `RAILWAY_*`, `GITHUB_TOKEN`, or `TEMPER_API_KEY` deploy credentials. I could not trigger the GHCR image build/Railway redeploy or send a live production Discord DM from this environment.

The clean deployment path is:

1. merge this commit to `main`
2. let `.github/workflows/docker.yml` publish `ghcr.io/nerdsane/temperpaw:edge` / `sha-*`
3. run `.github/workflows/railway-redeploy.yml` for that tag and expected commit SHA
4. verify production `/paw/version`, `/readyz`, and a real Discord DM image request

Until that deploy happens, the currently deployed Paw-Railway app will not have the new Discord attachment sender.
