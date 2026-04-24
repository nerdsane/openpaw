# 056 - Remove `llm_caller` Cutover

Date: 2026-04-24

## Summary

Completed the one-shot Session-turn cutover from the fresh `main` worktree:

- removed the legacy `call_llm` integration from `Session`
- deleted `os-apps/paw-agent/wasm/llm_caller`
- made `context_preparer`, `provider_caller`, and `provider_response_applier` standalone WASM owners
- added ADR-0040 to record the final architecture decision
- centralized prompt/executor tool defaults and alias normalization in `tool-catalog`

## Structural Red/Green

Started with a failing structural cutover script:

- `os-apps/paw-agent/tests/staged_turn_cutover.sh`

Initial failure:

```text
staged_turn_cutover: legacy call_llm integration is still present in os-apps/paw-agent/specs/session.ioa.toml
```

After the refactor:

```text
os-apps/paw-agent/tests/staged_turn_cutover.sh
result: staged_turn_cutover: ok
```

## Rust Verification

Touched crate checks:

```text
cargo check
cwd: os-apps/paw-agent/wasm/context_preparer
result: passed
```

```text
cargo check
cwd: os-apps/paw-agent/wasm/provider_caller
result: passed
```

```text
cargo check
cwd: os-apps/paw-agent/wasm/provider_response_applier
result: passed
```

```text
cargo check
cwd: os-apps/paw-agent/wasm/monty_repl
result: passed
note: one pre-existing unused-doc-comment warning in monty_repl
```

Touched crate tests:

```text
cargo test
cwd: os-apps/paw-agent/wasm/tool-catalog
result: 1 passed; 0 failed
```

```text
cargo test
cwd: os-apps/paw-agent/wasm/session-turn-artifacts
result: 5 passed; 0 failed
```

```text
cargo test
cwd: os-apps/paw-agent/wasm/context_preparer
result: 34 passed; 0 failed
```

```text
cargo test
cwd: os-apps/paw-agent/wasm/provider_caller
result: 34 passed; 0 failed
```

```text
cargo test
cwd: os-apps/paw-agent/wasm/provider_response_applier
result: 34 passed; 0 failed
```

## WASM Artifact Verification

Built the actual paw-agent WASM suite through the normal script:

```text
./build.sh
cwd: os-apps/paw-agent/wasm
result: passed
```

Key built artifacts:

- `context_preparer`: 442KB
- `provider_caller`: 509KB
- `provider_response_applier`: 365KB
- `monty_repl (wasip1)`: 6306KB

This verified that the artifact pipeline no longer expects `llm_caller` to exist.

## Runtime Verification

### First isolated boot: unrelated restore-state blocker

The first local boot attempt used the default persisted local DB and hit an
existing registry-restore problem unrelated to this cutover:

```text
Error: Failed to restore registry from Turso: Failed to restore tenant 'default' into registry:
failed to parse IOA for tenant 'default', entity 'File':
validation error: unsupported effect type 'set_counter_from_param'
```

That told us the branch still needed a clean isolated environment for honest
runtime proofing.

### Second isolated boot: clean DB, exposed app-bundle gap

Started `temperpaw-server` from the release binary in a fresh temp HOME + fresh
Turso file:

```text
HOME=/tmp/remove-llm-caller-e2e.FxE1zg/home \
TURSO_URL=file:/tmp/remove-llm-caller-e2e.FxE1zg/paw.db \
PORT=61074 \
PUBLIC_BASE_URL=http://127.0.0.1:61074 \
OTEL_ENABLED=false \
TEMPER_API_KEY=proof-temper-key \
PAW_TENANT=default \
TEMPERPAW_WASM_STARTUP_POLICY=load-only \
./target/release/temperpaw-server
```

`GET /healthz` succeeded, and a live Session could be created. The first Session
run failed in `PreparingContext`, but for a separate startup-bundle issue:

```text
content file write failed (HTTP 500):
{"error":{"code":"BlobAdapterError","message":"Blob adapter failed: WASM module 'blob_adapter' not found for tenant 'default'"}}
```

The Session history still proved the cutover had wired correctly up to the new
stage boundary:

- `ProvisionWorkspace` -> `Provisioning`
- `WorkspaceReady` -> `PreparingContext`
- `Fail` from integration `prepare_context`

This failure was not caused by `llm_caller` removal; the isolated app bundle was
missing unrelated `paw-fs` artifacts.

### Fix for isolated bundle gap

Built the missing `paw-fs` WASMs locally:

```text
./build.sh
cwd: os-apps/paw-fs/wasm/blob_adapter
result: passed
```

```text
./build.sh
cwd: os-apps/paw-fs/wasm/workspace_fs
result: passed
```

Verified artifacts now exist:

- `os-apps/paw-fs/wasm/blob_adapter.wasm`
- `os-apps/paw-fs/wasm/workspace_fs.wasm`

### Final live local E2E: PASS

Restarted the server in a brand-new isolated environment:

```text
HOME=/tmp/remove-llm-caller-e2e.nZRDcq/home \
TURSO_URL=file:/tmp/remove-llm-caller-e2e.nZRDcq/paw.db \
PORT=61185 \
PUBLIC_BASE_URL=http://127.0.0.1:61185 \
OTEL_ENABLED=false \
TEMPER_API_KEY=proof-temper-key \
PAW_TENANT=default \
TEMPERPAW_WASM_STARTUP_POLICY=load-only \
./target/release/temperpaw-server
```

Then drove a real mock-provider Session through completion with OData:

- created Session `ss-019dc13c-026f-7b90-b974-1e5d9e10a09a`
- configured it with:
  - `provider = mock`
  - `model = mock-model`
  - `temper_api_url = http://127.0.0.1:61185`
  - `user_message = {"steps":[{"final_text":"remove-llm-caller staged turn e2e complete"}]}`
- waited on:
  - `/observe/entities/Session/ss-019dc13c-026f-7b90-b974-1e5d9e10a09a/wait?statuses=Completed,Failed,Cancelled&timeout_ms=60000&poll_ms=500`

Observed final status:

```text
Completed
result = remove-llm-caller staged turn e2e complete
```

Observed staged Session history:

- `WorkspaceReady` -> `PreparingContext`
- `ContextReady` -> `CallingProvider`
- `ProviderResponseReady` -> `ApplyingProviderResponse`
- `CheckSteering` -> `Steering`
- `FinalizeResult` -> `Completed`

Observed staged artifacts on the live Session:

- `prepared_context_file_id = fl-019dc13c-033a-7011-b26f-254a5245550a`
- `provider_response_file_id = fl-019dc13c-038c-7111-b58b-29e4178f73e6`
- `prepared_context_bytes = 53287`
- `prepared_context_entries_loaded = 1`
- `prepared_context_content_files_loaded = 1`
- `provider_request_bytes = 0`
- `provider_response_bytes = 42`

This is the required live local E2E proof for the cutover: the new Session turn
path completed end-to-end with no `llm_caller` integration present.

## Files of Interest

- `docs/adrs/0040-remove-llm-caller-and-make-staged-turn-wasms-authoritative.md`
- `os-apps/paw-agent/specs/session.ioa.toml`
- `os-apps/paw-agent/policies/session.cedar`
- `os-apps/paw-agent/tests/staged_turn_cutover.sh`
- `os-apps/paw-agent/wasm/context_preparer/`
- `os-apps/paw-agent/wasm/provider_caller/`
- `os-apps/paw-agent/wasm/provider_response_applier/`
- `os-apps/paw-agent/wasm/tool-catalog/`
- `os-apps/paw-agent/wasm/session-turn-artifacts/`

## Limits

- The staged WASMs are now the authoritative deployable owners, but some internal helper code is still duplicated across those crates after the one-shot cutover.
- The successful live E2E used the deterministic `mock` provider. It proves the
  staged Session orchestration and artifact flow, not Anthropic/OpenAI/OpenRouter
  behavior.
- The isolated runtime proof surfaced unrelated `paw-fs` artifact expectations.
  Those were fixed locally for proofing by building `blob_adapter` and
  `workspace_fs`, but this branch does not otherwise redesign `paw-fs` startup.
