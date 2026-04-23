# Durable Dispatch Effects And Progress Boundaries Proof

Date: 2026-04-23

## Scope

This proof covers the fix for the dispatch retry/idempotency race in Temper and
the OpenPaw rollback of timeout-band-aid values after adding real progress
signals around long-running provider/tool boundaries.

Worktrees:

- Temper: `/Users/seshendranalla/Development/temper-worktrees/durable-dispatch-effects`
- OpenPaw: `/Users/seshendranalla/Development/openpaw/.worktrees/durable-dispatch-effects`

## Regression

The Temper regression test `dispatch_retry_idempotency.rs` models:

1. Actor persists `Start`, transitioning `TimedTask` from `Idle` to `Running`.
2. The caller's first ask times out before the actor reply is observed.
3. The dispatch retry lands after the entity is already `Running`.
4. The retry must replay the cached successful response and run post-dispatch
   effects, including arming the `Running` state timeout.

Before the fix, this failed with:

```text
retry should return the cached successful Start response, got Some("Action 'Start' not valid from state 'Running'")
```

After the fix, the same test passes and observes one pending state timeout for
`TimedTask`.

A follow-up tightening adds effect-aware idempotency cache state:

- Actor-side retries can still replay a cached successful transition response.
- HTTP/OData fast-path cache hits only return after post-dispatch effects are
  marked applied.
- If a caller retries after the actor persisted state but before effects were
  confirmed, the request re-enters dispatch so effects can fire.

## Commands

Temper:

```sh
cargo test -p temper-server --test dispatch_retry_idempotency -- --nocapture
cargo test -p temper-server effects -- --nocapture
cargo test -p temper-server attempt_budget_floor_prevents_per_attempt_timeout_from_disabling_retry -- --nocapture
cargo test -p temper-server idempotency_actor_key_matches_actor_persistence_id_shape -- --nocapture
```

Results:

- `dispatch_retry_idempotency`: 1 passed.
- effect-aware idempotency cache units under `effects` filter: passed,
  including `pending_effects_do_not_satisfy_protocol_cache_hit` and
  `put_effects_applied_satisfies_protocol_cache_hit`.
- retry policy floor unit: 1 passed.
- OData idempotency actor-key unit: 1 passed.

OpenPaw WASM helpers:

```sh
cargo test --manifest-path os-apps/paw-agent/wasm/monty_repl/Cargo.toml tool_progress_wrapper -- --nocapture
cargo test --manifest-path os-apps/paw-agent/wasm/llm_caller/Cargo.toml provider_progress_wrapper -- --nocapture
```

Results:

- Monty tool progress wrapper: 2 passed.
- Provider progress wrapper: 2 passed.

OpenPaw daemon:

```sh
cargo test -p temperpaw -- --nocapture
```

Result:

- `temperpaw`: 36 unit tests passed.
- `session_turn_architecture`: 4 integration tests passed.

Runtime boot:

```sh
find /tmp -maxdepth 1 -name 'temperpaw-durable-dispatch-effects.db*' -delete
mkdir -p /tmp/temperpaw-durable-dispatch-effects-home
HOME=/tmp/temperpaw-durable-dispatch-effects-home \
PORT=4473 \
TURSO_URL=file:/tmp/temperpaw-durable-dispatch-effects.db \
RUST_LOG=info \
TEMPERPAW_WASM_STARTUP_POLICY=warn \
./target/debug/temperpaw-server
```

Observed:

```sh
curl -s -o /tmp/temperpaw-healthz.out -w '%{http_code}' http://127.0.0.1:4473/healthz
# 200

curl -sS -w '\nHTTP %{http_code}\n' http://127.0.0.1:4473/paw/setup/status
# HTTP 200
```

The runtime log showed entity transitions during bootstrap, including Agent
`Configure` actions reaching `Active`.

## Built WASM Artifacts

The earlier local boot caveat about missing `blob_adapter` / `workspace_fs`
artifacts was resolved by building the WASM modules before the live E2E:

```sh
bash os-apps/paw-fs/wasm/blob_adapter/build.sh
bash os-apps/paw-fs/wasm/workspace_fs/build.sh
bash os-apps/paw-agent/wasm/build.sh
```

Built artifacts observed:

- `os-apps/paw-fs/wasm/blob_adapter.wasm` (64K)
- `os-apps/paw-fs/wasm/workspace_fs.wasm` (255K)
- `os-apps/paw-agent/wasm/monty_repl/monty_repl.wasm` (6.2M)
- `os-apps/paw-agent/wasm/llm_caller/llm_caller.wasm` (656K)
- `os-apps/paw-agent/wasm/provider_caller/provider_caller.wasm` (496K)
- `os-apps/paw-agent/wasm/provider_response_applier/provider_response_applier.wasm` (397K)
- all other `paw-agent` WASM modules from `os-apps/paw-agent/wasm/build.sh`

Fresh server startup then registered `paw-fs` with:

```text
wasm=["blob_adapter", "workspace_fs"]
```

and registered `paw-agent` with the expected provider/toolchain modules,
including `llm_caller`, `provider_caller`, `provider_response_applier`,
`monty_repl`, `workspace_provisioner`, `agent_reply`, and
`emit_ots_trajectory`.

## Live E2E Replay

Follow-up live run on port `4474`:

```sh
HOME=/tmp/openpaw-live-e2e-home \
PORT=4474 \
TURSO_URL=file:/tmp/openpaw-live-e2e.db \
TEMPER_API_KEY=live-e2e-key \
RUST_LOG=info \
TEMPERPAW_WASM_STARTUP_POLICY=warn \
./target/debug/temperpaw-server
```

Health:

```sh
curl -s -o /tmp/openpaw-live-health.out -w '%{http_code}' http://127.0.0.1:4474/healthz
# 200
```

OData API auth:

```sh
curl -H 'Authorization: Bearer live-e2e-key' \
  'http://127.0.0.1:4474/tdata/Agents?$top=1'
# HTTP 200
```

Dispatch replay:

1. Created a fresh `Agent`; server assigned entity id
   `aj-019db9b5-bfaa-7a12-a740-51616e4a0000`.
2. Dispatched `TemperPaw.Configure` with
   `Idempotency-Key: live-e2e-configure-1`; result `HTTP 200`, status `Active`.
3. Replayed the same `TemperPaw.Configure` with the same idempotency key; result
   `HTTP 200`, status `Active`, same two-event history.
4. Negative control: dispatched `TemperPaw.Configure` again with a different
   idempotency key; result `HTTP 409` with
   `Action 'Configure' not valid from state 'Active'`.

Final entity state:

```json
{
  "entity_id": "aj-019db9b5-bfaa-7a12-a740-51616e4a0000",
  "status": "Active",
  "total_event_count": 2,
  "events": ["Created", "Configure"],
  "fields": {
    "name": "Live E2E Agent",
    "role": "qa"
  }
}
```

This exercises the live HTTP/OData idempotency cache path fixed in Temper:
same key replays the cached successful response, different key reaches the
actor in `Active` and correctly fails.

## Live E2E Replay After Effect-Aware Cache Tightening

Rebuilt OpenPaw after the effect-aware idempotency cache change:

```sh
cargo build -p temperpaw
```

Started a fresh server:

```sh
HOME=/tmp/openpaw-live-e2e-v2-home \
PORT=4474 \
TURSO_URL=file:/tmp/openpaw-live-e2e-v2.db \
TEMPER_API_KEY=live-e2e-key \
RUST_LOG=info \
TEMPERPAW_WASM_STARTUP_POLICY=warn \
./target/debug/temperpaw-server
```

Observed live results:

```text
healthz=200
create_http=201 entity=aj-019db9bc-1763-7360-a376-81673ecd3ef8 status=Created events=1
configure_1_http=200 status=Active events=2 name=Live E2E V2 Agent
configure_2_http=200 status=Active events=2 name=Live E2E V2 Agent
negative_http=409 error=Action 'Configure' not valid from state 'Active'
final={"status":"Active","total_event_count":2,"events":["Created","Configure"],"name":"Live E2E V2 Agent"}
```

This rerun confirms the rebuilt live server still returns cached success only
for the same idempotency key and still rejects a distinct duplicate action from
the advanced state.

## Full Live Session E2E With Built WASM

Started a fresh server with built WASM artifacts:

```sh
HOME=/tmp/openpaw-full-e2e-home \
PORT=4474 \
TURSO_URL=file:/tmp/openpaw-full-e2e.db \
TEMPER_API_KEY=live-e2e-key \
RUST_LOG=info,temper_server::state::dispatch=debug,llm_caller=debug,monty_repl=debug,temperpaw_server::startup=info \
TEMPERPAW_WASM_STARTUP_POLICY=warn \
./target/debug/temperpaw-server
```

Health:

```text
healthz 200
```

Direct TemperFS `$value` proof, exercising the formerly missing
`blob_adapter` module:

```text
file_create 201 entity=fl-019db9c6-c4fb-7763-a3c2-c6cc98ce8282
file_put_value 204
file_get_value 200 blob-adapter-live-e2e
```

Then created and configured a live `Session` using the mock provider with a
deterministic tool-call plan:

- Provider returned one `execute` tool call.
- `monty_repl` executed the code.
- The tool called `temper.done("live e2e complete via Monty tool")`.
- The session reached `Completed`.

Observed polling:

```text
session_poll 1 status=Provisioning events=3
session_poll 2 status=PreparingContext events=4
session_poll 4 status=ApplyingProviderResponse events=9 progress_token=2
session_poll 5 status=Executing events=10 progress_token=2
session_poll 10 status=Executing events=11 progress_token=3
session_poll 11 status=Completed events=15 progress_token=5
```

Final state:

```json
{
  "session_id": "ss-019db9c6-c571-73f1-8a02-6e9232a83c8a",
  "status": "Completed",
  "result": "live e2e complete via Monty tool",
  "events": [
    "Created",
    "Configure",
    "ProvisionWorkspace",
    "WorkspaceReady",
    "ContextReady",
    "Heartbeat",
    "ProgressMade",
    "ProgressMade",
    "ProviderResponseReady",
    "ProcessToolCalls",
    "ProgressMade",
    "ProgressMade",
    "ProgressMade",
    "RecordResult",
    "MarkTrajectoryEmitted"
  ],
  "counters": {
    "context_tokens": 1,
    "input_tokens": 2,
    "output_tokens": 2,
    "progress_token": 5
  },
  "workspace_id": "ws-019db9c6-c6bf-73b0-84a9-ae3c2a73c437",
  "conversation_file_id": "fl-019db9c6-c6c9-7283-80be-6931c482d5e6",
  "file_manifest_id": "fl-019db9c6-c70d-7bf3-9c8c-8f69ecaac5f6",
  "session_file_id": "fl-019db9c6-c74f-7253-8e1c-2fe4c2f544e1",
  "session_leaf_id": "t-3"
}
```

This E2E validates the full live cascade:

- built `paw-fs` WASM is installed and handles `$value` upload/download;
- `Session.Configure` schedules `ProvisionWorkspace`;
- workspace provisioning writes TemperFS session artifacts;
- provider call emits `ProgressMade` before/after the call;
- provider response transitions to `ProcessToolCalls`;
- `monty_repl` emits `ProgressMade` around tool execution;
- post-dispatch effects run after terminal `RecordResult`, including reply
  delivery skip and OTS trajectory emission.
