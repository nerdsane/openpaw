# Discord DM Catch-Up Recovery

## Context

TemperPaw production was connected to Discord, but Discord DMs sent while the
gateway was unhealthy were not replayed after reconnect. The persisted Channel
entity still had the known DM user/thread id and an older cursor, while
Discord's bot DM listing returned no channels.

## Changes

- Reuse persisted Channel `thread_id` or `author_id` to seed the Discord DM
  cache by reopening the DM with Discord's create-DM endpoint.
- Merge seeded DM sources with Discord's REST-listed DMs during reconnect
  catch-up.
- Bump TemperPaw's Temper dependency rev to
  `a52f2dc4fe0d377a2a7a62f17930f9419672a2ad`, which includes the platform
  bundled-WASM readiness fix from Temper PR #314.

## Production Evidence Before Deploy

- `GET /readyz` returned `status=ready` and Discord `status=connected`.
- Active production `route_message` module hash was
  `ed10c8973bbc6e74d1bed6918da0b3e3b2003ac1e99fcecc64c40b3ff5458e86`.
- Channel `en-019eba01-164d-7d60-ab4d-5e710a5e39d6` was `Connected` with
  `thread_id=1018228973869727785`, `author_id=1018228973869727785`,
  `message_count=42`, and `last_discord_message_id=1516601740126846996`.
- `POST /users/@me/channels` for user `1018228973869727785` reopened Discord
  DM channel `1494059279202779136`.
- `GET /users/@me/channels` for the bot returned an empty list, so reconnect
  catch-up had no REST-listed DMs to scan.
- The reopened DM contained user messages newer than the persisted cursor,
  including `1516649307166740571` at `2026-06-17T03:42:52.810Z`.

## Verification

- Red: `cargo test -p paw-transport known_dm_thread_from_channel_entity_uses_persisted_author --quiet`
  failed before the helper existed.
- Red: `cargo test -p paw-transport catch_up_dm_sources_include_seeded_dm_channels_when_rest_listing_is_empty --quiet`
  failed before seeded DM sources were merged.
- Green: `cargo test -p paw-transport --quiet` passed 33 tests.
- Green: `bash os-apps/paw-channels/wasm/build.sh` rebuilt all channel WASM
  modules.
- Green: `bash scripts/verify_route_message_wasm.sh` passed with packaged
  `route_message` hash
  `0d3910b1af05ac840978eba9d5491a81ffa2afa9b07e88d6aac5a76866a68c26`.
- Green: CI-equivalent os-app WASM build list passed locally after unifying
  guest `temper-wasm-sdk` pins to the same Temper rev as the server.
- Green: `cargo test -p temperpaw --test route_message_wasm_packaging --quiet`
  passed 2 tests.
- Green: `cargo test --locked -p temperpaw --quiet` passed.
- Green: `cargo test --locked -p paw-codex-worker --quiet` passed 89 tests.
- Green: `cargo test --manifest-path os-apps/paw-patrol/wasm/review_gate_lifecycle/Cargo.toml --quiet`
  passed 3 tests.
- Green: `cargo fmt --all --check` passed.
- Green: `git diff --check` passed.
