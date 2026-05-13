# 072 - Discord Approval-Wait DM Routing

Date: 2026-05-09

## Scope

Follow-up investigation after the Discord DM reply transport hotfix. Live ingress and reply delivery were healthy, but a real DM was routed to a prior agent session stuck in `WaitingForApproval`.

Root cause: `route_message` treated `WaitingForApproval` as a non-terminal session state to continue waiting on. For Discord DM threads, that left follow-up user messages bound to an approval-gated session and made Paw appear unresponsive.

ADR judgement: no ADR added. This is a narrow routing reliability fix inside the existing `Channel.ReceiveMessage -> route_message` WASM transition. It does not add a new entity type, policy boundary, trigger, or deployment model.

## Change

- Updated `os-apps/paw-channels/wasm/route_message/src/lib.rs`.
- Added an explicit helper for statuses that should continue via a fresh session.
- Treat `WaitingForApproval` like a blocked continuation boundary for DM routing.
- When a bound session is `WaitingForApproval`, cancel that old session and create a fresh continuation under the same agent, then update the existing `ChannelSession.session_entity_id`.

## Verification

Local red-green:

```text
cargo test --manifest-path os-apps/paw-channels/wasm/route_message/Cargo.toml waiting_for_approval_should_not_swallow_follow_up_messages -- --nocapture
```

The test failed before implementation and passed after implementation.

Local regression:

```text
cargo test --manifest-path os-apps/paw-channels/wasm/route_message/Cargo.toml -- --nocapture
cargo fmt --check
git diff --check
cargo check -p temperpaw
bash os-apps/paw-channels/wasm/build.sh
```

Results:

```text
route_message: 14 passed
cargo fmt --check: passed
git diff --check: passed
cargo check -p temperpaw: passed
os-apps/paw-channels/wasm/build.sh: passed
```

GitHub/GHCR:

```text
git push origin HEAD:main
gh run view 25603783399 --repo nerdsane/temperpaw
```

Result:

```text
Docker run 25603783399
commit a10541982d4fd6e67cbc3cbf19660745c621cbd4
conclusion success
url https://github.com/nerdsane/temperpaw/actions/runs/25603783399
```

Railway:

```text
railway up --detach --message "Deploy GHCR edge a1054198 approval-wait DM routing"
```

Deployment:

```text
686744c3-d51c-45f9-a484-6cb0c9a758a1 SUCCESS
source image ghcr.io/nerdsane/temperpaw:edge
```

Live version:

```json
{
  "version": "sha-a1054198",
  "sha": "a10541982d4fd6e67cbc3cbf19660745c621cbd4"
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

Live end-to-end production DM:

Railway received the user's real Discord DM after the deploy:

```text
discord message received author_id=1018228973869727785 channel_id=1494059279202779136 message_id=1502689111876702228 preview="?"
```

The old blocked session was cancelled:

```text
Session ss-019e0b0a-94cf-7f61-80c8-d965936e6910
WaitingForApproval -> Cancelled
```

A fresh continuation was created and bound to the DM thread:

```text
route_message: creating session via http://127.0.0.1:8080/tdata/Sessions with 3 bytes
route_message: routed thread 1018228973869727785 to session ss-019e0d49-de20-72a0-ab8a-a4ece8b4baae
ChannelSession en-019e0862-4bb3-7723-a960-455a8331e8f4 UpdateSession success
```

The new session completed:

```text
Session ss-019e0d49-de20-72a0-ab8a-a4ece8b4baae
Status Completed
```

The reply was delivered back to Discord:

```text
delivering discord reply thread_id=1018228973869727785 content_len=1414
ReplyDelivered success=true
agent_reply: dispatched reply for agent aj-019d8cde-5bf6-7472-8ad1-2b2798c822b1 to thread 1018228973869727785
```

Final live entity checks:

```text
ChannelSession en-019e0862-4bb3-7723-a960-455a8331e8f4
Status Active
session_entity_id ss-019e0d49-de20-72a0-ab8a-a4ece8b4baae

Old session ss-019e0b0a-94cf-7f61-80c8-d965936e6910
Status Cancelled

New session ss-019e0d49-de20-72a0-ab8a-a4ece8b4baae
Status Completed
```

Live exact command follow-up:

The user then sent the original command again:

```text
discord message received message_id=1502689592426627082 preview="can you rerun the review job restart it"
```

It routed to a new continuation instead of being swallowed:

```text
route_message: routed thread 1018228973869727785 to session ss-019e0d4b-a974-7903-be5a-02a4c9c80274
ChannelSession en-019e0862-4bb3-7723-a960-455a8331e8f4 UpdateSession success
```

The command completed and restarted the review job:

```text
Session ss-019e0d4b-a974-7903-be5a-02a4c9c80274
Status Completed

Result:
Restarted.
CurationJob:en-019e0d4c-2e8b-7182-85a6-b7124f429944 -> Ready
DesignLanguage:en-019e0a77-f95b-7d41-b312-af7f27e3e22a -> UnderReview
```

The exact command reply was delivered to Discord:

```text
delivering discord reply thread_id=1018228973869727785 content_len=654
ReplyDelivered success=true
agent_reply: dispatched reply for agent aj-019d8cde-5bf6-7472-8ad1-2b2798c822b1 to thread 1018228973869727785
```

Follow-on review job state shortly after the reply:

```text
CurationJob en-019e0d4c-2e8b-7182-85a6-b7124f429944
Status Running
session_id ss-019e0d4c-3959-7c92-adee-2c3bbe9a8db5

Session ss-019e0d4c-3959-7c92-adee-2c3bbe9a8db5
Status CallingProvider

DesignLanguage en-019e0a77-f95b-7d41-b312-af7f27e3e22a
Status UnderReview
thumbnail_verified false
quality_review_passed false
```
