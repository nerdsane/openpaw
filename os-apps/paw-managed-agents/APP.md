# paw-managed-agents

`paw-managed-agents` implements the Anthropic managed-agents beta shape
(`managed-agents-2026-04-01`) using Temper-native entities, WASM
integrations, and Cedar policies. There is no separate REST facade today:
the app is exposed through Temper's OData surface.

The app exposes 10 entity types:

- `ManagedEnvironment` — reusable sandbox template shared by sessions
- `ManagedAgent` — reusable managed-agent definition
- `ManagedSession` — lifecycle wrapper around one inner `OpenPaw.Session`
- `AgentMcpServer` — MCP server rows attached to a managed agent
- `AgentSkill` — skill rows attached to a managed agent
- `AgentTool` — toolset/tool rows attached to a managed agent
- `AgentToolConfig` — per-tool permission/config rows
- `SessionEvent` — managed-agents event log rows
- `SessionResource` — session-scoped resources such as repos or files
- `EnvironmentPackage` — package-install rows attached to an environment

Execution follows the same bridge pattern used by `paw-wiki`'s `WikiJob`:
`ManagedSession` creates and steers an inner `OpenPaw.Session`, while the
inner session continues to run the real agent loop. `ManagedEnvironment` is a
configuration template only; it does not provision or own long-lived
infrastructure entities.

This app is also the first OpenPaw app to lean heavily on Temper
ADR-0041 field invariants for the public API contract, so the enum and
cross-entity validation rules live in the specs rather than hidden Rust
glue.

The app is designed to be installed with:

```bash
curl -X POST http://localhost:3000/api/apps/install \
  -H 'content-type: application/json' \
  -H 'x-tenant-id: default' \
  -H 'x-temper-principal-kind: admin' \
  -d '{"tenant":"default","app_name":"paw-managed-agents"}'
```
