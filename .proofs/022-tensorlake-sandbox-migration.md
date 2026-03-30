# Proof Report: 022 — Tensorlake Sandbox Migration

## Date
2026-03-30

## Branch / Commit
`feat/openpaw-self-heal-loop-codex`

## What Was Done

Replaced Modal with Tensorlake as the sandbox provider for OpenPaw agent execution. This eliminates the Python bridge layer entirely and has WASM modules call Tensorlake's REST API directly.

### Files Changed
- `crates/openpaw/src/config.rs` — removed `modal_token_id`/`modal_token_secret`, added `tensorlake_api_key`
- `crates/openpaw/src/startup.rs` — removed `launch_modal_bridge_url()`, `launch_local_sandbox_url()`, simplified sandbox URL logic
- `os-apps/paw-agent/wasm/sandbox_provisioner/src/lib.rs` — rewritten to call `POST api.tensorlake.ai/sandboxes` directly
- `os-apps/paw-agent/wasm/tool_runner/src/lib.rs` — file ops use `/api/v1/files`, process exec uses output-redirection polling
- `os-apps/paw-agent/wasm/tool_runner/src/entity_tools.rs` — `run_coding_agent` uses shared `run_bash_local`
- `os-apps/paw-agent/wasm/workspace_restorer/src/lib.rs` — file write uses Tensorlake API
- `os-apps/paw-agent/wasm/coding_agent_runner/src/lib.rs` — process exec uses Tensorlake API
- `os-apps/paw-agent/specs/agent.ioa.toml` — `modal_bridge_url` → `tensorlake_api_key` in integration configs
- `os-apps/paw-compute/specs/computer.ioa.toml` — default provider → `tensorlake`
- `.env` — removed `MODAL_TOKEN_ID`/`MODAL_TOKEN_SECRET`
- `README.md` — updated env var docs

### Files Deleted
- `os-apps/paw-agent/sandbox/modal_sandbox.py` — Python bridge (eliminated)
- `os-apps/paw-agent/sandbox/local_sandbox.py` — in-container HTTP server (eliminated)
- `scripts/openpaw_modal_sandbox.py` — convenience launcher (eliminated)

### Files Created
- `docs/adrs/0007-tensorlake-sandbox-migration.md` — architecture decision record

## Architecture Diagram

```text
BEFORE (Modal) — 4 files, Python bridge required
=================================================

  sandbox_provisioner.wasm ──HTTP──> modal_sandbox.py ──SDK──> Modal Cloud
                                     (Python bridge)
                                           |
                                           v
  tool_runner.wasm ──HTTP──> Modal container running local_sandbox.py
                             (custom HTTP server)


AFTER (Tensorlake) — 0 Python files, pure WASM
=================================================

  sandbox_provisioner.wasm ──HTTP──> api.tensorlake.ai/sandboxes
                                     (Tensorlake REST API)
                                           |
                                           v
  tool_runner.wasm ──HTTP──> <id>.sandbox.tensorlake.ai/api/v1/...
                             (native Tensorlake data plane)
```

## Verification Flow

### Build Verification

1. `cargo check --workspace` — Rust workspace compiles
2. `cd os-apps/paw-agent/wasm && bash build.sh` — all 13 WASM modules compile

### Code Verification

3. No remaining Modal references in code (only in historical proofs and ADR-0007 context)
4. Cedar policies unchanged — `sandbox_provisioner` and `tool_runner` already authorized for `http_call` and `access_secret`
5. Integration configs in `agent.ioa.toml` correctly reference `{secret:tensorlake_api_key}`

## Verification Results

| Step | Expected | Actual | Status |
|------|----------|--------|--------|
| Cargo check | Compiles clean | 1 pre-existing warning (unused field), no errors | PASS |
| WASM build | All 13 modules compile | All 13 built successfully | PASS |
| No Modal refs in code | Zero in .rs/.toml/.py | Zero (only in .proofs/ and ADR docs) | PASS |
| Config: TL_API_KEY loads | Field present in Config struct | `tensorlake_api_key: optional_env("TL_API_KEY")` | PASS |
| Startup: no bridge launch | No Python subprocess spawning | `launch_modal_bridge_url` and `launch_local_sandbox_url` deleted | PASS |
| Provisioner: Tensorlake API | POST to api.tensorlake.ai | Implemented with health polling | PASS |
| Tool runner: Tensorlake data plane | /api/v1/files and /api/v1/processes | Implemented with output-redirection polling | PASS |
| ADR-0007 written | Documents rationale and design | Created with full context | PASS |
| Runtime E2E (agent loop) | Agent provisions + executes tools | NOT YET TESTED — requires running OpenPaw with live TL_API_KEY | PENDING |

## What Worked

- Clean separation: all sandbox HTTP calls route through 3 helper functions (`read_file_local`, `write_file_local`, `run_bash_local`)
- Output-redirection polling pattern is portable — works regardless of whether the sandbox supports SSE
- Single API key auth is simpler than Modal's dual-credential model
- `coding_agent_runner` and `entity_tools::run_coding_agent` both delegate to shared `run_bash_local`

## What Didn't Work

- (none observed at compile/static level)

## Limitations

- WASM has no `sleep()` — polling loop makes rapid HTTP requests (mitigated by network latency as natural backoff)
- Process output is not streamed — entire stdout/stderr is read after process completion
- Max practical command timeout is ~300s (600 polling iterations)

## What Still Doesn't Work

- **Runtime E2E not yet tested**: Need to start OpenPaw with live `TL_API_KEY` and trigger an agent to verify the full Provision → SandboxReady → Thinking → Executing loop against real Tensorlake infrastructure
- **Tensorlake API response format**: The exact JSON shape of `POST /sandboxes` response needs validation against real API (field may be `id` vs `sandbox_id`)

## Artifacts

- ADR: `docs/adrs/0007-tensorlake-sandbox-migration.md`
- Updated ADR: `docs/adrs/0004-platform-upgrade-sre-modal-datadog.md` (superseded note on section 5)
