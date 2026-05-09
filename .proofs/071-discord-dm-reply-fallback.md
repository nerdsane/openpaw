# 071 - Discord DM Reply Fallback

Date: 2026-05-09

## Scope

Investigated Paw not responding in Discord DMs. Live channel state showed ingress was reaching Temper, but reply delivery was failing with:

```text
send_reply: webhook POST failed (HTTP 404)
```

Root cause: Discord reply delivery depended on an in-memory `user_id -> DM channel_id` cache. After reconnects or deploys, the Temper session/thread state could survive while the Discord transport cache was empty, causing `/reply` to return 404 instead of delivering the response.

ADR judgement: no ADR added. This is a narrow transport reliability bug fix that preserves the existing entity/WASM/Cedar architecture and does not introduce a new state machine, policy, integration boundary, trigger, or deployment model.

## Change

- Added a Discord REST helper to open/reuse a DM channel for a recipient id.
- Updated `/reply` and `/typing` handling to reopen and cache the DM channel when the warm cache is missing.
- Kept `thread_id` as the Discord user id, matching the existing DM routing contract.

## Verification

Local red-green:

```text
cargo test -p paw-transport resolve_dm_channel_id_reopens_and_caches_missing_dm_mapping -- --nocapture
cargo test -p paw-transport open_dm_channel_posts_recipient_and_returns_channel_id -- --nocapture
```

Both tests failed before implementation and passed after implementation.

Local regression:

```text
cargo fmt --check
cargo test -p paw-transport -- --nocapture
cargo check -p temperpaw
git diff --check
```

Results:

```text
paw-transport: 25 passed
cargo check -p temperpaw: passed
cargo fmt --check: passed
git diff --check: passed
```

GitHub/GHCR:

```text
git push origin HEAD:main
gh run list --repo nerdsane/temperpaw --workflow Docker --branch main --limit 1
```

Result:

```text
completed success Fix Discord DM reply fallback Docker main push 25601029022
```

Railway:

```text
railway up --detach --message "Deploy GHCR edge 9682dd4d Discord DM fallback"
```

Deployment:

```text
484d6fba-4401-4395-8df3-9efbd619031d SUCCESS
```

Live version:

```json
{
  "version": "sha-9682dd4d",
  "sha": "9682dd4d03154278d5796829bcf0595df3375066"
}
```

Live readiness:

```json
{
  "status": "ready",
  "discord": {
    "status": "connected",
    "configured": true,
    "connected": true,
    "desired_state": "connected",
    "connection_state": "Connected",
    "last_error": null,
    "next_retry_at": null
  }
}
```

Live end-to-end reply delivery:

Dispatched:

```http
POST /tdata/Channels('en-019d9109-f010-7ff1-bcb0-72700a94ef23')/Paw.Channel.SendReply
```

with:

```json
{
  "thread_id": "1018228973869727785",
  "content": "Paw DM transport hotfix is live; this is the end-to-end delivery check.",
  "agent_entity_id": "codex-discord-dm-hotfix"
}
```

Railway logs showed:

```text
discord reply webhook missing DM channel cache; reopening DM channel
delivering discord reply
event emitted ReplyDelivered
transition applied ReplyDelivered
trajectory.entry action=ReplyDelivered success=true
```

Live ingress check:

After deploy, Railway logged a real Discord DM:

```text
discord message received author_id=1018228973869727785 channel_id=1494059279202779136
discord receive_message dispatched
route_message: routed thread 1018228973869727785 to session ss-019e0b0a-94cf-7f61-80c8-d965936e6910
```

Note: the routed session is currently `WaitingForApproval`; that is separate session state from the Discord reply transport bug.
