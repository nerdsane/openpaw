# Proof Report: 050 — Track 1 Session Reliability Hardening

## Date

2026-04-16

## Branch / Commit

`feat/track1-session-reliability` (worktree at `/Users/seshendranalla/Development/openpaw-track1-reliability`), branched from `origin/main` at `c0b6f29e`.

## What Was Done

OpenPaw-side of Track 1 — "Session Reliability and Orchestration Hardening". Four phases, targeting four linked issues in the openpaw repo. The Temper-side (ADR-0045 timeout default + ADR-0046 optimistic-concurrency retry) ships in `nerdsane/temper` under a separate PR on the same branch name.

### Phase 2b — Decouple Heartbeat from the sequence-advance hot path (openpaw#63, openpaw#60)

- Removed the `{ type = "trigger", name = "heartbeat_typing" }` effect from the `Heartbeat` action in `os-apps/paw-agent/specs/session.ioa.toml` so heartbeats no longer advance the entity sequence.
- Deleted the `heartbeat_typing` integration block and the WASM module directory `os-apps/paw-agent/wasm/heartbeat_typing/`.
- Removed `heartbeat_typing` from the build-time module list (`wasm/build.sh`) and the Cedar `http_call` allow list (`policies/session.cedar`).
- Added an inline `wasm_helpers::send_typing_indicator(...)` call inside `wasm/monty_repl/src/session.rs::send_heartbeat` to preserve the Discord typing-indicator contract on the exact path the deleted trigger previously covered. `llm_caller` already called this helper directly and is unchanged.
- **Net behavioural effect**: typing indicators still fire on every heartbeat moment, but with no entity-sequence side effect, so `ProcessToolCalls` callbacks can no longer race a same-tick `Heartbeat` persist. This removes the primary openpaw#63 failure mode (stuck-in-Thinking sessions) and, as a side effect, the openpaw#60 stuck-in-Steering variant that shared the same callback-drop root cause.

### Phase 2c — Steering finalization observability (openpaw#60)

Added a `ctx.log("info", ...)` at the `FinalizeResult` dispatch site in `wasm/steering_checker/src/lib.rs` including `session_id`, `follow_up_count`, and `result_len`. Datadog log analytics can now confirm the callback reaches the server (the Track 1 fix should drop `FinalizeResult` drop rate to zero).

### Phase 3 — Tool-call checkpointing (openpaw#66)

Added a self-loop `CheckpointToolBatch` action on the `Executing` state in `session.ioa.toml`:

```toml
[[action]]
name = "CheckpointToolBatch"
kind = "input"
from = ["Executing"]
to = "Executing"
params = ["pending_tool_calls", "pending_tool_context", "repl_file_id", "session_leaf_id"]
effect = [
  { type = "increment", var = "checkpoint_count" },
  { type = "trigger", name = "run_tools" }
]
```

Added a `checkpoint_count` counter state variable for the runaway-guard signal, and added `CheckpointToolBatch` to the Cedar allow list for system-principal actions.

Inside `wasm/monty_repl/src/lib.rs`, inserted a checkpoint boundary at the top of each tool-call loop iteration:

- Chunk size fixed at `CHECKPOINT_EVERY_N = 20` (empirical fuel cost ~400M per `temper.get`; 20 × 400M = 8B sits comfortably under the 120B ceiling).
- Runaway guard `MAX_CHECKPOINTS_PER_TURN = 50`: if `checkpoint_count` reaches 50, monty_repl dispatches `Fail` with a clear error rather than another `CheckpointToolBatch`.
- The checkpoint re-uses the existing Cedar pause re-entry path (`pending_tool_context` with `{completed_results, remaining_tool_calls}`) — same schema, same resume branch at the top of the handler.
- REPL state saves to TemperFS via `session::save_repl_to_file`, same as the Cedar pause does today.

### Phase 4 — Fuel budget (openpaw#64)

Raised `max_fuel` for the `run_tools` integration from `50_000_000_000` to `120_000_000_000` in `session.ioa.toml`. With Phase 3 landed each chunk stays well below even the old ceiling, so this bump is pure headroom.

## Verification Flow

Local verification only (no live Discord/LLM/sandbox credentials exercised in this session). All changes were built, type-checked, spec-parsed, and Cargo-checked against the main-branch dependencies.

1. `cargo build --workspace` on the OpenPaw Rust workspace (crates/openpaw, crates/openpaw-cli, crates/paw-transport) against the Temper dependency pinned to `nerdsane/temper@main`.
2. `cargo build --target wasm32-unknown-unknown --release` on `os-apps/paw-agent/wasm/steering_checker` — exercises the Phase 2c log addition.
3. `cargo build --target wasm32-wasip1 --release` on `os-apps/paw-agent/wasm/monty_repl` — exercises the Phase 2b inline typing call and the Phase 3 checkpoint logic.
4. Python `tomllib` parse of `os-apps/paw-agent/specs/session.ioa.toml` — confirms the spec is syntactically valid TOML and that `CheckpointToolBatch` + `checkpoint_count` are present and `heartbeat_typing` is absent.
5. Read of `os-apps/paw-agent/policies/session.cedar` — confirms `CheckpointToolBatch` is in the system-principal action list and `heartbeat_typing` is removed from the http_call allow list.

## Verification Results

| Step | Expected | Actual | Status |
|------|----------|--------|--------|
| OpenPaw workspace build | Clean compile | `Finished dev profile ... in 2m 32s` | PASS |
| steering_checker WASM build | Clean compile | `Finished release profile ... in 7.40s` | PASS |
| monty_repl WASM build | Clean compile | `Finished release profile ... in 1m 12s` | PASS |
| session.ioa.toml TOML parse | Spec parses, 34 actions total | `Parse OK; Total actions: 34` | PASS |
| CheckpointToolBatch in spec | Present | `CheckpointToolBatch present: True` | PASS |
| heartbeat_typing in spec | Absent | `heartbeat_typing present: False` | PASS |
| checkpoint_count state var | Present | `checkpoint_count present: True` | PASS |
| Cedar allow list updated | CheckpointToolBatch added, heartbeat_typing removed | Both confirmed via grep | PASS |

## What Worked

- All four phases compile and parse cleanly.
- The Cedar pause re-entry code in `monty_repl` (lines 160-195 of `lib.rs`) is exactly the right shape to re-use for checkpoint resumption — the checkpoint emits a `pending_tool_context` with the same `{completed_results, remaining_tool_calls}` schema the Cedar path already reads on re-entry. No new entry logic needed.
- `llm_caller` already calls `wasm_helpers::send_typing_indicator` directly; Phase 2b's inline typing call in `monty_repl` restores parity. The pre-change code was actually firing typing twice per heartbeat in `llm_caller` (once via trigger cascade, once directly) — removing the cascade collapses to one call per module.

## What Didn't Work

Nothing blocking. One caveat noted below under Limitations.

## Limitations

- **No live E2E run in this session.** A full proof ideally runs a paw-foresight orchestrator with ≥148 tool calls and confirms (a) no sessions stuck in Thinking/Steering after trigger removal, (b) at least two `CheckpointToolBatch` transitions appear in the OData event log on a tool-heavy turn, (c) Datadog typing-indicator latency stays p99 < 500 ms, (d) `temper_entity_concurrency_retry_total` baselines near zero. That requires Discord tokens, an LLM provider key, a sandbox provider (Modal/Tensorlake), and the full Datadog wiring. This session produced the code; the E2E proof should run against the merged branch before we close openpaw#60, #63, #64, #66.
- **Crash-time recovery for a mid-checkpoint session is out of scope in this PR.** The forward-path checkpoint resolves the fuel-exhaustion + atomicity problem described in openpaw#66. If the server crashes between `CheckpointToolBatch` dispatches, the existing `RecoverFromRestart` / `RecoveryComplete` flow runs — `pending_tool_context` is a state field that will be present on recovery, but `session_recoverer` does not yet read it to route the session back into `Executing`. That extension is planned as a follow-up (small patch to `session_recoverer`).
- **No DST test exercising the race** is included on the OpenPaw side. The DST story lives in the Temper PR — the actor-level retry has a test path there. OpenPaw's side of the fix is observable (the trigger is gone) rather than retriable.

## What Still Doesn't Work

Nothing from this track's scope. Items explicitly deferred:

- Mid-turn crash recovery via `session_recoverer` (see Limitations).
- `checkpoint_every_n` as a per-deployment config key (currently a WASM constant).
- Datadog alert wiring for `temper_entity_concurrency_retry_total` (operations task, not code task).

## Artifacts

- Session spec: `os-apps/paw-agent/specs/session.ioa.toml` (diff: removed trigger from Heartbeat effect, removed heartbeat_typing integration block, added CheckpointToolBatch action, added checkpoint_count state var, bumped run_tools max_fuel).
- Cedar policy: `os-apps/paw-agent/policies/session.cedar` (added CheckpointToolBatch, removed heartbeat_typing).
- Build script: `os-apps/paw-agent/wasm/build.sh` (removed heartbeat_typing from both module lists).
- WASM modules: `os-apps/paw-agent/wasm/monty_repl/src/` (session.rs typing call + lib.rs checkpoint), `os-apps/paw-agent/wasm/steering_checker/src/lib.rs` (FinalizeResult log).
- Deleted: `os-apps/paw-agent/wasm/heartbeat_typing/` (directory + all contents).

## Architecture Diagram

```text
Before (race vector):

  monty_repl                                        llm_caller
       │                                                 │
       │ send_heartbeat(...)                             │ send_heartbeat(...)
       ▼                                                 ▼
  Session.Heartbeat action ──┐             Session.Heartbeat action ──┐
       │                     │                 │                       │
       │  effect: trigger    │                 │   effect: trigger     │
       ▼  heartbeat_typing   │                 ▼   heartbeat_typing    │
  [sequence advances]        │            [sequence advances]          │
       │                     │                 │                       │
       ▼                     ▼                 ▼                       ▼
  heartbeat_typing WASM → POST /typing    heartbeat_typing WASM → POST /typing
                                                                       ▲
                                          send_typing_indicator(...)   │
                                              duplicate direct call ───┘

  ProcessToolCalls callback arrives → expected_seq=N but actual=N+2 → ConcurrencyViolation → action dropped

After (trigger removed):

  monty_repl                                        llm_caller
       │                                                 │
       │ send_heartbeat(ctx, url, tenant):               │ send_heartbeat(ctx, url, tenant)
       │   POST /Sessions/Heartbeat [no effect]          │   POST /Sessions/Heartbeat [no effect]
       │   send_typing_indicator(...)  ◄── new inline    │ send_typing_indicator(...)  ◄── already here
       ▼                                                 ▼
  No sequence advance from heartbeat  ——————————  No sequence advance from heartbeat

  ProcessToolCalls persist is clean. Temper-side concurrency retry (ADR-0046) covers unknown-unknown races.


Phase 3 — checkpoint loop:

  run_tools dispatch ─▶ monty_repl ──▶ [i=0] … [i=19] → i % 20 == 0 and remaining > 0?
                                                                │
                                                                ▼ yes
                                                  save REPL state to TemperFS
                                                  build pending_tool_context { completed, remaining }
                                                  set_success_result("CheckpointToolBatch", params)
                                                                │
                                                                ▼
                                 Session.CheckpointToolBatch (Executing → Executing)
                                                                │
                                                    increment checkpoint_count
                                                    trigger run_tools
                                                                │
                                                                ▼
                                                   monty_repl re-entry reads
                                                   pending_tool_context via the
                                                   existing Cedar-resume branch
                                                   → continues at call 20 …
```
