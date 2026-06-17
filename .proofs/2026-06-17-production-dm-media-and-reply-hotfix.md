# Production DM Media and Reply Hotfix

Date: 2026-06-17

## Scope

Fix the deployed Discord DM path after Paw returned Codex 400s for normal replies and generated `MediaGenerationRequest` rows without an actual Discord image attachment.

Existing architecture ADR coverage:

- `os-apps/paw-media/adrs/001-codex-subscription-image-generation.md`
- `os-apps/paw-media/adrs/002-production-renderer-packaging-and-result-guards.md`
- `os-apps/paw-media/adrs/003-app-scoped-media-generation-route.md`
- `os-apps/paw-channels/adrs/005-discord-pawfs-image-attachments.md`
- `os-apps/paw-agent/adrs/034-dm-image-reply-attachments.md`
- `os-apps/paw-agent/adrs/036-codex-tool-history-as-context.md`

No new ADR was added for this patch because the changes are implementation bugfixes inside the accepted media/reply designs:

- Empty `model` action params now use the renderer default instead of being sent to Codex as `""`.
- `send_reply` now reads current `SendReply` action params from `trigger_params` before falling back to Channel fields, avoiding stale or missing `reply_attachments_json`.

## Red/Green Evidence

`openai_codex_image_generate`:

- Red: `empty_model_field_uses_provider_default` failed while `field_or_default` treated an empty string as the selected model.
- Green: `cargo test --manifest-path os-apps/paw-media/wasm/openai_codex_image_generate/Cargo.toml --quiet`
- Result: 10 tests passed.

`send_reply`:

- Red: `delivery_prefers_current_action_params_over_stale_channel_fields` failed with `old-thread` instead of `new-thread`.
- Green: `cargo test --manifest-path os-apps/paw-channels/wasm/send_reply/Cargo.toml --quiet`
- Result: 3 tests passed.

Repo-level regression checks:

- `cargo test -p temperpaw --test paw_media_image_generation --locked`: 10 tests passed.
- `cargo test -p temperpaw --test session_turn_architecture --locked`: 24 tests passed.

## Production Runtime

Production URL:

- `https://openpaw-production.up.railway.app`

Deployed Docker/application version after PR 412:

- `/paw/version`: `sha-d7ce7265`
- Main commit: `d7ce72659367c454b4e4a7c3d49d3f6ee6f86f12`
- Docker image: `ghcr.io/nerdsane/temperpaw:sha-d7ce726`
- Railway deployment: `70848167-b703-4886-a1a8-5427e7e75c1a`

Hot-loaded WASM modules in production:

- `provider_caller`: `9f46c7e59a450d6559cdd909573e706907a48184093c5b667d5f3dcf3085554d`
- `openai_codex_image_generate`: `e286a1ae58bd98c8f0358cb2d5181ac51256865247dca0c13c485eb2f81c6aca`
- `send_reply`: `6afb2c8150ccc9fcd92f57926478b19071115e2b76d56738498a1ddb9219a043`

## Media Generation Smoke

Created a production `MediaGenerationRequest` and dispatched `Temper.Generate`.

Observed result:

- `media_generation_id`: `en-019ed7c3-6286-7660-b0be-3f72263a7b83`
- `status`: `Complete`
- `result_file_id`: `fl-019ed7c3-fbb8-75b1-ba8e-53eda76eb156`
- `result_file_version_id`: `019ed7c3-ff05-7da0-80b1-655024a8aa54`
- `result_path`: `/generated/smoke/codex-cat-smoke-20260617184606.png`
- `mime_type`: `image/png`
- `provider_response_id`: `resp_0fa791b4f2af3422016a3323af73948190aad2e88c594501e9`
- `GET /tdata/Files('fl-019ed7c3-fbb8-75b1-ba8e-53eda76eb156')/$value`: 2,378,927 bytes
- Local proof image: `/tmp/temperpaw-proof/cat-smoke-20260617.png`
- Local sha256: `29d4f9a7dd5893129c1f66a2a620eeee36b2d23983a4db73767075e44342eafb`

Renderer log after the fix:

- `openai_codex_image_generate: calling Codex image_generation model=gpt-5.5`

## Discord Image Delivery Smoke

Active production Discord Channel:

- `channel_entity_id`: `en-019eba01-164d-7d60-ab4d-5e710a5e39d6`
- `thread_id`: `1018228973869727785`
- `agent_entity_id`: `aj-019d8cde-5bf6-7472-8ad1-2b2798c822b1`

Dispatched `Paw.Channel.SendReply?await_integration=true` with:

- `reply_attachments_json`: one `pawfs_file` attachment pointing at `fl-019ed7c3-fbb8-75b1-ba8e-53eda76eb156`

Observed production logs:

- `invoking WASM integration module` with `module=send_reply` and hash `6afb2c8150ccc9fcd92f57926478b19071115e2b76d56738498a1ddb9219a043`
- `GET /tdata/Files('fl-019ed7c3-fbb8-75b1-ba8e-53eda76eb156')/$value` returned 200
- `delivered discord reply attachments`, `attachment_count=1`
- `Channel.ReplyDelivered Connected -> Connected ... succeeded`

## Normal DM Reply Smoke

Injected a normal DM-shaped `ReceiveMessage` into the active Channel:

- `message_id`: `codex-proof-status-20260617185351`
- `author_id/thread_id`: `1018228973869727785`
- `content`: `status`

Observed production logs:

- `route_message: routed thread 1018228973869727785 to session ss-019ed7ca-7b19-7a70-8d2c-4f72f82eeb29`
- `provider_caller` invoked with hash `9f46c7e59a450d6559cdd909573e706907a48184093c5b667d5f3dcf3085554d`
- `agent_reply: dispatched reply for agent aj-019d8cde-5bf6-7472-8ad1-2b2798c822b1 to thread 1018228973869727785`
- `Channel.ReplyDelivered Connected -> Connected ... succeeded`
- `emit_ots_trajectory: emitted trajectory trj-ss-019ed7ca-7b19-7a70-8d2c-4f72f82eeb29 for session ss-019ed7ca-7b19-7a70-8d2c-4f72f82eeb29 (status=Completed)`

The old production symptoms were not present in this run:

- No `OpenAI Codex API returned 400`
- No `No tool call found for function call output`
- No `SessionEntry list failed (HTTP 413)`
- No `QueryTooLarge`

## Remaining Durability Gap

Production is fixed now through the persisted WASM module upload path, but the clean bundle path still needs follow-up.

`TEMPERPAW_GENESIS_BOOTSTRAP_REFS` remains pinned to older Genesis app refs, including:

- `temperpaw/paw-agent@dc6a81fd65ebef9514fd7e91a6b4fae92477c2b7`
- `temperpaw/paw-media@7098fc6c...`

Attempts to publish refreshed Genesis app bundles were blocked by Genesis Git remote failures:

- full clone stalled in `fetch-pack/index-pack`
- shallow clone unsupported
- force-push fallback failed with `send-pack: protocol error: bad band #117`

Until Genesis publish is repaired, production restarts rely on persisted hot-loaded modules being recovered correctly. The source fixes in this branch make the code path durable once merged and bundled, but Genesis app refs still need a successful publish/update.
