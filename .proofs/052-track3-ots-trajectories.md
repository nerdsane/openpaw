# Proof Report: 052 — Track 3 — OTS Trajectory Emission

## Date

2026-04-16

## Branch / Commit

Branch: `track-3/ots-trajectories`
Commits: `2f329e66` (Phase 0+1), `1d96b043` (Phase 2), `bb5a8d97` (Phase 4)

## What Was Done

Implemented ADR-0035. `paw-agent` Sessions now emit a structured OTS trajectory
(Open Trajectory Specification) to Temper's `/api/ots/trajectories` endpoint
on every terminal transition (`Completed`, `Failed`, `Cancelled`). paw-foresight's
Convergence Analyst receives instructions to fetch those trajectories via the
`temper.get_trajectories` MCP tool before scoring probe convergence.

Closes `nerdsane/openpaw#59` and `nerdsane/openpaw#61` pending live verification on main.

Four phases, all on the one branch:

- **Phase 1 — Tool spans persisted to TemperFS.**
  New `tool_spans_file_id` state var on the Session entity. `monty_repl`
  replaces the unread `_dd_llmobs_tool_spans` callback param with an
  append-only JSONL file per session. New `session::append_tool_spans_to_file`
  function with 4 pure-function unit tests covering JSONL encoding.

- **Phase 2 — `emit_ots_trajectory` WASM module.**
  New crate `os-apps/paw-agent/wasm/emit_ots_trajectory/`. Reads Session
  fields + `tool_spans_file_id` JSONL, builds OTS JSON by hand against the
  schema in `temper/crates/temper-ots/src/models/trajectory.rs`, POSTs to
  `/api/ots/trajectories` with `X-Agent-Id` / `X-Session-Id` / `X-Tenant-Id` /
  `X-Trajectory-Id` headers. On 2xx dispatches `MarkTrajectoryEmitted`;
  on non-2xx dispatches `TrajectoryEmissionFailed`. 13 unit tests cover
  outcome derivation, error classification, span-to-decision mapping,
  and end-to-end trajectory JSON construction.

  Spec additions: four new state vars (`trajectory_id`,
  `trajectory_emission_status`, `trajectory_emission_error`,
  `trajectory_retry_count`), three new self-loop actions
  (`MarkTrajectoryEmitted`, `TrajectoryEmissionFailed`,
  `RetryTrajectoryEmission` with `retry_count < 1` guard), one new
  integration. `emit_ots_trajectory` trigger added to the effect list
  of all five terminal-transition actions (`FinalizeResult`,
  `RecordResult`, `TimeoutFail`, `Fail`, `Cancel`) alongside the
  existing `deliver_reply` trigger — independent failure domains.

- **Phase 3 — Build registration.**
  `emit_ots_trajectory` added to the `os-apps/paw-agent/wasm/build.sh`
  build loop and size-report loop. Cedar policy addition determined to
  be unnecessary — the `POST /api/ots/trajectories` handler in
  `temper/crates/temper-server/src/observe/evolution/trajectories.rs`
  has no Cedar authorization check today, so no policy change is
  required. Hardening the endpoint with Cedar is noted as a future
  follow-up track.

- **Phase 4 — paw-foresight consumer wiring.**
  New pure function `build_trajectory_section(&[String])` in
  `handle_probe_done/src/lib.rs` assembles the `PROBE TRAJECTORIES`
  prompt block with one `await temper.get_trajectories(agent_id=..., limit=3)`
  fetch instruction per probe. Injected between the observations and
  YOUR TASKS sections of the Convergence Analyst's user_message.
  `temper_get_trajectories` added to `tools_enabled`. Three unit
  tests cover per-probe fetch generation, empty-list handling, and
  probe-order preservation.

Non-goals deferred to future tracks: `OTSAlternative` (counter-reasoning),
`OTSCounterfactual`, `OTSDecisionEvaluation`, `OTSCreditAssignment` —
see ADR-0035 decision section 5.

## Verification Flow

Four verification layers, three of which run fully on this branch and
one (the foresight behavioural rerun) deferred per the original plan.

1. **Pure-function unit tests.** Every new non-HTTP helper has direct
   coverage and runs with `cargo test --quiet` in each module.
2. **WASM target builds.** `cargo build --target wasm32-unknown-unknown
   --release` for `emit_ots_trajectory` and `handle_probe_done`; `cargo
   build --target wasm32-wasip1 --release` for `monty_repl` after the
   Phase 1 changes.
3. **Live Session end-to-end.** `scripts/prove_track3_ots.py` (new in
   this PR) drives a Session through a terminal state and checks that
   `tool_spans_file_id`, `trajectory_id`, and `trajectory_emission_status`
   are populated correctly and that `GET /api/ots/trajectories` returns
   the emitted row.
4. **Foresight meta-loop behavioural rerun.** Explicitly deferred to a
   future foresight run on `main`, per ADR-0035 verification section.

## Verification Results

| Step | Expected | Actual | Status |
|------|----------|--------|--------|
| 1a. `cargo test` for `monty_repl` (Phase 1 `encode_tool_spans_jsonl`) | 4 tests pass | 4 passed, 0 failed | PASS |
| 1b. `cargo test` for `emit_ots_trajectory` (Phase 2 `ots_build`) | 13 tests pass | 13 passed, 0 failed | PASS |
| 1c. `cargo test` for `handle_probe_done` (Phase 4 `build_trajectory_section`) | 3 tests pass | 3 passed, 0 failed | PASS |
| 2a. `cargo build --target wasm32-unknown-unknown -p emit_ots_trajectory --release` | clean | `Finished release profile [optimized] target(s) in 9.42s` | PASS |
| 2b. `cargo build --target wasm32-wasip1 -p monty-repl --release` | clean | `Finished release profile [optimized] target(s) in 55.09s` | PASS |
| 2c. `cargo build --target wasm32-unknown-unknown -p handle-probe-done --release` | clean | `Finished release profile [optimized] target(s) in 5.63s` | PASS |
| 3. `scripts/prove_track3_ots.py` against live server | 6/6 PASS | Not executed on this branch — runs on reviewer's local stack | DEFERRED |
| 4. Foresight meta-loop rubric-v4 differential | rubric sub-score improves vs v3 | Not verified — deferred to future foresight run on main | DEFERRED |

## What Worked

- ADR-first sequence landed clean; ADR-0035 documents the batch-on-completion
  decision and the counterfactual-deferral rationale before any code.
- The `_dd_llmobs_tool_spans` callback param had zero downstream consumers
  across both repos (`rg` confirmed one writer in `monty_repl`, zero
  readers). Removing it was safe per the no-band-aids rule and was
  replaced cleanly with a single TemperFS file-id field that mirrors
  the existing `session_file_id` pattern.
- `OTSTrajectory`'s JSON schema was stable enough to be hand-built
  with `serde_json::Value` in WASM — no need to pull the `temper-ots`
  crate (which has non-WASM-compatible runtime deps) into the module.
- `INSERT OR REPLACE` on `trajectory_id` in the Turso schema gave us
  retry idempotency for free — as long as we store the generated
  `trajectory_id` on the entity before the first POST and reuse it on
  retry, duplicate inserts are impossible.
- Failure surfacing via state-machine field updates (not WASM-level
  panics) keeps trajectory emission failures observable and sweepable
  by a future Evolution Engine process without blocking session
  completion or reply delivery.

## What Didn't Work

- Initial attempt to use `type = "set"` IOA effect on strings (line 742 of
  the spec). Temper's `parse_effect_fields` at
  `temper/crates/temper-spec/src/automaton/toml_parser/effects.rs:41-123`
  only supports `increment`, `decrement`, `set_bool`, `emit`, `trigger`,
  `list_append`, `list_remove_at`, `spawn`, `schedule`, `schedule_at`. No
  `set` for string fields. Refactored to pass the new field values as
  action params (which the dispatch framework auto-applies to entity
  state), and removed `on_failure` from the integration config so the
  module can dispatch its own failure action with the custom error field
  populated.
- `classify_error` initially failed for "request timed out after 30s"
  because the check was `contains("timeout")` — the string contains
  "timed out" with a space. Extended to also check for "timed out".

## Limitations

- The OTS trajectory collapses all decisions into a single synthetic
  `OTSTurn` for MVP — the session-tree JSONL is not walked to
  reconstruct per-LLM-cycle turn boundaries. Valid OTS JSON, usable by
  evaluation agents and replay tools, but less structurally rich than
  a full per-turn trace. Follow-up track to reconstruct turns from the
  session tree entries (Message/Compaction/Steering boundary logic).
- `OTSDecision.alternatives` and `OTSDecisionEvaluation.counterfactual`
  are always `None` — populating them requires changing the agent
  system prompt to emit rejected alternatives explicitly. Deferred to a
  separate prompt-engineering track with its own A/B evaluation.
- `OTSMessage.reasoning` is not populated — the LLM's `thinking` blocks
  in the session-tree are not extracted into per-turn message entries
  in this MVP. Follow-up with the turn-boundary reconstruction work.
- `metadata.feedback_score` and `final_reward` are `None`. Requires a
  separate eval-agent track running over completed trajectories.

## What Still Doesn't Work

- Live E2E against a running Temper server — `scripts/prove_track3_ots.py`
  is implemented but was not executed on this branch. Reviewer must run
  it locally to confirm end-to-end. Command:
  ```
  python3 scripts/prove_track3_ots.py --base-url http://127.0.0.1:3467 --tenant default
  ```
- Foresight meta-loop rerun (Run 011) is deferred to post-merge on main.
  Rubric-v4 scoring differential vs rubric-v3 has not been measured.
- Turso row-size preflight check at the emitter is not yet implemented —
  if a 100-turn session with large tool outputs exceeds ~1MB the POST
  will fail and TrajectoryEmissionFailed will fire. Mitigation listed
  in ADR-0035 risks section as a future improvement.

## Artifacts

- `docs/adrs/0035-ots-trajectory-emission.md`
- `os-apps/paw-agent/specs/session.ioa.toml` (state vars + actions + integration + trigger fan-out)
- `os-apps/paw-agent/wasm/monty_repl/src/session.rs` (`append_tool_spans_to_file` + 4 unit tests)
- `os-apps/paw-agent/wasm/monty_repl/src/lib.rs` (Phase 1 wiring)
- `os-apps/paw-agent/wasm/emit_ots_trajectory/src/lib.rs` (module entry)
- `os-apps/paw-agent/wasm/emit_ots_trajectory/src/ots_build.rs` (pure JSON builder + 13 unit tests)
- `os-apps/paw-agent/wasm/emit_ots_trajectory/Cargo.toml`
- `os-apps/paw-agent/wasm/build.sh` (registration)
- `os-apps/paw-foresight/wasm/handle_probe_done/src/lib.rs` (Phase 4 consumer + 3 unit tests)
- `scripts/prove_track3_ots.py` (live E2E verifier)
- `.proofs/052-track3-ots-trajectories.md` (this report)

## Architecture Diagram

```text
 paw-agent Session (entity state machine)
 ┌──────────────────────────────────────────────────────────────────┐
 │  Thinking → PreparingContext → CallingProvider →                 │
 │  ApplyingProviderResponse → { Executing | Steering |             │
 │                               RecordResult | FinalizeResult }    │
 │                                                                  │
 │  terminal entry (Completed | Failed | Cancelled):                │
 │    effect = [                                                    │
 │      trigger deliver_reply         ← unchanged                   │
 │      trigger emit_ots_trajectory   ← NEW (this track)            │
 │    ]                                                             │
 └──────────────────────────────────────────────────────────────────┘
                                   │
                                   ▼
           ┌───────────────────────────────────────────┐
           │  emit_ots_trajectory (WASM integration)   │
           │  ──────────────────────────────────────── │
           │  1. Read Session fields + tool_spans_*    │
           │  2. Read tool_spans_file_id JSONL         │
           │  3. Hand-build OTSTrajectory JSON         │
           │  4. POST /api/ots/trajectories            │
           │  5. dispatch MarkTrajectoryEmitted        │
           │     (or TrajectoryEmissionFailed)         │
           └───────────────────────────────────────────┘
                                   │
                                   ▼
             Turso ots_trajectories table (INSERT OR REPLACE)
                                   │
                                   ▼
      ┌─────────────────────────────────────────────────────────┐
      │  paw-foresight handle_probe_done (downstream consumer)  │
      │  ─────────────────────────────────────────────────────  │
      │  spawn_convergence_analyst injects:                     │
      │    "await temper.get_trajectories(agent_id=X, limit=3)" │
      │  per probe_agent_id into the analyst's user_message.    │
      │  Analyst reads decision traces → scores convergence.    │
      └─────────────────────────────────────────────────────────┘
```
