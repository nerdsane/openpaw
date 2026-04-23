# Proof Report: 055 - Session Lifecycle And LLM Config

## Date

2026-04-23

## Branch / Commit

Branch: `codex/session-lifecycle-config`

## What Was Done

- Added a reusable TemperPaw `SessionLink` entity and `session_link_monitor` WASM integration to supervise child Sessions and notify parent entities on completion or failure.
- Wired `WikiJob` child Session spawning through `SessionLink` instead of leaving the parent to infer child failure out-of-band.
- Removed runtime model/provider fallbacks from core Session, Agent, managed-agent, wiki-job, channel routing, compaction, LLM calling, setup, and deploy paths.
- Required model/provider to come from explicit agent/job config, inherited Session config, or configured setup secrets.

## Verification Flow

1. Red test: added `crates/temperpaw/tests/session_lifecycle_and_config.rs` before implementation.
2. Green implementation: added `SessionLink`, the monitor WASM module, wiki wiring, and explicit model/provider validation.
3. Unit/workspace verification.
4. WASM target verification for touched modules.
5. Local bounded server boot smoke with no LLM provider/model configured.

## Verification Results

| Step | Expected | Actual | Status |
|------|----------|--------|--------|
| `cargo test -p temperpaw --test session_lifecycle_and_config` before implementation | Fails on missing `SessionLink` and hardcoded model/provider defaults | Failed before green changes | Pass |
| `cargo test -p temperpaw --test session_lifecycle_and_config` | New lifecycle/config tests pass | 2 passed | Pass |
| `cargo test --workspace` | Workspace tests pass | 21 + 36 + 2 + 4 + 11 passed | Pass |
| `cargo build --workspace` | Workspace builds | Finished dev build | Pass |
| `cargo fmt --check` | Formatting is clean | No output, exit 0 | Pass |
| `cargo check --target wasm32-unknown-unknown` for touched WASM modules | Touched unknown-unknown WASM modules compile | Passed for session_link_monitor, llm_caller, context_compactor, route_message, wiki build_session_message, managed-agent orchestrator/updater, foresight, consilium, autoreason, and heal modules | Pass |
| `cargo check --target wasm32-wasip1` for `monty_repl` | Monty REPL compiles to WASI | Passed with existing unused doc-comment warning | Pass |
| `bash os-apps/paw-agent/wasm/build.sh` | Paw-agent WASM bundle builds, including new monitor | All modules built; `session_link_monitor` built successfully | Pass |
| Bounded boot smoke | Server reaches `/healthz` without provider/model fallback env | `/healthz` responded on local port 19467; load-only startup logged expected missing local WASM artifact errors | Pass |

## What Worked

- `SessionLink` gives child-session supervision a Temper-native state machine instead of bespoke Rust or job-local polling.
- Wiki jobs now create a reusable child-session link as soon as a child Session is spawned.
- Runtime LLM calls now fail loudly when model/provider are not configured instead of silently choosing Anthropic/Sonnet.
- Server boot reached health with `LLM_PROVIDER` and `LLM_MODEL` unset, proving startup no longer invents those values.

## What Didn't Work

- Native `cargo test` inside `os-apps/paw-agent/wasm/llm_caller` still cannot link host SDK imports (`host_get_context`, `host_http_call`, etc.). The module was verified with the WASM target instead.

## Limitations

- The boot smoke used load-only WASM startup to keep the local run bounded. That mode logs missing artifact errors for modules that were not installed into the isolated temporary home. The paw-agent bundle itself was built separately with `build.sh`.

## What Still Doesn't Work

- A full production-like end-to-end WikiJob run against the deployed Railway instance was not executed from this worktree, because that would require live credentials and mutating the production job/session graph.

## Artifacts

- Boot log: `/tmp/openpaw-session-lifecycle-boot.log`
- Health response capture: `/tmp/openpaw-session-lifecycle-health.out`

## Architecture Diagram

```text
WikiJob(SessionSpawned)
  |
  v
SessionLink.Configure
  |
  v
SessionLink.Watching --CheckChild--> child Session terminal?
  |                                      |
  | no                                   | yes
  v                                      v
ChildPending                       Parent Complete/Fail
  |                                      |
  v                                      v
CheckChild self-loop              ParentNotified / NotifyFailed
```
