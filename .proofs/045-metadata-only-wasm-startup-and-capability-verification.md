# 045: Metadata-Only WASM Startup Restore And Capability Verification

Date: 2026-04-14

## Goal

Finish the baseline-memory reduction pass without breaking app or agent capability:

- startup should restore only persisted WASM metadata, not bulk `wasm_bytes`
- persisted modules should still lazy-compile on first use
- core startup apps and system skills should still be available
- manual app installation should still work end to end

## Code Changes

### Metadata-only startup restore

Temper startup recovery now restores only module registry metadata and hashes:

- `temper-store-turso`: added `load_wasm_module_metadata_all_tenants()`
- `temper-server`: `load_wasm_modules()` now bulk-loads only `tenant`, `module_name`, `sha256_hash`, and metadata
- legacy rows with missing hashes fall back to a per-module byte fetch to recover the hash, instead of bulk-loading all bytes
- first-use execution still compiles lazily through `ensure_wasm_module_cached(...)`

Relevant files:

- `/Users/seshendranalla/Development/temper/crates/temper-store-turso/src/store/wasm.rs`
- `/Users/seshendranalla/Development/temper/crates/temper-store-turso/src/store/tests.rs`
- `/Users/seshendranalla/Development/temper/crates/temper-server/src/state/persistence/mod.rs`
- `/Users/seshendranalla/Development/temper/crates/temper-server/tests/wasm_dispatch.rs`

### Startup-surface reduction already in effect

OpenPaw boots only core apps by default, and app manifests drive eager vs lazy module treatment:

- core startup apps: `paw-agent`, `paw-channels`, `paw-fs`
- manual-install apps remain available in the catalog and installable later

Relevant files:

- `/Users/seshendranalla/Development/openpaw-codex/crates/openpaw/src/startup.rs`
- `/Users/seshendranalla/Development/openpaw-codex/os-apps/paw-agent/app.toml`
- `/Users/seshendranalla/Development/openpaw-codex/os-apps/paw-channels/app.toml`
- `/Users/seshendranalla/Development/openpaw-codex/os-apps/paw-fs/app.toml`

## Red-Green Verification

### New tests

1. `temper-store-turso`

- `load_wasm_module_metadata_all_tenants_returns_metadata_without_bulk_bytes`
- proves startup metadata restore can enumerate persisted modules without bulk-loading `wasm_bytes`

2. `temper-server`

- `persisted_wasm_modules_with_missing_hash_still_execute_after_startup_restore`
- proves legacy rows without `sha256_hash` still register correctly at startup and lazy-compile on first invoke

### Test results

Passed:

```bash
cargo test -p temper-store-turso load_wasm_module_metadata_all_tenants_returns_metadata_without_bulk_bytes -- --nocapture
cargo test -p temper-server --test wasm_dispatch persisted_wasm_modules_with_missing_hash_still_execute_after_startup_restore -- --nocapture
cargo test -p temper-server --test wasm_dispatch -- --nocapture
cargo test --workspace -- --nocapture   # openpaw-codex
cargo build -p openpaw --release
```

Also fixed the stale `temper-wasm` default-memory expectation in:

- `/Users/seshendranalla/Development/temper/crates/temper-wasm/src/engine/tests.rs`

## End-to-End Runtime Verification

### Isolated server

Started one isolated release OpenPaw instance:

- home: `/var/folders/6m/lm283ng13931_42z4z8n1x7c0000gn/T/openpaw-capability-check.ww2k9_fe`
- port: `57076`
- `OPENPAW_WASM_STARTUP_POLICY=load-only`
- `TEMPER_RUNTIME_METRICS_INTERVAL_SECS=2`
- `TEMPER_ACTOR_IDLE_TIMEOUT=20`
- `TEMPER_PASSIVATION_CHECK_INTERVAL=5`
- `OTEL_ENABLED=true`
- `DD_ENV=isolated-capability-check`

### Server health

`GET /healthz` returned `200`.

`GET /observe/health` returned:

```json
{
  "status": "healthy",
  "uptime_seconds": 124,
  "specs_loaded": 38,
  "active_actors": 1,
  "indexed_entities": 104,
  "transitions_total": 269,
  "errors_total": 0,
  "event_store": "turso"
}
```

### Core capability: system skills still load

Queried the exact skill-file surface agents rely on:

```http
GET /tdata/Files?$filter=startswith(path,'/system/skills/') and name eq 'SKILL.md' and Status ne 'Archived'
```

Observed live skill files including:

- `/system/skills/platform-awareness/SKILL.md`
- `/system/skills/proactive-tool-use/SKILL.md`
- `/system/skills/research-first-planning/SKILL.md`
- `/system/skills/temper-app-creation/SKILL.md`

Read one skill body directly:

```http
GET /tdata/Files('os-sys-skill-file-platform-awareness')/$value
```

Returned `200` and the file content began with:

```text
---
name: platform-awareness
---
# Platform Awareness
```

This confirms core skills still bootstrap into TemperFS and remain readable through the API paths agents use.

### App catalog: manual apps still visible

`GET /api/os-apps` returned `17` apps, including manual-install apps such as:

- `paw-research`
- `paw-pm`
- `paw-ingest`
- `paw-compute`
- `paw-research`
- `dsf-harness`
- `dsf-team`

### Manual app install still works

Installed `paw-research` at runtime:

```http
POST /api/os-apps/paw-research/install
Authorization: Bearer benchmark-secret
Content-Type: application/json

{"tenant":"default"}
```

Response:

```json
{"app":"paw-research","tenant":"default","added":["WebQuery"],"updated":[],"skipped":[],"status":"installed"}
```

The tenant install index contained:

- `default|paw-agent`
- `default|paw-channels`
- `default|paw-fs`
- `default|paw-research`

### Installed app is actually usable

Confirmed the entity set exists after install:

```http
GET /tdata/WebQueries
```

Returned `200` with `@odata.context: "$metadata#WebQueries"`.

Confirmed the app guide was materialized into TemperFS:

```http
GET /tdata/Files('os-app-guide-paw-research')/$value
```

Returned `200` and the body began with:

```text
# paw-research
```

### Installed app action flow still works with lazy compile

Executed the same `monty_repl` research flow the platform uses:

1. Create entity

```http
POST /tdata/WebQueries
{"QueryType":"search","Query":"rust async patterns","Url":""}
```

2. Execute action

```http
POST /tdata/WebQueries('en-019d8c39-3b39-75b0-a056-baec99074dab')/Temper.ExecuteSearch?await_integration=true
{"query":"rust async patterns"}
```

3. Read entity back

Observed transitions:

- `Created`
- `ExecuteSearch`
- `RecordError`

Final entity state:

```json
{
  "status": "Failed",
  "fields": {
    "Status": "Failed",
    "error": "web_search: missing exa_api_key secret. Configure EXA_API_KEY."
  }
}
```

This is the expected behavior in the isolated environment because no Exa secret was configured. The important part is that the installed app entity, bound action, WASM trigger, and callback path all executed correctly.

The server log confirms lazy compilation on first use:

```text
temper_wasm::engine: WASM module compiled and cached ... module=web_search
temper_server::state::persistence: lazy-compiled persisted WASM module on first use tenant=default module=web_search ...
temper_server::state::dispatch::wasm: invoking WASM integration module tenant=default entity_type="WebQuery" ...
temper_server::entity_actor::effects: event emitted entity_type=WebQuery ... event=RecordError
```

## Memory / Module Snapshot

### Live module corpus

Persisted module count in the isolated DB:

```text
WASM_MODULE_COUNT 28
default|monty_repl|6711219
default|llm_caller|615736
default|route_message|376656
default|context_compactor|337760
default|steering_checker|325749
default|plan_review_feedback_handler|321825
default|request_approval|267740
default|request_plan_review|265033
default|web_fetch|258595
default|session_recoverer|254951
```

### Process memory

After boot plus app install plus installed-app execution:

```text
PID=88224
RSS=255184 KB
Physical footprint=90.0M
vmmap TOTAL resident=607.4M
vmmap TOTAL physical=90.0M
```

This is consistent with the reduced baseline established in proof 044 and confirms the metadata-only restore path did not regress the memory floor.

## Conclusion

The optimization held without shutting off important capabilities:

- startup no longer bulk-loads persisted `wasm_bytes`
- persisted modules still execute correctly via lazy compile on first use
- core system skills still bootstrap and remain readable
- manual-install apps still appear in the catalog
- manual-install apps still install correctly
- installed app entity sets and action-triggered WASM flows still work end to end

The remaining baseline-memory story is the persisted module corpus itself, not runaway actor hydration and not eager startup compilation.
