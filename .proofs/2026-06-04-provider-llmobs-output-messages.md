# Provider LLMObs Output Messages Proof

Date: 2026-06-04

## Scope

Fix provider LLM spans that appeared in Datadog LLM Observability as
`No content` despite successful provider calls.

Datadog's OpenTelemetry LLMObs mapping extracts output content from direct
`gen_ai.output.messages` attributes first, then from span events named
`gen_ai.client.inference.operation.details` that carry the same attribute:

https://docs.datadoghq.com/llm_observability/instrumentation/otel_instrumentation/

## Decision Record

Added `os-apps/paw-agent/adrs/030-provider-llmobs-output-messages.md`.

Summary: `provider_caller` keeps legacy `gen_ai.completion`, and also emits
structured `gen_ai.output.messages` as a span attribute and on a
`gen_ai.client.inference.operation.details` event.

## Red

Command:

```bash
cargo test --manifest-path os-apps/paw-agent/wasm/provider_caller/Cargo.toml llm_success_span_attrs_include_datadog_output_messages -- --nocapture
```

Observed before implementation:

```text
error[E0425]: cannot find function `llm_success_span_attributes` in this scope
```

This proved the new Datadog output-message contract did not yet exist.

## Green

Commands:

```bash
cargo fmt --manifest-path os-apps/paw-agent/wasm/provider_caller/Cargo.toml
cargo test --manifest-path os-apps/paw-agent/wasm/provider_caller/Cargo.toml llm_success_span_attrs_include_datadog_output_messages -- --nocapture
cargo test --manifest-path os-apps/paw-agent/wasm/provider_caller/Cargo.toml -- --nocapture
cargo test -p temperpaw --test datadog_observability_contract temperpaw_guest_observability_api_exposes_session_tool_and_llmobs_semconv -- --nocapture
cargo build --workspace
```

Results:

```text
focused provider_caller test: pass
provider_caller suite: 27 passed
Datadog observability contract test: 1 passed
workspace build: finished dev profile successfully
```

## WASM Build

Required app WASM modules were built before the local server boot:

```bash
bash os-apps/paw-fs/wasm/blob_adapter/build.sh
bash os-apps/paw-fs/wasm/workspace_fs/build.sh
bash os-apps/paw-agent/wasm/build.sh
bash os-apps/paw-research/wasm/build.sh
bash os-apps/paw-channels/wasm/build.sh
bash os-apps/paw-ingest/wasm/build.sh
bash os-apps/paw-patrol/wasm/build.sh
bash os-apps/paw-skills/wasm/build.sh
```

All completed successfully.

## End-to-End

Started the server from the fresh main worktree with an isolated local store:

```bash
HOME=/tmp/temperpaw-llmobs-e2e-home \
PORT=4567 \
PAW_TENANT=llmobs_e2e \
TURSO_URL=file:/tmp/temperpaw-llmobs-e2e-data/paw.db \
TEMPER_EVENT_STORE=turso \
TEMPER_PLATFORM_STORE=turso \
TEMPER_QUERY_PROJECTION_STORE=turso \
LLM_PROVIDER=mock \
LLM_MODEL=mock-fast \
OTEL_ENABLED=false \
TEMPERPAW_ORPHANED_SESSION_RECOVERY=false \
RUST_LOG=info,temperpaw=debug \
target/debug/temperpaw-server
```

Readiness:

```text
GET /healthz -> 200
GET /readyz -> {"status":"ready",...}
```

Created and configured a real Session through OData:

```text
POST /tdata/Sessions
POST /tdata/Sessions('ss-llmobs-output-content-e2e')/TemperPaw.Configure
```

Configure payload used `provider=mock`, `model=mock-fast`, no tools, and a
direct reply route.

Observed final Session entity:

```json
{
  "entity_id": "ss-llmobs-output-content-e2e",
  "status": "Completed",
  "fields": {
    "provider": "mock",
    "model": "mock-fast",
    "provider_auth_status": "skipped",
    "prepared_context_inline_json": "{...}",
    "result": "LLMObs output message proof: say the content is visible.",
    "trajectory_emission_status": "emitted",
    "trajectory_id": "trj-ss-llmobs-output-content-e2e"
  },
  "processed_idempotency_keys": {
    "...:Configure:...": 2,
    "...:ProvisionWorkspace:...": 3,
    "...:WorkspaceReady:...": 4,
    "...:ContextReadyAuthSkipped:...": 5,
    "...:ProviderResponseReady:...": 6,
    "...:RecordResult:...": 7,
    "...:MarkTrajectoryEmitted:...": 8
  }
}
```

Trajectory API check:

```text
GET /api/ots/trajectories?limit=20
trajectory_id=trj-ss-llmobs-output-content-e2e
outcome=success
turn_count=1
```

The local E2E exercised the Temper-native transition path through
`provider_caller`. Live Datadog export was not attempted in this proof because
the isolated run intentionally used `OTEL_ENABLED=false` and no Datadog
credentials.

