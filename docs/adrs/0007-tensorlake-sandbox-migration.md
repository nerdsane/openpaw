# ADR-0007: Replace Modal with Tensorlake for Sandbox Provisioning

## Status

Accepted

## Context

OpenPaw's sandbox system required a Python bridge (`modal_sandbox.py`) because Modal only exposes sandbox creation through its Python SDK — there is no REST API for `Sandbox.create()` or in-container file/process operations. This forced a 4-file architecture:

1. `scripts/openpaw_modal_sandbox.py` — convenience launcher (venv management, .env loading)
2. `os-apps/paw-agent/sandbox/modal_sandbox.py` — HTTP bridge translating REST calls to Modal Python SDK
3. `os-apps/paw-agent/sandbox/local_sandbox.py` — custom HTTP server running inside Modal containers
4. `os-apps/paw-agent/wasm/sandbox_provisioner/src/lib.rs` — WASM module calling the bridge

This violates the Temper-native rule (ADR-0005): stateful orchestration should flow through entity state machines and WASM integrations, not through host-side Python processes. The bridge also added latency (extra hop), operational complexity (venv management, process daemonization), and a dual-credential model (`MODAL_TOKEN_ID` + `MODAL_TOKEN_SECRET`).

## Decision

### Replace Modal with Tensorlake

Tensorlake provides a full REST API for the entire sandbox lifecycle:

**Control Plane** (`api.tensorlake.ai`):
- `POST /sandboxes` — create a Firecracker MicroVM sandbox
- `DELETE /sandboxes/<id>` — terminate
- `POST /sandboxes/<id>/suspend` / `resume` / `snapshot`

**Data Plane** (`<id>.sandbox.tensorlake.ai`):
- `GET/PUT/DELETE /api/v1/files?path=...` — file I/O
- `GET /api/v1/files/list?path=...` — directory listing
- `POST /api/v1/processes` — start process
- `GET /api/v1/processes/<pid>/output/follow` — stream output (SSE)
- `POST /api/v1/processes/<pid>/signal` — signal process

All endpoints use Bearer token auth with a single `TL_API_KEY`.

### Process execution design: output-redirection polling

WASM `http_call` is synchronous — no SSE streaming support. Rather than streaming process output, commands are wrapped to redirect stdout/stderr to temp files:

```
(original_command) > /tmp/.paw-out-{id} 2> /tmp/.paw-err-{id}; echo $? > /tmp/.paw-rc-{id}
```

The WASM module polls `GET /api/v1/files?path=/tmp/.paw-rc-{id}` until the exit code file appears (network latency provides ~50-200ms natural backoff), then reads stdout/stderr files and cleans up.

### No local fallback

`TL_API_KEY` is required in all environments. There is no local sandbox fallback. A `SANDBOX_URL` env var override exists for testing against custom endpoints.

## Consequences

### Positive

- Eliminates 3 Python files (`modal_sandbox.py`, `local_sandbox.py`, `openpaw_modal_sandbox.py`)
- Single API key auth replaces dual-credential model
- WASM modules call Tensorlake REST directly — no bridge hop
- Gains Firecracker MicroVM benefits: sub-second cold starts, snapshot/restore, auto-suspend/resume
- Fully Temper-native: no host-side Python processes in the critical path
- Native file listing API replaces Python `os.walk` subprocess for file enumeration

### Negative

- Requires Tensorlake account and API key for all environments (no offline local dev)
- Process execution adds polling overhead vs. the old synchronous `POST /v1/processes/run`
- Younger platform than Modal — less community adoption, potential stability risk

### Supersedes

ADR-0004 section 5 ("Target Modal for governed remote sandboxes") is superseded by this decision.
