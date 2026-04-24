# 058 - Stage-Owned Session Turn Cleanup

Date: 2026-04-24

## Summary

This pass finished the post-cutover cleanup after ADR-0040:

- added ADR-0043 to lock the stage-ownership rule in code shape
- removed local shared tool-catalog definitions from the staged Session-turn WASMs
- moved shared session-turn artifacts and `gen_ai.*` builders behind
  `session-turn-artifacts`
- rewrote `provider_response_applier` to a small stage-owned module
- stripped `provider_caller` of prep/applier-only copied code
- stripped `context_preparer` of provider/applier entrypoints and foreign helper blocks
- tightened the structural cutover test so these regressions fail red

## Red/Green

New red condition:

```text
staged_turn_cutover: os-apps/paw-agent/wasm/context_preparer/src/lib.rs still defines the shared tool catalog locally
```

After the cleanup:

```text
bash os-apps/paw-agent/tests/staged_turn_cutover.sh
result: staged_turn_cutover: ok
```

## ADR

Added:

- `docs/adrs/0043-stage-owned-session-turn-wasms-and-tiny-shared-crates.md`

It records that:

- `context_preparer`, `provider_caller`, and `provider_response_applier` each
  own exactly one Session-turn stage
- only tiny pure shared crates are allowed underneath those stages
- `tool-catalog` and `session-turn-artifacts` are the sanctioned shared crates
- staged crates must not reintroduce foreign `run_*` entrypoints or local
  copies of shared tool/artifact builders

## Rust Verification

Targeted checks:

```text
cargo check --manifest-path os-apps/paw-agent/wasm/context_preparer/Cargo.toml
result: passed
```

```text
cargo check --manifest-path os-apps/paw-agent/wasm/provider_caller/Cargo.toml
result: passed
```

```text
cargo check --manifest-path os-apps/paw-agent/wasm/provider_response_applier/Cargo.toml
result: passed
```

Focused tests:

```text
cargo test --manifest-path os-apps/paw-agent/wasm/tool-catalog/Cargo.toml
result: 1 passed; 0 failed
```

```text
cargo test --manifest-path os-apps/paw-agent/wasm/session-turn-artifacts/Cargo.toml
result: 5 passed; 0 failed
```

```text
cargo test --manifest-path os-apps/paw-agent/wasm/context_preparer/Cargo.toml
result: 3 passed; 0 failed
```

```text
cargo test --manifest-path os-apps/paw-agent/wasm/provider_caller/Cargo.toml
result: 12 passed; 0 failed
```

```text
cargo test --manifest-path os-apps/paw-agent/wasm/provider_response_applier/Cargo.toml
result: 3 passed; 0 failed
```

## Artifact Verification

Built the live WASM suite used by the Session pipeline:

```text
bash os-apps/paw-fs/wasm/blob_adapter/build.sh
result: passed
```

```text
bash os-apps/paw-fs/wasm/workspace_fs/build.sh
result: passed
```

```text
bash os-apps/paw-agent/wasm/build.sh
result: passed
```

Key rebuilt outputs:

- `context_preparer`: 451KB
- `provider_caller`: 547KB
- `provider_response_applier`: 370KB
- `monty_repl (wasip1)`: 6366KB

Also rebuilt the server binary from this worktree:

```text
cargo build -p temperpaw --bin temperpaw-server --release
result: passed
```

## Live Local E2E

Re-verified after rebasing this cleanup onto current `origin/main` (which
already includes the original `llm_caller` removal as PR `#122`).

Started a fresh isolated server:

```text
HOME=/tmp/remove-llm-caller-followup-e2e/home
TURSO_URL=file:/tmp/remove-llm-caller-followup-e2e/paw.db
PORT=61741
PUBLIC_BASE_URL=http://127.0.0.1:61741
OTEL_ENABLED=false
TEMPER_API_KEY=proof-temper-key
PAW_TENANT=default
TEMPERPAW_WASM_STARTUP_POLICY=load-only
./target/release/temperpaw-server
```

Health check:

```text
curl -fsS http://127.0.0.1:61741/healthz
result: passed
```

First re-run note:

- The older structured `user_message = {"steps":[...]}` payload used by the
  earlier proof now fails in `PreparingContext` on current `main` with:
  `user_message is empty - nothing to send to the LLM`
- This is a real shape change in the current prep path, so the live proof below
  uses the currently supported mock-provider input form: a plain string message

Created and configured a real mock-provider Session:

- `session_id = ss-019dc197-3367-7ad2-aa57-ee188616874d`
- `provider = mock`
- `model = mock`
- `temper_api_url = http://127.0.0.1:61741`
- `user_message = "Reply with exactly: live session ok"`
- `tools_enabled = false`

Waited on:

```text
/observe/entities/Session/ss-019dc197-3367-7ad2-aa57-ee188616874d/wait?statuses=Completed,Failed,Cancelled&timeout_ms=60000&poll_ms=500
```

Observed final state:

```text
status = Completed
result = Reply with exactly: live session ok
```

Observed staged transitions:

- `WorkspaceReady` -> `PreparingContext`
- `ContextReady` -> `CallingProvider`
- `ProviderResponseReady` -> `ApplyingProviderResponse`
- `CheckSteering` -> `Steering`
- `FinalizeResult` -> `Completed`
- `MarkTrajectoryEmitted` -> `Completed`

Observed live artifacts:

- `prepared_context_file_id = fl-019dc197-3456-7831-b329-cd3c54fc9053`
- `provider_response_file_id = fl-019dc197-34b7-7680-a64e-4294c5f2a92a`
- `prepared_context_bytes = 53228`
- `prepared_context_entries_loaded = 1`
- `prepared_context_content_files_loaded = 1`
- `provider_request_bytes = 0`
- `provider_response_bytes = 35`
- `session_leaf_id = a-2`

This proves the cleanup did not just preserve compilation. The isolated runtime
still executed the staged Session path end to end with the refactored stage
owners and rebuilt WASM artifacts.
