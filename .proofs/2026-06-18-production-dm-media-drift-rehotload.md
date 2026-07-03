# Production DM Media Drift Re-hotload

Date: 2026-06-18

## Scope

Re-verify the production Discord DM image path after a user reported Paw still
falling back to a generated SVG instead of returning a generated image
attachment.

This is a follow-up to `.proofs/2026-06-17-production-dm-media-and-reply-hotfix.md`.
No new ADR was added: the architecture did not change. The issue was production
serving stale WASM module hashes after the earlier successful proof.

## Regression Observed

The user-facing failed reply said:

> Done — the image generator path is blocked by the current Codex/OpenAI account model permissions, so I closed the loop with a generated SVG instead.
> Created: `/workspace/cat-sunlit-window.svg`

Production trace:

- Session: `ss-019eda7a-abfb-75e0-9abd-e672b7d55694`
- Status: `Completed`
- Result: exact SVG fallback text above
- `reply_attachments_json`: empty

Production WASM hashes had drifted from the fixed set:

- `route_message`: stale `9f679aba98010b87a777224555e74521cc3e27b7e8b4719399c835ec6a4df408`
- `monty_repl`: stale `a32adbc638414343dbcfad68d72eb3308d1b643930c9b49c6f76f0ab4ce232e2`
- `openai_codex_image_generate`: stale `9c5520c2bacf3600380760b2e07c5794949a2779476cef964ac1fa197ff1bb3c`
- `send_reply`: stale `aa92dcd9e1dc06e8fd2c928b1ad6b08845b2325230e5c8d5057ae02f3076ddfd`
- `agent_reply`: stale `6af95d432bed49c23a0b145c67ef7ab782f59dbfab2388d89d26a11d15225bfa`

## Re-hotloaded Modules

Re-hotloaded the fixed module set from PR #413 / commit
`2ec64fd0 Fix Paw DM image generation delivery`, then verified
`/observe/wasm/modules` showed each module cached with the expected hash:

- `route_message`: `e90792f38e37d6ba25d0619c47667869a621c7e089a77dc01d0cd395535f8a97`
- `monty_repl`: `af3293660446b6ff5de831d4d02f149d49a0a8f253ce620696ecf364a41f1676`
- `openai_codex_image_generate`: `f5ba13c268f0cf639114922080502f5bda715c206c2188f1fb5f0baa91cdc67f`
- `send_reply`: `1ad697cf014c884767a8a91cba463693e532af5ae52dacb8af89ba344df4b123`
- `agent_reply`: `e36d025b52bf420b3e5a8fd1c6fb10a1a9eca157f253c683f492b7e80fe6495f`

## Live DM Image Proof

Injected a normal Discord-DM-shaped `Channel.ReceiveMessage`:

- Channel: `en-019eba01-164d-7d60-ab4d-5e710a5e39d6`
- Thread: `1018228973869727785`
- Message: `codex-proof-dm-cat-after-expire-20260618113453`
- Content: `Generate an image of a cat for me.`

Observed:

- Session: `ss-019eda82-7fce-73f2-85af-7d13b0d218b1`
- Session status: `Completed`
- Session result: `Done. Image generated here: /generated/images/en-019eda83-499e-7461-a01c-73f0ea4f9370.png`
- Session `reply_attachments_json`: one PawFS PNG attachment
- MediaGenerationRequest: `en-019eda83-7ca9-75a0-b844-b86050c9427a`
- MediaGenerationRequest status: `Complete`
- Result file: `fl-019eda84-4e22-7513-9cdb-3d412bd7bbcb`
- File status: `Ready`
- File MIME: `image/png`
- File size: `2188244`
- File content hash: `sha256:ddeaa6038ecfcb0c512c2bb2c789ed2f44b084b2c54ffb851c8b9f38d5404586`

Downloaded the file through `Files('fl-019eda84-4e22-7513-9cdb-3d412bd7bbcb')/$value`:

- Local proof file: `/tmp/temperpaw-rehotload-cat.png`
- `file`: `PNG image data, 1122 x 1402, 8-bit/color RGB, non-interlaced`
- `wc -c`: `2188244`
- `shasum -a 256`: `ddeaa6038ecfcb0c512c2bb2c789ed2f44b084b2c54ffb851c8b9f38d5404586`
- Visual sanity check: real generated orange cat PNG.

Discord delivery logs:

- `2026-06-18T11:36:10.565689Z` `paw_transport::discord::transport`
  logged `delivered discord reply attachments`, `attachment_count=1`,
  `thread_id=1018228973869727785`.
- `2026-06-18T11:36:10.637330Z` `wasm_guest` logged
  `agent_reply: dispatched reply` for
  `ss-019eda82-7fce-73f2-85af-7d13b0d218b1`.

## Text-only Attachment Regression Proof

The Channel entity still had stale image attachment state before this check, so
this verified that fixed `send_reply` uses the current action params rather than
leaking old Channel attachment fields.

Injected another DM-shaped message:

- Message: `codex-proof-dm-text-noattachment-20260618114700`
- Content: `Reply with exactly: text-only attachment check.`

Observed:

- Session: `ss-019eda86-e43a-7e20-b0b8-68024ac68feb`
- Session status: `Completed`
- Result: `text-only attachment check`
- Channel `reply_attachments_json` after reply: empty string
- Discord logs showed `delivering discord reply` and `agent_reply: dispatched reply`
- No `delivered discord reply attachments` log was emitted for that text-only reply.

## Linear

Linear `ARN-56` was reopened with drift evidence, updated with this proof, and
moved back to Done.

## Remaining Durability Note

The live production path is fixed again through hotloaded modules. To make this
survive restart or redeploy, PR #413 / commit `2ec64fd0` must be the deployed
artifact source, or the Genesis/app bundle module set must be refreshed so
production cannot reload the stale hashes again.
