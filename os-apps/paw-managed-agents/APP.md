# paw-managed-agents

`paw-managed-agents` exposes a managed-agent style API on top of OpenPaw's
existing Temper-native primitives. The public entities are:

- `ManagedEnvironment`
- `ManagedAgent`
- `ManagedSession`

Child entities break the API's structured arrays into queryable rows:

- `AgentMcpServer`
- `AgentSkill`
- `AgentTool`
- `AgentToolConfig`
- `SessionEvent`
- `SessionResource`
- `EnvironmentPackage`

Execution stays Temper-native. `ManagedSession` does not run its own loop.
Instead, WASM integrations bridge to:

- `OpenPaw.Agent` / `OpenPaw.Session` for the actual agent loop
- `Paw.Compute.Computer` for persistent environment records

The app is designed to be installed with:

```bash
curl -X POST http://localhost:3000/api/apps/install \
  -H 'content-type: application/json' \
  -H 'x-tenant-id: default' \
  -H 'x-temper-principal-kind: admin' \
  -d '{"tenant":"default","app_name":"paw-managed-agents"}'
```
