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
- Red follow-up: `missing_current_attachment_param_does_not_reuse_stale_channel_attachment` failed by reusing a stale `pawfs_file` attachment when the current action omitted `reply_attachments_json`.
- Green: `cargo test --manifest-path os-apps/paw-channels/wasm/send_reply/Cargo.toml --quiet`
- Result: 4 tests passed.

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
- `send_reply`: `1ad697cf014c884767a8a91cba463693e532af5ae52dacb8af89ba344df4b123`

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

Follow-up stale attachment regression:

- User reported every subsequent Paw message included the same cat image.
- Root cause: `send_reply` used Channel state as the fallback for `reply_attachments_json`, so a later text-only action could inherit the previous attachment.
- Hot-loaded corrected `send_reply`: `1ad697cf014c884767a8a91cba463693e532af5ae52dacb8af89ba344df4b123`
- Text-only proof action omitted `reply_attachments_json` while Channel state still had stale attachment history.
- Action result fields showed `reply_attachments_json: ""`.
- Production logs after the corrected upload:
  - `invoking WASM integration module` with hash `1ad697cf014c884767a8a91cba463693e532af5ae52dacb8af89ba344df4b123`
  - `delivering discord reply`, `content_len=95`
  - `Channel.ReplyDelivered Connected -> Connected ... succeeded`
  - no `delivered discord reply attachments` for that proof send
  - no PawFS `$value` fetch for the cat file after that proof send

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

## 2026-06-18 Final DM Image E2E

Additional production failure modes were fixed and verified after the original hotfix:

- `monty_repl` now captures generated image results as dispatch side effects, so a PawFS image is still propagated to `reply_attachments_json` even when the Python snippet prints a summary instead of returning the structured image object.
- `openai_codex_image_generate` records PawFS file metadata without returning inline base64 in the awaited action response, avoiding Monty host buffer overflow.
- `route_message` production was hot-loaded to the bounded SessionEntry leaf lookup implementation, replacing the stale ordered `SessionEntries?$orderby=Sequence desc&$top=1` scan that could fail with HTTP 413.
- `agent_reply` production was hot-loaded to the implementation that forwards `Session.reply_attachments_json` into `Channel.SendReply`.

No new ADR was added for this pass: the changes are implementation fixes inside the existing media, route, and reply-delivery ADRs listed above. The state-machine architecture did not change.

Focused tests:

- Red then green: `cargo test --manifest-path os-apps/paw-agent/wasm/monty_repl/Cargo.toml --quiet`
  - Final result: 73 tests passed.
  - New guards cover file-only `__temperpaw_image` results and dispatch-captured image results.
- `cargo test --manifest-path os-apps/paw-media/wasm/openai_codex_image_generate/Cargo.toml --quiet`
  - Final result: 12 tests passed.
- `cargo test -p temperpaw --test paw_media_image_generation --locked --quiet`
  - Final result: 10 tests passed.
- `cargo test --manifest-path os-apps/paw-channels/wasm/route_message/Cargo.toml --quiet`
  - Final result: 25 tests passed.
- `cargo test --manifest-path os-apps/paw-agent/wasm/agent_reply/Cargo.toml --quiet`
  - Final result: 7 tests passed.

Production module hashes verified via `/observe/wasm/modules/*`:

- `route_message`: `e90792f38e37d6ba25d0619c47667869a621c7e089a77dc01d0cd395535f8a97`
- `monty_repl`: `af3293660446b6ff5de831d4d02f149d49a0a8f253ce620696ecf364a41f1676`
- `openai_codex_image_generate`: `f5ba13c268f0cf639114922080502f5bda715c206c2188f1fb5f0baa91cdc67f`
- `send_reply`: `1ad697cf014c884767a8a91cba463693e532af5ae52dacb8af89ba344df4b123`
- `agent_reply`: `e36d025b52bf420b3e5a8fd1c6fb10a1a9eca157f253c683f492b7e80fe6495f`

Final plain-DM proof:

- Incoming Channel action: `Paw.Channel.ReceiveMessage?await_integration=true`
- `message_id`: `codex-proof-dm-cat-final-20260618031511`
- `channel_entity_id`: `en-019eba01-164d-7d60-ab4d-5e710a5e39d6`
- `thread_id`: `1018228973869727785`
- Content: `Generate an image of a cat for me.`
- Routed Session: `ss-019ed8b9-bf11-73b0-989a-573d93ada06f`
- Session status: `Completed`
- Session result:
  - `Done.`
  - `File:fl-019ed8ba-bb1b-7d31-81bc-77382a4ab782`
- Session `reply_attachments_json`:
  - `kind`: `pawfs_file`
  - `file_id`: `fl-019ed8ba-bb1b-7d31-81bc-77382a4ab782`
  - `media_generation_id`: `en-019ed8b9-edd8-7882-ba60-5171db72d659`
  - `path`: `/generated/images/en-019ed8b9-edd8-7882-ba60-5171db72d659.png`

Media entity:

- `MediaGenerationRequest`: `en-019ed8b9-edd8-7882-ba60-5171db72d659`
- `Status`: `Complete`
- `Provider`: `openai_codex`
- `Model`: empty in entity state, normalized by renderer to the Codex backend default
- `provider_response_id`: `resp_04b2bf5e721b2a5f016a3362ccdad48196a5f0003958a4b721`
- `result_file_id`: `fl-019ed8ba-bb1b-7d31-81bc-77382a4ab782`

File entity:

- `File`: `fl-019ed8ba-bb1b-7d31-81bc-77382a4ab782`
- `Status`: `Ready`
- `Path`: `/generated/images/en-019ed8b9-edd8-7882-ba60-5171db72d659.png`
- `MimeType`: `image/png`
- `size_bytes`: `2327315`
- `content_hash`: `sha256:4836f4bf1f7274bd3b8c1c6215a71f7c4b439e49fc0963023d648e6ab86251e0`
- Downloaded proof file: `/tmp/temperpaw-final-plain-dm-cat.png`
- Local file inspection: `PNG image data, 1536 x 1024, 8-bit/color RGB, non-interlaced`
- Local sha256: `4836f4bf1f7274bd3b8c1c6215a71f7c4b439e49fc0963023d648e6ab86251e0`

Discord delivery logs:

- `2026-06-18T03:16:25.909517Z`
  - target: `paw_transport::discord::transport`
  - message: `delivered discord reply attachments`
  - `attachment_count=1`
  - `thread_id=1018228973869727785`
- `2026-06-18T03:16:26.795519Z`
  - target: `wasm_guest`
  - message: `agent_reply: dispatched reply for agent aj-019d8cde-5bf6-7472-8ad1-2b2798c822b1 to thread 1018228973869727785`
  - session: `ss-019ed8b9-bf11-73b0-989a-573d93ada06f`

This proves the production Discord DM path now accepts a normal image request, invokes `temper.image_generate`, creates a real PawFS PNG through the Codex media backend, records the image attachment on the Session, forwards it through `agent_reply` and `send_reply`, and uploads it back to the Discord DM as a file attachment.
