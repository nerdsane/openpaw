# Proof Report: 056 - OpenAI SSE Batch Completion And Terminal Cleanup

## Date

2026-04-23

## Branch / Commit

Branch: `codex/fix-openai-sse-batch`

## What Was Done

- Added a parser seam in `llm_caller` for OpenAI/Codex SSE responses so incomplete streams are rejected unless `response.completed` is present.
- Added regression tests proving that a completed stream preserves all sibling `function_call` items and that a truncated stream cannot return a partial tool batch.
- Extended `Session.RecordResult` so terminal completion can clear `pending_tool_calls`, `pending_tool_context`, and `pending_decision_id`.
- Cleared those pending execution fields on all `RecordResult` paths, including `temper.done()` in `monty_repl` and direct provider-completion paths in `llm_caller`.
- Extended `Session.FinalizeResult` and `steering_checker` so the normal successful steering completion path also clears `pending_tool_calls`, `pending_tool_context`, and `pending_decision_id`.
- Added repo-level regression tests to keep the spec, CSDL, and runtime cleanup aligned.

## Verification Flow

1. Red test: added `llm_caller` SSE parser regressions before the helper existed.
2. Green implementation: introduced the SSE parser/helper and retried only on missing `response.completed`.
3. Red test: added a repo-level regression asserting `RecordResult` can clear pending execution state.
4. Green implementation: updated the session spec, CSDL, `monty_repl`, and `llm_caller` completion paths.
5. Live E2E red check: booted a no-Discord local server with `openai_codex` and a Codex token, ran a real OData session, and observed that the session completed via `FinalizeResult` with stale `pending_tool_calls`.
6. Red test: added a repo-level regression asserting `FinalizeResult` can clear pending execution state.
7. Green implementation: updated the session spec, CSDL, and `steering_checker` finalize path.
8. Restarted the live server from the updated worktree and reran the same Codex-backed session over OData.
9. Ran focused and broader tests.
10. Built the main `temperpaw` crate and exercised CLI/runtime checks, including a live provider-backed session.

## Verification Results

| Step | Expected | Actual | Status |
|------|----------|--------|--------|
| `cargo test openai_sse_parser -- --nocapture` in `os-apps/paw-agent/wasm/llm_caller` before implementation | Fails because parser seam is missing | Failed with unresolved helper functions | Pass |
| `cargo test openai_sse_parser -- --nocapture` after implementation | New SSE regressions pass | 2 passed | Pass |
| `cargo test record_result_clears_pending_tool_state_on_terminal_completion --test session_turn_architecture -- --nocapture` before implementation | Fails because `RecordResult` cannot clear pending fields | Failed on missing spec/runtime cleanup | Pass |
| `cargo test record_result_clears_pending_tool_state_on_terminal_completion --test session_turn_architecture -- --nocapture` after implementation | Regression passes | 1 passed | Pass |
| `cargo test --test session_turn_architecture finalize_result_clears_pending_tool_state_on_terminal_completion -- --nocapture` before implementation | Fails because `FinalizeResult` cannot clear pending fields | Failed on missing spec/runtime cleanup | Pass |
| `cargo test --test session_turn_architecture -- --nocapture` after `FinalizeResult` fix | Repo-level architecture tests pass | 6 passed | Pass |
| `cargo test --lib -- --nocapture` in `os-apps/paw-agent/wasm/llm_caller` | All crate unit tests pass | 36 passed | Pass |
| `cargo test --test session_lifecycle_and_config -- --nocapture` in `crates/temperpaw` | Session lifecycle/config tests pass | 2 passed | Pass |
| `cargo build -p temperpaw` | Main crate builds cleanly | Finished dev build | Pass |
| `cargo run -p temperpaw -- --help` | Binary starts and prints CLI usage | `temperpaw-server` help printed with `run` and `doctor` commands | Pass |
| `cargo run -p temperpaw -- doctor` | Non-destructive runtime sanity check runs | Doctor completed; data/vault/database ok; API key and Discord config missing | Pass |
| Live no-Discord server boot with `LLM_PROVIDER=openai_codex`, `LLM_MODEL=gpt-5.4`, `OPENAI_CODEX_TOKEN=...`, `TEMPERPAW_WASM_STARTUP_POLICY=build-if-missing` | Server builds/registers bundled WASM modules and reaches healthy | Healthy on `http://127.0.0.1:3478/tdata`; Discord/Slack intentionally not started | Pass |
| Live OData Session `ss-019dbc4b-93ac-72b1-a245-4439729ee9b3` before `FinalizeResult` fix | Successful completion reveals any stale terminal execution state | Completed with `fields.result="sessions=0"` but stale `fields.pending_tool_calls` remained set | Pass |
| Live OData Session `ss-019dbc4f-7215-74b2-b82b-b5e07768dcce` after `FinalizeResult` fix | Successful completion clears all pending execution state | Completed with `result="sessions=3"`, `pending_tool_calls=""`, `pending_tool_context=""`, `pending_decision_id=""` | Pass |

## What Worked

- The OpenAI/Codex parser now treats `response.completed` as the batch integrity boundary instead of accepting whatever `output_item.done` events happened to arrive first.
- Completed streams preserve multiple sibling tool calls in order, which removes the orphaned-call failure mode behind this incident.
- Terminal sessions now explicitly clear pending execution/approval fields on both `RecordResult` and `FinalizeResult`, so entity state no longer implies work is still queued after completion.
- A real no-Discord `openai_codex` session ran end to end over OData and hit the actual provider, tool, steering, and terminal-completion pipeline.
- The main server crate and CLI surface still build and run after the change set.

## What Didn't Work

- A full provider-backed `cargo test` shape for `llm_caller` hit the known native host-symbol linking issue when Cargo tried to build the WASM crate as a native dylib (`host_get_context`, `host_http_call`, etc.). `cargo test --lib` passed and covered the unit tests we changed.

## Limitations

- The live E2E used direct authenticated OData session calls rather than a Discord transport, intentionally matching the requested no-Discord path.
- The tool exercise depended on model behavior. In the pre-fix live run, the model invoked `temper.list` with an empty filter and still completed after the tool error; the important verification point was terminal cleanup on the real provider/tool path.

## Artifacts

- This report: `.proofs/056-openai-sse-batch-completion.md`
- Live session evidence: `/tmp/openpaw-codex-session-entity-2.json`, `/tmp/openpaw-codex-session-entity-3.json`
- Server log: `/tmp/openpaw-codex-e2e-server.log`

## Architecture Diagram
```text
OpenAI SSE stream
  |
  v
parse_openai_sse_response_body
  |                    \
  | saw response.completed \
  v                        \ missing response.completed
complete batch               -> retry/fail, never emit partial tool batch
  |
  v
ProcessToolCalls -> monty_repl -> HandleToolResults -> CallingProvider
                                                     |
                                                     v
                                          CheckSteering or RecordResult
                                               |                |
                                               v                v
                                          FinalizeResult   RecordResult
                                               \              /
                                                v            v
                                clear pending_tool_calls/tool_context/decision_id
```
