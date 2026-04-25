# Proof Report: 058 - Workflow Trace Context

## Date

2026-04-24

## Branch / Commit

- Branch: `codex/workflow-traces`
- Temper dependency: `97acd90fc7f0e10bc8b2624db65568adbb625571`
- OpenPaw commit: finalized by PR #123 after rebase onto current `main`

## What Was Done

- Merged the Temper platform primitive from ADR-0059 so dispatch, WASM callbacks,
  adapter callbacks, reactions, scheduled actions, and spawned child entities can
  keep one workflow trace context instead of creating per-hop trace islands.
- Updated OpenPaw's `Cargo.lock` to consume the merged Temper commit.
- Accepted ADR-0037 and linked it to Temper ADR-0059 as the owner of true
  end-to-end workflow flamegraphs.
- Rebased onto current `main`, which already includes the staged-turn cleanup
  and its CI portability fixes for the current provider-caller path.

## Verification Flow

All OpenPaw Cargo commands were run from `/tmp` with `--manifest-path` so the
parent checkout's local `.cargo/config.toml` Temper patch did not mask the Git
dependency.

```sh
cargo fmt --all -- --check
```

```sh
cargo clippy \
  --manifest-path /Users/seshendranalla/Development/openpaw/.worktrees/openpaw-workflow-traces/Cargo.toml \
  -p temperpaw \
  --all-targets \
  -- -D warnings
```

```sh
cargo test \
  --manifest-path /Users/seshendranalla/Development/openpaw/.worktrees/openpaw-workflow-traces/Cargo.toml \
  -p temperpaw \
  --quiet
```

```sh
cargo test \
  --manifest-path /Users/seshendranalla/Development/openpaw/.worktrees/openpaw-workflow-traces/Cargo.toml \
  -p paw-transport \
  paw_api_client_includes_traceparent_from_active_span
```

```sh
cargo test \
  --manifest-path /Users/seshendranalla/Development/openpaw/.worktrees/openpaw-workflow-traces/Cargo.toml \
  -p temperpaw \
  local_wasm_policy_defaults_and_overrides
```

Local server boot:

```sh
rm -rf /tmp/openpaw-workflow-traces-e2e
mkdir -p /tmp/openpaw-workflow-traces-e2e

HOME=/tmp/openpaw-workflow-traces-e2e/home \
PORT=4489 \
PUBLIC_BASE_URL=http://127.0.0.1:4489 \
OTEL_ENABLED=false \
TEMPER_API_KEY=workflow-trace-e2e-key \
TEMPERPAW_WASM_STARTUP_POLICY=load-only \
PAW_TENANT=default \
TURSO_URL=file:/tmp/openpaw-workflow-traces-e2e/paw.db \
RUST_LOG=info \
./target/debug/temperpaw-server
```

Local E2E action dispatch under an explicit W3C traceparent:

```sh
BASE=http://127.0.0.1:4489
AUTH='Authorization: Bearer workflow-trace-e2e-key'
TENANT='x-tenant-id: default'
TRACE='traceparent: 00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01'

curl -fsS -X POST "$BASE/tdata/Agents" \
  -H "$AUTH" -H "$TENANT" -H "$TRACE" -H 'content-type: application/json' \
  -d '{"Id":"ag-e2e-workflow-trace"}'

curl -fsS -X POST "$BASE/tdata/Agents('\''ag-e2e-workflow-trace'\'')/TemperPaw.Configure" \
  -H "$AUTH" -H "$TENANT" -H "$TRACE" -H 'content-type: application/json' \
  -d '{"name":"Workflow Trace E2E","role":"proof","description":"verifies dispatch workflow trace context via OpenPaw","model":"mock","provider":"mock","tools_enabled":"false","max_turns":"1"}'

curl -fsS "$BASE/tdata/Agents('\''ag-e2e-workflow-trace'\'')" \
  -H "$AUTH" -H "$TENANT"
```

State verification:

```sh
sqlite3 -header -column /tmp/openpaw-workflow-traces-e2e/paw.db \
  "SELECT tenant, entity_type, entity_id, action, success, from_status, to_status
   FROM trajectories
   WHERE entity_type='Agent' AND entity_id='ag-e2e-workflow-trace'
   ORDER BY created_at;
   SELECT field_name, field_value, status
   FROM entity_field_index
   WHERE tenant='default'
     AND entity_type='Agent'
     AND entity_id='ag-e2e-workflow-trace'
     AND field_name IN ('Status','name','provider','model')
   ORDER BY field_name;"
```

## Verification Results

| Step | Expected | Actual | Status |
|------|----------|--------|--------|
| `cargo fmt --all -- --check` | Rust formatting matches repo style | Passed locally | Pass |
| `cargo clippy -p temperpaw --all-targets -- -D warnings` | OpenPaw clippy gate passes against Temper Git dependency | Passed locally | Pass |
| `cargo test -p temperpaw --quiet` | OpenPaw server tests pass against the merged Temper dependency | Passed locally after rebasing onto current `main` | Pass |
| `paw_api_client_includes_traceparent_from_active_span` | HTTP client forwards active span `traceparent` | Passed | Pass |
| `local_wasm_policy_defaults_and_overrides` | OpenPaw compiles against Temper Git dependency and startup policy test passes | Passed | Pass |
| Server boot | `temperpaw-server` starts with merged Temper dependency | `/healthz` responded successfully | Pass |
| E2E dispatch | `Agent.Configure` completes via OData action | Entity reached `Active` with expected fields | Pass |
| Dispatch trace attributes | Dispatch span includes workflow root attributes | Logs showed `workflow.root_entity_type="Agent"`, `workflow.root_entity_id="ag-e2e-workflow-trace"`, `workflow.run_id="Agent:ag-e2e-workflow-trace"` | Pass |

## What Worked

- OpenPaw builds and runs against Temper `97acd90fc7f0e10bc8b2624db65568adbb625571`.
- A local OData action dispatch under an explicit `traceparent` preserved the
  dispatch workflow root attributes.
- The state transition is inspectable from Temper trajectories alone:

```text
tenant   entity_type  entity_id              action     success  from_status  to_status
default  Agent        ag-e2e-workflow-trace  Configure  1        Created      Active
```

```text
field_name  field_value         status
Status      Active              Active
model       mock                Active
name        Workflow Trace E2E  Active
provider    mock                Active
```

## What Didn't Work

- `TEMPERPAW_WASM_STARTUP_POLICY=load-only` emitted expected missing-artifact
  warnings for local WASM modules that were not built for this proof run.

## Limitations

- This proof intentionally used `OTEL_ENABLED=false`, so it validates local span
  construction and entity state, not ingestion into live Datadog.
- The E2E action was `Agent.Configure`, which does not require local WASM
  artifacts. Full live Datadog validation should be done in an environment with
  OTEL export enabled and a workflow that exercises WASM callbacks.

## What Still Doesn't Work

- This change does not paper over workflow-specific publish bugs. It makes the
  missing hop observable in the same flamegraph so failures such as skipped
  `DesignLanguages.UpdateQuality` or `DesignLanguages.Publish` can be traced to
  their exact dispatch boundary.

## Artifacts

- Temper ADR: `docs/adrs/0059-workflow-trace-context-propagation.md` in
  `nerdsane/temper`
- OpenPaw ADR: `docs/adrs/0037-end-to-end-tracing-and-traceparent-propagation.md`
- Local database: `/tmp/openpaw-workflow-traces-e2e/paw.db`
- Local server port: `4489` during verification

## Architecture Diagram

```text
external trigger / HTTP request
        |
        v
Temper dispatch root span
  workflow.root_entity_type
  workflow.root_entity_id
  workflow.run_id
        |
        v
entity action trajectory
        |
        +--> WASM callback span
        |
        +--> adapter callback span
        |
        +--> reaction / scheduled action span
        |
        +--> spawned child entity dispatch span
        |
        v
Datadog trace tree for the workflow
```
