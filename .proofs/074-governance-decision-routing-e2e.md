# Governance Decision Routing E2E Proof

- Date: 2026-05-11
- Temper PRs:
  - https://github.com/nerdsane/temper/pull/221 merged at `dd80afd2b22f01c77c67a2f83dcbe52320237c98`
  - https://github.com/nerdsane/temper/pull/222 merged at `5976540461ed20b1e0e837890cf9bee66f60f617`
- TemperPaw branch: `codex/governed-agent-denials`
- TemperPaw Temper pin: `5976540461ed20b1e0e837890cf9bee66f60f617`

## Scope

Verify unified agent-facing authorization routing for governed Temper mutations:

1. A denied agent-scoped WASM management mutation creates a Temper `PendingDecision`.
2. The decision can be read through the tenant-scoped decision lookup endpoint.
3. The same decision appears in the tenant-scoped pending decision list for the owning agent/session.
4. TemperPaw Monty uses those tenant-scoped routes for `temper.get_decisions()` and `temper.poll_decision(id)`.

Discord transport was not configured in this local E2E (`discord.configured=false`), so this proof verifies the live Temper/TemperPaw decision API and session-readable routing surface, not an outbound Discord message.

## Local Verification

```console
$ cargo test
running 58 tests
test result: ok. 58 passed; 0 failed
```

Run from `os-apps/paw-agent/wasm/monty_repl`. This covered the tenant-scoped batchable decision path, tenant-scoped decision poll path, and top-level `decision_id` parser.

```console
$ cargo check --locked -p temperpaw -p paw-codex-worker
Finished `dev` profile [unoptimized + debuginfo] target(s) in 17.59s
```

```console
$ cargo test --locked -p temperpaw --quiet
test result: ok. 58 passed; 0 failed
test result: ok. 3 passed; 0 failed
test result: ok. 5 passed; 0 failed
test result: ok. 2 passed; 0 failed
test result: ok. 54 passed; 0 failed
test result: ok. 6 passed; 0 failed
test result: ok. 13 passed; 0 failed
```

## Live Server E2E

Server command:

```console
$ HOME=/tmp/temperpaw-governance-e2e-home \
  RUSTUP_HOME=/Users/seshendranalla/.rustup \
  CARGO_HOME=/Users/seshendranalla/.cargo \
  PORT=3479 \
  PUBLIC_BASE_URL=http://127.0.0.1:3479 \
  OTEL_ENABLED=false \
  TEMPER_API_KEY=governance-e2e-key \
  PAW_TENANT=default \
  TURSO_URL=file:/tmp/temperpaw-governance-e2e.db \
  TEMPER_EVENT_STORE=turso \
  TEMPER_PLATFORM_STORE=turso \
  TEMPER_QUERY_PROJECTION_STORE=turso \
  TEMPERPAW_WASM_STARTUP_POLICY=build \
  BUILD_VERSION=governance-e2e \
  BUILD_SHA=5976540461ed20b1e0e837890cf9bee66f60f617 \
  cargo run -p temperpaw --bin temperpaw-server
```

Boot result:

```json
{
  "status": "ready",
  "discord": {
    "status": "disconnected",
    "configured": false,
    "connected": false
  }
}
```

Denied mutation request:

```http
POST /api/wasm/modules/e2e_denied_module
Authorization: Bearer governance-e2e-key
x-tenant-id: default
x-temper-principal-kind: agent
x-temper-principal-id: ag-e2e
x-temper-ctx-sessionid: ss-e2e
content-type: application/json

{"wasm_base64":"AA=="}
```

Result:

```json
{
  "http_status": 403,
  "decision_id": "PD-019e1963-441f-7901-b309-ba088b8d0742",
  "error": {
    "code": "AuthorizationDenied",
    "message": "no matching permit policy Decision PD-019e1963-441f-7901-b309-ba088b8d0742"
  }
}
```

Tenant-scoped lookup:

```http
GET /api/tenants/default/decisions/PD-019e1963-441f-7901-b309-ba088b8d0742
```

Result:

```json
{
  "http_status": 200,
  "id": "PD-019e1963-441f-7901-b309-ba088b8d0742",
  "tenant": "default",
  "agent_id": "ag-e2e",
  "action": "manage_wasm",
  "resource_type": "WasmModule",
  "resource_id": "e2e_denied_module",
  "status": "pending",
  "principal_kind": "Agent",
  "session_id": "ss-e2e",
  "governance_decision_id": "GD-019e1963-4421-7990-aa6b-b154ba674127"
}
```

Owner-filtered tenant pending list:

```http
GET /api/tenants/default/decisions?status=pending
```

Result:

```json
{
  "http_status": 200,
  "list_contains_decision": true,
  "decision": {
    "id": "PD-019e1963-441f-7901-b309-ba088b8d0742",
    "agent_id": "ag-e2e",
    "session_id": "ss-e2e",
    "action": "manage_wasm",
    "resource_type": "WasmModule",
    "resource_id": "e2e_denied_module",
    "status": "pending"
  }
}
```

## Result

The previously observed dead end is closed locally: after a governed WASM mutation denial, an agent/session can inspect the created pending decision through tenant-scoped Temper APIs without cross-tenant decision access or a separate TemperPaw authorization layer.
