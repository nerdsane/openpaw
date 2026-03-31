# Proof Report: 008 — Curl-Style Channel Conversation Continuation

## Date
2026-03-27

## Branch
`feat/openpaw-self-heal-loop-codex`

## What Was Proven
The channel/session path preserves conversation continuity across messages on the same thread when driven through the OData API instead of Discord.

The verified path was:

`curl/OData -> Channel.ReceiveMessage -> route_message -> ChannelSession lookup -> AgentRoute -> first agent -> SendReply webhook -> second Channel.ReceiveMessage on same thread -> continuation agent from prior session tree -> SendReply webhook`

## Code Change That Made This Reliable
`route_message` now forwards the full runtime config needed by channel-created agents, including `sandbox_url` and other `OpenPaw.Configure` fields, instead of dropping them on the floor during initial route-based agent creation.

File changed:
- `os-apps/paw-channels/wasm/route_message/src/lib.rs`

## Verification Flow
1. Started a local webhook collector.
2. Created a fresh `Channel` with `webhook_url` pointing at that collector.
3. Registered an `AgentRoute` for that channel with:
   - a small system prompt that requires remembering and recalling a token
   - `sandbox_url=http://127.0.0.1:3477`
4. Sent message 1 on a fresh thread:
   - `Remember this token: moon-biscuit-42`
5. Received webhook reply 1:
   - `REMEMBERED moon-biscuit-42`
6. Sent message 2 on the same `channel_id + thread_id + author_id`:
   - `What token did I ask you to remember?`
7. Received webhook reply 2:
   - `RECALL moon-biscuit-42`
8. Verified the second turn used a continuation agent whose `parent_agent_id` points at the first agent and whose `session_file_id` matches the first agent's session tree.

## Verification Results
| Step | Expected | Actual | Status |
|------|----------|--------|--------|
| First reply | Agent should acknowledge the token | Reply was `REMEMBERED moon-biscuit-42` from agent `019d2ccf-a45c-7273-9f78-af0b653b4840` | PASS |
| Same-thread continuity | Second reply should recall the prior token | Reply was `RECALL moon-biscuit-42` on the same thread | PASS |
| Continuation agent | Second turn should not be a fresh blank conversation | Second agent `019d2ccf-adb6-7a62-adbf-e13b042423c7` had `parent_agent_id=019d2ccf-a45c-7273-9f78-af0b653b4840` | PASS |
| Session tree reuse | Continuation should reuse the same session history | Both turns used `session_file_id=019d2ccf-a4b8-7063-bf6e-40ba78aa82da` | PASS |
| ChannelSession rebinding | ChannelSession should now point at the continuation agent | `ChannelSession('019d2ccf-a473-7203-98b2-a4288b99bbff')` rebound to the second agent | PASS |
| Sandbox forwarding | Route-created agent should preserve requested sandbox | Proof route used `sandbox_url=http://127.0.0.1:3477` | PASS |

## Key Artifacts
- Proof script: `scripts/prove_channel_continuation.py`
- Channel: `019d2ccf-a2b3-7121-b6d4-b15fbfb0d345`
- Channel route id: `019d2ccf-a3ca-7273-976b-906d9dcf340c`
- Channel name: `curl-proof-1774573298`
- Thread id: `thread-1774573298`
- ChannelSession: `019d2ccf-a473-7203-98b2-a4288b99bbff`
- First agent: `019d2ccf-a45c-7273-9f78-af0b653b4840`
- Second agent: `019d2ccf-adb6-7a62-adbf-e13b042423c7`
- Session file: `019d2ccf-a4b8-7063-bf6e-40ba78aa82da`

## Exact Replies
Message 1:
- Input: `Remember this token: moon-biscuit-42`
- Reply: `REMEMBERED moon-biscuit-42`

Message 2:
- Input: `What token did I ask you to remember?`
- Reply: `RECALL moon-biscuit-42`

## Rerun
From the worktree:

```bash
python3 scripts/prove_channel_continuation.py
```
