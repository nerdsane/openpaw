# Proof Report: 049 — Bounded Session Context And Turn Pipeline

## Date
2026-04-15

## Branch / Commit
- **openpaw**: `codex/session-memory-architecture` (proof captured from the branch head after rebase onto `origin/main`)

## What Was Done
Implemented the long-term Temper-native session memory refactor described in ADR-0034.

The change replaces the single large `llm_caller` turn path with an explicit staged Session pipeline:

1. `PreparingContext`
2. `CallingProvider`
3. `ApplyingProviderResponse`

The implementation work included:

1. Adding a new ADR:
   - `docs/adrs/0034-bounded-session-context-and-llm-turn-decomposition.md`
2. Refactoring `llm_caller` into reusable library entry points for:
   - context preparation
   - provider calling
   - provider response application
3. Adding new Temper-native WASM modules:
   - `context_preparer`
   - `provider_caller`
   - `provider_response_applier`
4. Rewiring the Session spec to use the staged pipeline actions and states:
   - `ContextReady`
   - `ProviderResponseReady`
   - `prepare_context`
   - `call_provider`
   - `apply_provider_response`
5. Introducing bounded-context artifacts written to TemperFS:
   - `prepared_context_file_id`
   - `provider_response_file_id`
6. Adding new memory and pipeline metrics:
   - `temper_session_context_tokens`
   - `temper_session_context_bytes`
   - `temper_session_context_prepare_duration_ms`
   - `temper_session_context_entries_loaded`
   - `temper_session_context_content_files_loaded`
   - `temper_session_provider_request_bytes`
   - `temper_session_provider_response_bytes`
   - `temper_session_compaction_trigger_total`
   - `temper_session_large_content_externalized_total`
   - `temper_session_memory_limit_exceeded_total`
7. Updating the Datadog dashboard and monitors so the new metrics are visible and alertable.
8. Fixing compaction token accounting so compaction summaries contribute to future token estimates instead of being stored as `0`.

## Red-Green TDD
### Red
- Added a new architecture contract test:
  - `crates/openpaw/tests/session_turn_architecture.rs`
- The test encoded the required staged Session states, actions, fields, Cedar callbacks, and Datadog metric/dashboard coverage.
- Added a unit test for compaction summary token accounting in:
  - `os-apps/paw-agent/wasm/session-tree-lib/src/lib.rs`
- Before the implementation, these tests failed because the staged pipeline and token accounting changes did not exist.

### Green
- Implemented the spec, policy, WASM, dashboard, monitor, and token-accounting changes.
- Re-ran the targeted tests until they passed.

## Files Changed
- `docs/adrs/0034-bounded-session-context-and-llm-turn-decomposition.md`
- `crates/openpaw/tests/session_turn_architecture.rs`
- `crates/openpaw/src/startup.rs`
- `dd-dashboards/openpaw-overview.json`
- `dd-monitors/openpaw-monitors.json`
- `os-apps/paw-agent/specs/session.ioa.toml`
- `os-apps/paw-agent/specs/model.csdl.xml`
- `os-apps/paw-agent/policies/session.cedar`
- `os-apps/paw-agent/wasm/build.sh`
- `os-apps/paw-agent/wasm/llm_caller/Cargo.toml`
- `os-apps/paw-agent/wasm/llm_caller/src/lib.rs`
- `os-apps/paw-agent/wasm/context_preparer/Cargo.toml`
- `os-apps/paw-agent/wasm/context_preparer/src/lib.rs`
- `os-apps/paw-agent/wasm/provider_caller/Cargo.toml`
- `os-apps/paw-agent/wasm/provider_caller/src/lib.rs`
- `os-apps/paw-agent/wasm/provider_response_applier/Cargo.toml`
- `os-apps/paw-agent/wasm/provider_response_applier/src/lib.rs`
- `os-apps/paw-agent/wasm/context_compactor/src/lib.rs`
- `os-apps/paw-agent/wasm/session-tree-lib/src/lib.rs`
- `os-apps/paw-agent/wasm/monty_repl/src/entity_ops.rs`

## Verification Flow
1. Add failing architecture contract tests for the staged Session flow and Datadog coverage.
2. Add a failing unit test for compaction summary token accounting.
3. Implement the staged Session states, actions, policy callbacks, and WASM integrations.
4. Build the new WASM modules individually.
5. Run the full `os-apps/paw-agent/wasm/build.sh` sweep.
6. Build `openpaw` itself.
7. Start `openpaw-server` from the isolated worktree environment.
8. Create a real Session against the running server and drive it through a mock-provider turn.
9. Query live Session history and confirm the staged state transitions occurred.
10. Record the evidence in this report.

## Verification Results
| Step | Expected | Actual | Status |
|------|----------|--------|--------|
| `cargo test --test session_turn_architecture` in `crates/openpaw` | Architecture contract test should pass only once staged Session flow, Cedar callbacks, and dashboard coverage are present | Passed | PASS |
| `cargo test datadog_configs_use_tenant_aware_entity_queries` in `crates/openpaw` | Dashboard/monitor startup assertions should pass with new metrics present | Passed | PASS |
| Compaction summary token test in `session-tree-lib` | Compaction entries should contribute non-zero estimated tokens | Passed after `append_compaction()` stored estimated summary tokens | PASS |
| `cargo build --target wasm32-unknown-unknown --release` in `os-apps/paw-agent/wasm/llm_caller` | Shared staged-turn library should compile to WASM | Passed | PASS |
| `cargo build --target wasm32-unknown-unknown --release` in `context_preparer`, `provider_caller`, `provider_response_applier` | New staged WASM modules should compile | All passed | PASS |
| `./build.sh` in `os-apps/paw-agent/wasm` | Full paw-agent WASM suite should build cleanly | Passed after fixing unrelated `monty_repl` compile drift | PASS |
| `cargo build -p openpaw --release` | Main server crate should compile with the new contract checks | Passed | PASS |
| `GET /healthz` on live server | Server should boot and report healthy | Returned `200` from `http://127.0.0.1:45991/healthz` | PASS |
| Live Session run with mock provider | Session should complete successfully using staged pipeline | Session `ss-019d93c3-93b8-7f92-bbac-ed6775baa69e` reached `Completed` with result `bounded-session pipeline proof complete` | PASS |
| Live Session history inspection | History should show `PreparingContext -> CallingProvider -> ApplyingProviderResponse` transitions | Observed `WorkspaceReady -> PreparingContext`, `ContextReady -> CallingProvider`, `ProviderResponseReady -> ApplyingProviderResponse`, `RecordResult -> Completed` | PASS |

## Runtime Evidence
The live Session history for `ss-019d93c3-93b8-7f92-bbac-ed6775baa69e` showed the new Temper-native staged pipeline:

1. `WorkspaceReady` moved the Session from `Provisioning` to `PreparingContext`
2. `ContextReady` moved the Session from `PreparingContext` to `CallingProvider`
3. `ProviderResponseReady` moved the Session from `CallingProvider` to `ApplyingProviderResponse`
4. `RecordResult` moved the Session from `ApplyingProviderResponse` to `Completed`

The final Session fields also showed the new artifact and metric-bearing state:

- `prepared_context_file_id = fl-019d93c3-948b-7b41-b3f3-ca34c7c51a00`
- `provider_response_file_id = fl-019d93c3-94f0-7e93-aba3-071ebde0c5fe`
- `context_tokens = 16`
- `prepared_context_bytes = 48439`
- `prepared_context_entries_loaded = 1`
- `prepared_context_content_files_loaded = 1`
- `provider_request_bytes = 0`
- `provider_response_bytes = 39`

## What Worked
- The Session spec split cleanly along real Temper-native orchestration boundaries instead of introducing an out-of-band coordinator.
- Reusing `llm_caller` as an `rlib` kept the provider logic centralized while still letting us split the runtime responsibilities across new WASM modules.
- The new architecture contract test gave a strong regression net for the spec, Cedar policy, and Datadog surface.
- The live history endpoint made it easy to prove the new state machine transitions end-to-end.

## What Didn't Work
- The first isolated startup attempt failed because the proof environment did not yet have the required `paw-fs` WASM artifacts available.
- A later startup path still emitted build-if-missing cargo errors because the isolated environment initially resolved an older toolchain that could not parse `edition = "2024"`.
- `monty_repl` had unrelated compile drift around `runtime_headers`, which had to be corrected before the full paw-agent WASM sweep would go green.

## Limitations
- This proof used the mock provider path for a deterministic end-to-end run. It proves the staged Session pipeline and artifact handling, but it is not a live Anthropic/OpenAI/OpenRouter validation.
- The Datadog dashboard and monitor files were updated and covered by repo tests, but this proof did not deploy them to a live Datadog account from this branch.
- The live server was started with environment overrides to point `HOME`, `RUSTUP_HOME`, and `CARGO_HOME` at an isolated proof setup.

## What Still Doesn't Work
- The server’s startup `build-if-missing` behavior is still sensitive to the active Rust toolchain resolution in isolated environments. The feature works, but the proof surfaced that the startup shellout path should be made more deterministic.
- This change makes long conversations bounded and observable, but it does not yet add a richer document-preserving retrieval policy beyond the current file-backed and search-based path.

## Artifacts
- ADR: `docs/adrs/0034-bounded-session-context-and-llm-turn-decomposition.md`
- Architecture contract test: `crates/openpaw/tests/session_turn_architecture.rs`
- Live health check:
  - `http://127.0.0.1:45991/healthz`
- Live Session id:
  - `ss-019d93c3-93b8-7f92-bbac-ed6775baa69e`
- Proof server command:
  - `RUSTUP_HOME=/Users/seshendranalla/.rustup CARGO_HOME=/Users/seshendranalla/.cargo RUSTUP_TOOLCHAIN=stable HOME=/tmp/openpaw-session-memory-proof-4-home PORT=45991 PUBLIC_BASE_URL=http://127.0.0.1:45991 OTEL_ENABLED=false OPENPAW_WASM_STARTUP_POLICY=build-if-missing TEMPER_API_KEY=proof-temper-key PAW_TENANT=default RUST_LOG=warn ./target/release/openpaw-server`
- Targeted verification commands:
  - `cargo test --test session_turn_architecture`
  - `cargo test datadog_configs_use_tenant_aware_entity_queries`
  - `cargo build --target wasm32-unknown-unknown --release`
  - `cargo build -p openpaw --release`
  - `./build.sh`

## Architecture Diagram
```text
User / Trigger
      |
      v
  Session entity
      |
      v
Provisioning
      |
      v
PreparingContext --(NeedsCompaction)--> Compacting
      |                                     |
      |                                     v
      +------------(CompactionComplete)-----+
      |
      v
CallingProvider
      |
      v
ApplyingProviderResponse
   |            |
   |            +--> ProcessToolCalls / CheckSteering / RecordResult
   |
   v
Completed

Prepared context and provider response bodies are persisted as TemperFS artifacts
instead of staying live in one oversized WASM heap.
```
