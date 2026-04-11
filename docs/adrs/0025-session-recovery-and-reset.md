# ADR-0025: Session Recovery and Conversation Reset

**Status:** Accepted
**Date:** 2026-04-11
**Related:** ADR-0005 (Temper-native orchestration), ADR-0022 (lazy sandbox provisioning), paw-agent/ADR-001 (agent-session separation)

## Context

### Problem 1: Server restart kills all in-flight sessions

When the OpenPaw server crashes or restarts, Phase 7b in `startup.rs` finds all non-terminal Session entities and dispatches `Fail`. Every in-flight session — whether it was mid-LLM-call, mid-tool-execution, or waiting for approval — dies. The user sees an error message in Discord ("process restart — session recovered from Thinking state") and must re-send their message to start over.

This is wasteful because conversation context already survives restarts. The session tree is persisted in TemperFS as a JSONL file (`session_file_id` + `session_leaf_id`). Agent memories persist (scoped to Agent ID). The only thing lost is the ephemeral WASM runtime state (LLM streaming buffers, REPL heap). The session can be resumed from its persisted tree.

### Problem 2: Orphaned tool_use repair lives in the wrong layer

When a session crashes during the Executing state, the assistant's `tool_use` message is recorded in the session tree but the `tool_result` entries may not be. This leaves an orphaned `tool_use` that makes the conversation unparseable for the LLM.

Two band-aid repairs currently handle this:

1. `route_message:858` — when the next user message arrives, checks if the tree leaf is an orphaned `tool_use` and synthesizes error `tool_result` entries. This is chat-routing doing session-lifecycle work.

2. `llm_caller:2468` (`repair_interrupted_tool_use_messages`) — scans the in-memory message array before each LLM call and patches any `tool_use` blocks missing matching `tool_result`s. This is the LLM caller doing session-lifecycle work.

Both repairs trigger too late (on next interaction) and in the wrong layer (routing/LLM calling instead of session lifecycle).

### Problem 3: No way to reset a conversation

`continue_with_new_session` in `route_message` always carries forward `session_file_id` from the prior session. The conversation tree grows indefinitely within a DM channel. There is no mechanism for a user to start a fresh conversation without creating a new Discord channel. Compaction mitigates token count but the history accumulates forever.

## Decision

### 1. Resume same Session via `Recovering` state

Add a `Recovering` state to the Session state machine. On server restart, dispatch `RecoverFromRestart` (not `Fail`) for sessions that were in a recoverable state (Thinking, Executing, Compacting, Steering, WaitingForApproval).

The `RecoverFromRestart` action transitions to `Recovering` and triggers a `session_recoverer` WASM integration that:

1. Loads the session tree from TemperFS
2. Detects orphaned `tool_use` at the leaf and appends synthetic error `tool_result` entries
3. Appends a recovery steering message
4. Writes the repaired tree back to TemperFS
5. Dispatches `RecoveryComplete` which transitions to `Thinking` and re-enters the LLM loop

This is a resume, not a continuation. The same Session entity stays alive. No new entity is created. The ChannelSession doesn't change. The audit trail shows a clean recovery event on the session.

Sessions in Created or Provisioning state (no conversation tree yet) still receive `Fail` — there's nothing to resume.

**Why resume-same-session, not create-continuation-session:**

- The session tree is a channel-level artifact shared across sessions. Creating a new Session wrapping the same tree is an artificial boundary.
- A Session represents one continuous run. A server restart is an interruption in that run, not the end.
- No entity proliferation, no ChannelSession update required, cleaner audit trail.
- Cleanly separates "recover" (same session resumes) from "reset" (new session, new tree).

### 2. Move orphaned tool_use repair into session lifecycle

The `session_recoverer` WASM module is the authoritative repair location. By the time `call_llm` fires after recovery, the tree is clean.

The existing repairs become:

- `route_message:858` — fallback for non-recovery edge cases (session completed normally with orphaned leaf). Delegates to shared `SessionTree::interrupted_tool_results_for_leaf()` method.
- `llm_caller:2468` — defensive safety net with a warning log. Should never trigger after proper recovery.

### 3. Recovery loop prevention

A `recovery_count` counter on the Session entity tracks how many times recovery has been attempted. The `RecoverFromRestart` action has a guard (`recovery_count < 3`) that caps recovery at 3 attempts. If the guard fails, startup falls back to `Fail` and the user sees the error.

Since we resume the same entity (not create a new one), the counter accumulates naturally.

### 4. `/reset` slash command

A new `/reset` Discord slash command lets users start a fresh conversation:

1. Cancels the active Session (if steerable)
2. Expires the current ChannelSession
3. Creates a new Session with no inherited `session_file_id` — a clean tree
4. Creates a new ChannelSession pointing to the new Session

The optional `message` parameter provides the first message for the new conversation. Without it, a system notice acknowledges the fresh start.

## Consequences

### Positive

- **Seamless crash recovery** — users experience a delay, not a failure. No re-sending required.
- **Orphaned tool_use fixed at the right layer** — session lifecycle, not routing or LLM calling.
- **No entity proliferation** — same Session entity survives restarts.
- **Clean audit trail** — session events show crash and recovery inline.
- **Conversation reset capability** — users can start fresh when needed.
- **Temper-native** — Rust startup dispatches one action (`RecoverFromRestart`); WASM handles all orchestration.

### Negative

- **New state in Session spec** — `Recovering` adds complexity to the state machine. Existing self-loop actions (Steer, SwitchProvider, Heartbeat, Fail, Cancel) must include the new state.
- **Recovery may re-execute work** — an LLM call that was in-flight gets re-issued. For Thinking state this is idempotent. For Executing state, some tool side effects may have occurred but results were lost; the LLM will retry from the error tool_results.
- **WaitingForApproval recovery creates stale decisions** — the pending Cedar decision references the old state. On recovery the LLM re-invokes the tool and a new decision is created. The old decision is orphaned (harmless).

### State machine diff

```
States: + Recovering
State vars: + recovery_count (counter, initial 0)

New actions:
  RecoverFromRestart: [Thinking,Executing,Compacting,Steering,WaitingForApproval] → Recovering
  RecoveryComplete: [Recovering] → Thinking

Updated from-lists:
  Steer: + Recovering
  SwitchProvider: + Recovering
  Fail: + Recovering
  Cancel: + Recovering
  Heartbeat: + Recovering

New integration:
  recover_session → session_recoverer WASM (on_failure = Fail)
```
