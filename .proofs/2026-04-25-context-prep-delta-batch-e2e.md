# Context Preparer Delta/Batch Live E2E Proof

Date: 2026-04-25

## Scope

Proves the merged active `context_preparer` path uses the prepared-context reuse artifact and TemperFS batch version reads for a channel continuation.

The local `tpaw` CLI was not installed in this environment, so the equivalent repo-built server binary was used:

```sh
cargo build -p temperpaw
target/debug/temperpaw-server
```

The live server was started from a clean temp DB with a temp `os-apps` root containing only the Paw apps required for this flow (`paw-fs`, `paw-agent`, `paw-research`, `paw-channels`). This avoids the unrelated `katagami-curation` legacy `reactions.toml` startup failure while still exercising the real Paw channel and session WASM modules.

## Local Server

```sh
env HOME=/tmp/openpaw-context-prep-e2e-home \
  PORT=4577 \
  TURSO_URL=file:/tmp/openpaw-context-prep-e2e.db \
  TEMPER_API_KEY=live-e2e-key \
  LLM_PROVIDER=mock \
  LLM_MODEL=mock \
  OTEL_ENABLED=false \
  RUST_LOG=info,temperpaw=debug,context_preparer=debug,temper_server::api::files=debug,temper_server::state::file_reads=debug \
  TEMPERPAW_WASM_STARTUP_POLICY=build \
  target/debug/temperpaw-server
```

Ready check:

```text
HTTP/1.1 200 OK
{"status":"ready","healthz":"/healthz","discord":{"status":"disconnected","configured":false,"connected":false}}
```

Startup log evidence:

```text
Temper Paw listening on port 4577
startup: time to ready elapsed_ms=9993 tenant=default
```

## E2E Driver

The driver created:

- one persistent mock `Agent`
- one webhook `Channel`
- one channel-scoped `AgentRoute` bound with `agent_id`
- first `Paw.Channel.ReceiveMessage` containing a mock plan
- second `Paw.Channel.ReceiveMessage` on the same thread with a 12 KB delta message

Result:

```json
{
  "agent_id": "aj-019dc444-f28d-78b3-aae8-53d24a84a340",
  "base_url": "http://127.0.0.1:4577",
  "batch_version_read_seen": true,
  "channel_id": "en-019dc444-f29e-7582-b95b-a5282d456345",
  "channel_name": "context-delta-proof-1777114346",
  "channel_session_id": "en-019dc444-f3ee-7522-94d0-2cda4bfcbf7c",
  "first_entries_loaded": 1,
  "first_prepared_context_file_id": "fl-019dc444-f5f1-74c1-bcb6-26e83c0a9c32",
  "first_progress_token": 5,
  "first_reply_content": "FIRST_CONTEXT_READY",
  "first_session_id": "ss-019dc444-f3df-7b91-a413-4ac98658c749",
  "parent_session_id": "ss-019dc444-f3df-7b91-a413-4ac98658c749",
  "reuse_log_seen": true,
  "route_id": "en-019dc444-f3b8-7613-b4c9-9d4ebb92d420",
  "second_content_files_loaded": 2,
  "second_entries_loaded": 3,
  "second_prepared_context_file_id": "fl-019dc444-f5f1-74c1-bcb6-26e83c0a9c32",
  "second_progress_token": 5,
  "second_reply_content": "SECOND_DELTA_READY",
  "second_session_id": "ss-019dc444-f81c-7c33-85c4-c7aee1c5e7b5",
  "session_file_id": "fl-019dc444-f469-78e3-810d-13b6c86ab1e1",
  "thread_id": "thread-context-delta-1777114346"
}
```

Assertions proven:

- first reply: `FIRST_CONTEXT_READY`
- second reply: `SECOND_DELTA_READY`
- continuation created a new Session
- continuation reused the same `session_file_id`
- continuation recorded `parent_session_id` equal to the first Session id
- continuation reused the same `prepared_context_file_id`
- continuation loaded delta context: first entries `1`, second entries `3`
- both sessions recorded `ProgressMade` (`progress_token=5`)

## Read Path Evidence

Batch immutable version read:

```text
POST /api/files/read-version-text-batch status=200
```

Reuse log from active `context_preparer`:

```text
context_preparer: reused prepared context delta_entries=2 delta_content_files=1
```

Success markers:

```text
CONTEXT_PREPARER_REUSE=true
BATCH_VERSION_READ=true
```

## Validation Commands

Implementation and follow-up guards:

```sh
cargo test -p temperpaw --test session_turn_architecture
cargo test -p temperpaw --test session_turn_architecture --locked
cargo fmt --check
cargo build --target wasm32-wasip1 --release
```

WASM artifacts rebuilt before live E2E:

```sh
bash os-apps/paw-agent/wasm/build.sh
bash os-apps/paw-channels/wasm/build.sh
```
