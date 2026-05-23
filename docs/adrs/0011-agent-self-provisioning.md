# ADR-0011: Agent Self-Provisioning

## Status

Superseded by ADR-0051 for app install and repair semantics.

This ADR is historical. Agents still use Cedar-governed platform APIs, but the
normal app install/repair path is Genesis pinned refs:
`search -> publish/update -> install -> verify`.

## Context

OpenPaw agents need to create new capabilities at runtime — entity specs with state machines, WASM integration modules, and Cedar authorization policies. Today, agents can submit specs (`temper.submit_specs`) and upload WASM (`temper.upload_wasm`), but three gaps prevent full self-provisioning:

1. **Hard-coded blocks in dispatch.** The monty_repl dispatch layer hard-codes errors for `approve_decision`, `deny_decision`, and `set_policy`, preventing agents from managing governance decisions or Cedar policies regardless of their Cedar authorization level.

2. **No Cedar policy methods.** Agents have no dispatch methods to create, read, update, or delete Cedar policies — even though the Temper platform exposes full policy CRUD APIs (`/api/tenants/{tenant}/policies/*`).

3. **Incomplete capability installer.** The CapabilityRequest → capability_installer flow handles `os_app`, `specs`, `wasm`, and `secret` types, but not `cedar_policy` or bundled `app` (specs + WASM + policies together).

The Temper platform already enforces Cedar authorization on every API call and auto-creates GovernanceDecision entities when Cedar denies an action. The denial-to-approval flow (agent denied → GovernanceDecision created → human approves via Observe/Discord UI → Cedar permit generated → agent retries) is fully operational. The code-level blocks in dispatch are redundant and inconsistent with Cedar-as-sole-gate.

## Decision

### Remove code-level blocks, expose all provisioning APIs

Replace the hard-coded error in `dispatch.rs` with real dispatch methods that call Temper platform APIs. Cedar is the **only** authorization gate. No method is blocked in code — if Cedar denies, the platform handles it.

### New dispatch methods

| Method | Temper API | Purpose |
|--------|-----------|---------|
| `temper.submit_policy(id, text)` | `POST /api/tenants/{t}/policies/create` | Create Cedar policy |
| `temper.list_policies()` | `GET /api/tenants/{t}/policies/list` | List all policies |
| `temper.get_policy(id)` | Filter from list | Read one policy |
| `temper.update_policy(id, text)` | `PATCH /api/tenants/{t}/policies/entry/{id}` | Update policy text |
| `temper.delete_policy(id)` | `DELETE /api/tenants/{t}/policies/entry/{id}` | Remove policy |
| `temper.approve_decision(id, scope)` | `POST /api/tenants/{t}/decisions/{id}/approve` | Approve governance decision |
| `temper.deny_decision(id)` | `POST /api/tenants/{t}/decisions/{id}/deny` | Deny governance decision |

### Two provisioning paths

**Direct path** — Agent calls `submit_specs()`, `upload_wasm()`, `submit_policy()` directly. Cedar evaluates each call. If permitted, the capability is hot-loaded immediately (specs available in OData, WASM modules available for next invocation, Cedar policies active after reload). If denied, GovernanceDecision is created and the agent can poll it.

**Approval path** — Agent calls `temper.install_app(name, reason, payload, capability_type)` which creates a CapabilityRequest entity. Human approves via Observe UI. The `capability_installer` WASM integration executes the actual installation. Works for all types: `os_app`, `specs`, `wasm`, `cedar_policy`, `app` (bundled).

### Bundled app creation

The `app` capability type lets an agent submit specs + WASM + policies as one unit through the CapabilityRequest approval flow. The payload is:

```json
{
  "specs": {"MyEntity.ioa.toml": "<ioa content>"},
  "wasm_modules": {"my_handler": "<base64 wasm>"},
  "policies": {"my_policy": "<cedar text>"}
}
```

The `capability_installer` processes these in dependency order: specs first, then WASM, then policies. Failure at any step transitions to `InstallFailed` with the error.

### WASM compilation workflow

Agents compile Rust to WASM in the Tensorlake sandbox:

```python
sandbox.bash("cd /workspace/my_module && cargo build --target wasm32-unknown-unknown --release")
wasm_b64 = sandbox.bash("base64 -w0 /workspace/my_module/target/wasm32-unknown-unknown/release/my_module.wasm")
temper.upload_wasm("my_module", wasm_b64)
```

No `compile_wasm` dispatch method is needed — the sandbox has the Rust toolchain.

### Cedar governance

Default-deny. The Cedar policies shipped with paw-agent control access:

- **Read policies**: Any authenticated agent can `list_policies` and `get_policy`
- **Write policies**: Only `supervisor`, `human`, `admin` can `submit_policy`, `update_policy`, `delete_policy`
- **Approve/deny decisions**: Only `supervisor`, `human`, `admin` can `approve_decision`, `deny_decision`
- **Submit specs / upload WASM**: Governed by platform Cedar (`submit_specs` on `SpecRegistry`, `manage_wasm` on `WasmModule`)

To grant an agent policy-write access, a human adds a Cedar permit rule via the Observe UI or decision approval flow. The agent cannot bootstrap its own permissions — the human-in-the-loop is the safety valve.

## Alternatives Considered

### 1. Keep code-level blocks, add CapabilityRequest-only path

Keep `dispatch.rs` blocks for policy writes, require all policy changes to go through CapabilityRequest → human approval.

**Rejected because:** This creates two authorization systems (code blocks + Cedar) that can diverge. Cedar is already the platform's authorization engine — adding code blocks on top is redundant and makes the system harder to reason about. The CapabilityRequest path is preserved as an option, not the only option.

### 2. Separate "policy agent" role

Create a dedicated PolicyAgent entity type that's the only principal allowed to manage Cedar policies.

**Rejected because:** Cedar already supports fine-grained principal-based authorization. Adding a new entity type for what Cedar already handles is over-engineering. The existing `agent_type` attribute on principals is sufficient for role-based policy control.

## Consequences

### Positive

- **Cedar as single source of truth.** No authorization logic split between code and Cedar.
- **Full self-provisioning.** Agents can create entity types, WASM modules, and Cedar policies — the complete stack for a new capability.
- **Hot-loading.** All provisioning is immediate — no daemon restart. Specs are available in OData, WASM modules are compiled and cached, Cedar policies are validated and activated.
- **Auditable.** Every provisioning action is logged as a trajectory entry. GovernanceDecision entities record all denials and approvals.
- **Progressive trust.** Default-deny + human approval for sensitive actions. As trust is established, humans can grant broader Cedar permits.

### Negative

- **Recursive policy risk.** An agent with policy-write access could theoretically grant itself more permissions. Mitigated by: default-deny requires human to grant the initial policy-write, and the Observe UI shows all policy changes for review.
- **Partial bundle failure.** The `app` type installs sequentially — a failure mid-way leaves partial state. Mitigated by: `InstallFailed` records which step failed, and retry is safe (idempotent upserts).
