# Proof Report: 017 — Discord End to End

## Date

2026-03-27

## Branch / Commit

- Branch: `feat/openpaw-self-heal-loop-codex`
- Commit: working tree implementation

## Vision Target

This proof targets the top `.vision` priority:

- `Discord end-to-end`

It is the first human-facing proof that the deployed OpenPaw service can actually be used through Discord instead of only through OData or synthetic channel injection.

## What Was Done

- Kept Discord transport wiring intact while moving the demo proofing path to channel/webhook automation first
- Prepared the system for a real DM-driven verification run

## Flow Diagram

```text
human DM on Discord
    |
    v
Discord gateway / transport
    |
    v
Channel.ReceiveMessage
    |
    v
AgentRoute -> Paw
    |
    v
Channel.SendReply
    |
    v
Discord DM reply
```

## What Is Intended To Be Proven

- Discord messages reach the governed `Channel` model.
- Paw can be invoked from the real transport, not just synthetic tests.
- Replies make it back out through Discord.

## Verification Flow

1. Start the daemon with `DISCORD_BOT_TOKEN` and `ANTHROPIC_API_KEY`
2. Send a DM to the bot from a real Discord account
3. Verify the DM is routed into `Channel.ReceiveMessage`, an agent reply is generated, and a DM response is delivered back

## Verification Results

- Not executed in this environment.
- This phase requires a human to send the DM.

## Honest Assessment Against Vision

- Proven by implementation:
  - The transport code is present and remains in the architecture.
- Not proven by this report:
  - That Discord gateway auth works on this branch.
  - That a DM really reaches Paw and results in a delivered reply.
  - That Discord thread/session continuity behaves correctly.
- Still below vision:
  - The human experience described in `.vision` is not demonstrated until this proof is actually run.

## Artifacts

- [`crates/paw-transport/src/discord/transport.rs`](/Users/seshendranalla/Development/openpaw-codex/crates/paw-transport/src/discord/transport.rs)
- [`souls/paw.md`](/Users/seshendranalla/Development/openpaw-codex/souls/paw.md)
