# Proof Report: 058 — Session Stall Remediation

## Date

2026-04-24, updated 2026-04-25 with live local E2E evidence

## Branch / Commit

- Branch: `codex/session-stall-remediation`
- Commit: PR head commit
- Companion Temper branch: `codex/session-stall-remediation`

## What Was Done

- Added ADR-0043 for session phase latency budgets and response-application contracts.
- Added configurable budgets for context prepare, provider caller, and provider response apply.
- Emitted `temper_session_phase_duration_ms`, `temper_session_phase_step_duration_ms`, and `temper_session_phase_budget_exceeded_total` from the session turn pipeline.
- Changed fresh session-tree response application so it does not rebuild or serialize the full legacy conversation payload.
- Added workspace provisioning phase metrics.
- Added Datadog dashboard widgets and monitors for session phase budgets and background query projection metrics.
- Fixed `paw-agent` app manifest coverage for terminal Session hooks after live E2E exposed missing `agent_reply` and `emit_ots_trajectory` module declarations.

## Verification Flow

| Step | Expected | Actual | Status |
|------|----------|--------|--------|
| Red test: fresh session-tree response apply | Test fails before helper exists | `cargo test --lib fresh_session_tree_response_apply...` failed with missing `legacy_updated_conversation_payload` | Pass |
| Provider response applier unit tests | Fresh session-tree mode skips legacy payload; legacy mode keeps payload | `cargo test --lib response_apply` passed, 2 tests | Pass |
| Context preparer unit tests | Context assembly helpers still pass after phase instrumentation | `cargo test --lib` passed, 3 tests | Pass |
| Provider caller unit test | Existing progress wrapper behavior preserved | `cargo test --lib provider_progress_wrapper_emits_start_and_end_on_success` passed | Pass |
| Dashboard/monitor guard | New and existing observability queries are covered | `cargo test -p temperpaw dashboard_and_monitors_cover_session_context_metrics` passed | Pass |
| Dashboard JSON | Dashboard and monitor files remain valid JSON | `jq empty dd-dashboards/temperpaw-overview.json` and `jq empty dd-monitors/temperpaw-monitors.json` passed | Pass |
| WASM build: context preparer | Module compiles for `wasm32-unknown-unknown` | `cargo build --target wasm32-unknown-unknown --release` passed | Pass |
| WASM build: provider caller | Module compiles for `wasm32-unknown-unknown` | `cargo build --target wasm32-unknown-unknown --release` passed | Pass |
| WASM build: provider response applier | Module compiles for `wasm32-unknown-unknown` | `cargo build --target wasm32-unknown-unknown --release` passed | Pass |
| WASM build: workspace provisioner | Module compiles for `wasm32-unknown-unknown` | `cargo build --target wasm32-unknown-unknown --release` passed | Pass |
| Red test: terminal Session WASM modules | Test fails before `paw-agent/app.toml` declares terminal hook modules | `cargo test -p temperpaw paw_agent_manifest_declares_terminal_session_wasm_modules -- --nocapture` failed with missing `agent_reply` | Pass |
| Terminal Session WASM manifest guard | App manifest declares modules referenced by terminal Session hooks | Same test passed after declaring `agent_reply` and `emit_ots_trajectory` | Pass |
| Live local E2E: initial Session run | Server boots, direct Session completes under the mock provider | Session `ss-019dc3bb-b4f6-73e3-8bc6-e75b2b35bead` reached `Completed`, but emitted `DeliveryFailed` because `agent_reply` was not declared; `emit_ots_trajectory` was also missing | Pass |
| Live local E2E: fixed Session run | Server boots, direct Session completes, terminal hooks fire, trajectory emits | Session `ss-019dc3c2-b610-7d30-b802-dd0085ab1728` reached `Completed`, emitted `MarkTrajectoryEmitted`, and had no `DeliveryFailed` / `TrajectoryEmissionFailed` | Pass |

## Verification Results

- The response applier no longer constructs a large legacy conversation payload when `PreparedContextArtifact.use_session_tree` is true.
- Phase and step metrics are emitted around artifact reads, provider HTTP, artifact writes, assistant response append, and workspace bootstrap.
- Budget exceedance emits a counter and fails the phase before the broader state timeout.
- Datadog files parse as JSON and tests assert the new dashboard/monitor queries exist.
- The changed WASM modules compile to release WASM.
- A live local server booted with `TEMPERPAW_WASM_STARTUP_POLICY=load-only`, `readyz` returned 200, and direct Session state transitions completed end to end against a fresh file-backed Turso DB.
- The fixed live E2E run completed the Session from `Configure` at `2026-04-25T08:30:10.970690Z` to `RecordResult` at `2026-04-25T08:30:14.628574Z` in about 3.66s, and to `MarkTrajectoryEmitted` at `2026-04-25T08:30:15.259492Z` in about 4.29s.
- Final fixed live phase timings: workspace bootstrap 380ms / workspace ready 381ms; context load 110ms / system prompt assembly 645ms / prepared artifact write 95ms; provider artifact read 54ms / mock provider HTTP 24ms / provider response write 94ms; response applier prepared artifact read 55ms / provider response read 54ms / session-tree append 142ms.
- The final fixed live Session produced `trajectory_id=trj-ss-019dc3c2-b610-7d30-b802-dd0085ab1728` and `trajectory_emission_status=emitted`.

## What Worked

- The targeted red/green tests caught the response-apply contract change cleanly.
- The rebase kept the new `session_turn_artifacts` / `wasm_helpers` cleanup intact while reapplying latency metrics.
- The dashboard and monitor changes are test-covered instead of only visually edited.

## What Didn't Work

- `cargo test --locked` is not usable in the standalone WASM crate directories because their local patch configuration tries to refresh crate-local `Cargo.lock` files. Verification used normal cargo commands, then generated lock churn was removed from the patch.
- The first live E2E run found a real app-bundle wiring bug: `os-apps/paw-agent/specs/session.ioa.toml` references terminal integrations, but `os-apps/paw-agent/app.toml` did not declare `agent_reply` or `emit_ots_trajectory`. The Session still reached `Completed`, but terminal effects failed. This is now covered by a regression test.
- Local `load-only` boot required prebuilt WASM artifacts. `monty_repl` could not be rebuilt against the current local Temper SDK branch because of an `HttpRequest` / `http_call_batch` API mismatch, so the existing bundled artifact was used for the live local E2E.

## Limitations

- I did not replay the exact production dark academia `source_search` session against real external LLM providers and Datadog. The live E2E used the deterministic mock provider, so `provider_http=24ms` proves local provider-adjacent plumbing is fast but does not measure real LLM inference latency.
- The OpenPaw root test workspace is patched to the local `/Users/seshendranalla/Development/temper` checkout, so the `temperpaw` dashboard guard test compiled against that local checkout. The test itself only validates OpenPaw dashboard/monitor definitions.
- I did not run a Discord/Slack channel-transport E2E. In the local `load-only` boot, `paw-channels` reported missing `route_message` and `transport_reconcile` artifacts, which is outside this Session stall patch but should be addressed before a full channel ingress proof.

## What Still Doesn't Work

- Phase metric helper functions are still local to the standalone WASM crates. ADR-0043 records a follow-up to move those into shared WASM helpers if they spread further.
- Session phase metrics will identify future slow substeps, but the exact production stall cannot be fully proven fixed until the branches are deployed and the same workload is observed.
- `paw-channels` needs its terminal transport WASM artifacts available for a full live channel-message replay.

## Artifacts

- ADR: `docs/adrs/0043-session-phase-latency-budgets-and-response-application-contract.md`
- Dashboard: `dd-dashboards/temperpaw-overview.json`
- Monitors: `dd-monitors/temperpaw-monitors.json`
- Session spec: `os-apps/paw-agent/specs/session.ioa.toml`
- App manifest: `os-apps/paw-agent/app.toml`
- WASM modules:
  - `os-apps/paw-agent/wasm/context_preparer/src/lib.rs`
  - `os-apps/paw-agent/wasm/provider_caller/src/lib.rs`
  - `os-apps/paw-agent/wasm/provider_response_applier/src/lib.rs`
  - `os-apps/paw-agent/wasm/workspace_provisioner/src/lib.rs`

## Architecture Diagram

```text
Session action
  -> context_preparer
       emits phase/step durations
       writes prepared-context artifact
  -> provider_caller
       measures artifact read, provider_http, response artifact write
       writes provider-response artifact
  -> provider_response_applier
       reads artifacts
       appends assistant entry to session tree
       dispatches ProcessToolCalls / RecordResult / CheckSteering
       skips legacy conversation serialization for fresh session-tree turns
```
