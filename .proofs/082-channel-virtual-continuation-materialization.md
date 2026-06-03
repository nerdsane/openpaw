# 082 Channel Virtual Continuation Materialization

Date: 2026-06-03

## Scope

Follow-up investigation for live Discord thread amnesia after OpenAI Codex auth
failures.

The bug was not merely first-turn context preparation. The live failure path was:

1. A channel-routed Session failed at the OpenAI Codex auth boundary before
   `provider_response_applier`.
2. Because `provider_response_applier` never ran, the first-turn
   `SessionEntry` rows were never durably materialized.
3. The failed Session still had `session_file_id=session-entries:<session_id>`,
   `session_leaf_id=u-<session_id>-0`, and
   `session_entries_materialized=false`.
4. On the next Discord message, `route_message` tried to append to that empty
   SessionEntry tree.
5. The missing parent leaf was treated as a recoverable append failure, so the
   router started a clean continuation and dropped prior conversational context.

## Fix

`os-apps/paw-channels/wasm/route_message/src/lib.rs` now:

- imports and uses `create_initial_session_entries`;
- materializes the virtual first turn when `session_entries_materialized` is
  explicitly `false` and there are no prior SessionEntry rows;
- appends the new user message to the materialized prior user entry;
- marks carried continuation Sessions as `session_entries_materialized=true`;
- no longer treats `session entries continuation missing parent leaf` as a safe
  clean-continuation fallback.

ADR: `os-apps/paw-channels/adrs/003-virtual-session-continuation-materialization.md`.

## Red/Green Evidence

Red:

- The existing route-message unit expectation allowed
  `session entries continuation missing parent leaf` to start a clean
  continuation. The targeted test failed once the desired behavior was changed.

Green:

- `cargo fmt --manifest-path os-apps/paw-channels/wasm/route_message/Cargo.toml`
- `CARGO_NET_GIT_FETCH_WITH_CLI=true cargo test --manifest-path os-apps/paw-channels/wasm/route_message/Cargo.toml -- --nocapture`
  - 22 passed
- `CARGO_NET_GIT_FETCH_WITH_CLI=true bash os-apps/paw-channels/wasm/build.sh`
  - built `channel_connect`, `send_reply`, `transport_reconcile`, and
    `route_message`

## Hot Upload

Patched `route_message.wasm` was hot-uploaded to production before the image
deploy so new channel routes could use the fix immediately.

- module: `route_message`
- sha256: `039a7ea7b7a6afe4a5a9c42569cf8624440edec2ce806f4b0bea9a42191cb039`
- size: `517993`

## Production E2E

Production endpoint:

- `https://openpaw-production.up.railway.app`
- pre-image version during proof:
  `sha-7ccf62264c3c93bd3f5b09c0bab88cac2743d758`

Proof created a synthetic failed prior Session matching the real auth-failure
shape:

- prior Session: `ss-019e8f0e-e8d0-7702-9c45-9b53a25cb51b`
- prior status: `Failed`
- prior `session_file_id`:
  `session-entries:ss-019e8f0e-e8d0-7702-9c45-9b53a25cb51b`
- prior `session_entries_materialized`: `false`
- prior SessionEntry count before route: `0`

Then it created a proof Channel/ChannelSession and dispatched
`Paw.Channel.ReceiveMessage`.

Continuation result:

- continuation Session: `ss-019e8f0e-fe6d-7132-b3ea-d0783128a528`
- ChannelSession: `en-019e8f0e-f3c3-7422-946c-456c8cb9d823`
- continuation status: `Failed` at the expected OpenAI Codex auth boundary
- continuation `parent_session_id`:
  `ss-019e8f0e-e8d0-7702-9c45-9b53a25cb51b`
- continuation `session_file_id` reused the prior
  `session-entries:<prior_session_id>` ref
- continuation `session_entries_materialized`: `true`
- continuation `session_leaf_id`: `u-2`
- prepared context contained both the prior user message and follow-up user
  message

Durable `SessionEntry` rows after route:

- `h-ss-019e8f0e-e8d0-7702-9c45-9b53a25cb51b`, sequence `0`, header
- `u-ss-019e8f0e-e8d0-7702-9c45-9b53a25cb51b-0`, sequence `1`, user,
  parented to the header
- `u-2`, sequence `2`, user, parented to
  `u-ss-019e8f0e-e8d0-7702-9c45-9b53a25cb51b-0`

Assertions passed:

- `empty_virtual_before_route=true`
- `same_session_entries_ref=true`
- `continuation_marked_materialized=true`
- `prior_user_materialized=true`
- `continuation_appended_to_prior_user=true`
- `prepared_context_contains_both_messages=true`

The proof ChannelSession was expired and the proof Channel was archived after
the assertion.

## Katagami Review Job Status

The ten quality-review jobs referenced by the operator were checked separately:

- 6 completed
- 4 failed

The four failures were artifact/input issues, not OpenAI Codex auth or session
continuity:

- one missing `Files(...)/$value` artifact returned HTTP 404
- three shadcn preview-shot files were missing required shots/component recipes

## User-Facing Root Cause

The hard `OpenAI Codex sign-in is required` message is real: production does not
currently have a valid OpenAI Codex OAuth token. The route now fails explicitly
at that auth boundary rather than surfacing the raw `token_revoked` body.

The ongoing-session amnesia was a second bug layered on top of auth failure:
auth failure prevented first-turn SessionEntries from being materialized, and
the channel router treated that empty tree as permission to start clean. This
fix preserves the thread context on the next message even if the previous
OpenAI Codex turn failed before provider response application.
