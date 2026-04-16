# paw-managed-agents Proof

Date: 2026-04-15

## Scope

Implemented `os-apps/paw-managed-agents` as an OpenPaw app aligned to the
Anthropic `managed-agents-2026-04-01` beta shape.

Follow-up review fixes on 2026-04-15 also addressed:

- removal of the stale `os-apps/paw-managed-agents/wasm/environment_provisioner/`
  build residue
- provider-level sandbox policy propagation in `paw-agent`, including
  provider request serialization and package-install setup hooks
- cleanup of pre-existing `paw-agent` CSDL drift so the XML matches the live
  `Session` runtime contract

The final architecture is:

- `ManagedEnvironment` is a reusable sandbox configuration template
- `ManagedAgent` is a managed-agent definition that bridges to an inner
  `OpenPaw.Agent`
- `ManagedSession` bridges to an inner `OpenPaw.Session`
- child entities model tools, skills, MCP servers, packages, resources, and
  session events
- runtime behavior is Temper-native: entity specs, WASM integrations, Cedar
  policy, and cross-invariants
- there is no REST facade in this implementation; the surface is OData only

This is also the first OpenPaw app to rely heavily on ADR-0041 field
invariants for event-shape discrimination and lifecycle validation.

## Red → Green

### Red

The review feedback uncovered one real platform-level failure that the earlier
proof had not covered deeply enough:

- app installation was loading entity specs and Cedar policy, but not
  `specs/cross-invariants.toml`

That meant negative lifecycle checks could pass incorrectly even when the
specs looked right. In practice:

- archived managed agents still allowed new `ManagedSession` rows
- archived/terminated managed sessions still allowed new child rows

The end-to-end proof reproduced that failure before the platform fix.

### Green

Fixes applied in the OpenPaw app:

1. Removed the `Computer`-based environment architecture entirely.
2. `ManagedEnvironment` now remains a pure template for sandbox settings.
3. `session_orchestrator` now passes sandbox config through to the inner
   `OpenPaw.Session.Configure` action.
4. `paw-agent` session state now accepts managed sandbox config fields:
   networking type, allowed hosts JSON, MCP/package-manager toggles, and
   package manifests.
5. Lazy sandbox provisioning in `paw-agent` now reads those session fields
   instead of assuming default sandbox behavior.
6. `managed_agent_updater` now scopes tool-config fetches correctly and bumps
   `Version` on update.
7. `event_emitter` now reads the session tree once per invocation and emits
   `agent.tool_use` / `agent.tool_result` with captured tool input.
8. The proof runner now verifies tool events, bridged sandbox config, and
   archived-parent negative cases.

Fixes applied in Temper:

1. OS app bundles now load `specs/cross-invariants.toml`.
2. OS app installation persists cross-invariants alongside specs/policies.
3. Bootstrap registration now receives those persisted cross-invariants so the
   in-memory registry enforces them immediately.
4. `PlatformStore` now supports constraint persistence in both the real and
   simulation paths.

## Verification

### Focused unit tests

```text
$ cargo test --manifest-path os-apps/paw-agent/wasm/wasm-helpers/Cargo.toml --quiet
running 17 tests
.................
test result: ok. 17 passed; 0 failed

$ cargo test -p temper-platform test_load_app_bundle_reads_cross_invariants --quiet
running 1 test
.
test result: ok. 1 passed; 0 failed
```

### Managed-agents WASM build

```text
$ bash os-apps/paw-managed-agents/wasm/build.sh
-> session_orchestrator built successfully
-> event_emitter built successfully
-> session_terminator built successfully
  -> managed_agent_updater built successfully
```

### paw-agent WASM rebuild

```text
$ bash os-apps/paw-agent/wasm/build.sh
-> llm_caller built successfully
-> sandbox_provisioner built successfully
-> workspace_provisioner built successfully
-> context_compactor built successfully
-> steering_checker built successfully
-> coding_agent_runner built successfully
-> heartbeat_scan built successfully
-> heartbeat_scheduler built successfully
-> heartbeat_typing built successfully
-> cron_compute_next built successfully
-> workspace_restorer built successfully
-> agent_reply built successfully
-> request_approval built successfully
-> request_plan_review built successfully
-> capability_installer built successfully
-> plan_approval_handler built successfully
-> plan_review_feedback_handler built successfully
-> monty_repl built successfully
```

### OpenPaw server build against patched Temper

```text
$ cargo build --config /tmp/openpaw-managed-agents-patch.toml \
    --target-dir /tmp/openpaw-managed-agents-target \
    -p openpaw --bin openpaw-server --release
Finished `release` profile [optimized] target(s) in 54.18s
```

The patch config pointed OpenPaw’s Temper git dependencies at the local Temper
checkout for verification only.

### End-to-end lifecycle proof

Fresh tenant on the existing local server:

- server: `http://127.0.0.1:3113`
- port: `3113`
- tenant: `managed-agents-review-12`

Proof command:

```text
$ OPENPAW_SERVER=http://127.0.0.1:3113 \
  OPENPAW_TENANT=managed-agents-review-12 \
  OPENPAW_REQUEST_TIMEOUT=120 \
  python3 -u os-apps/paw-managed-agents/tests/prove_paw_managed_agents.py
```

Observed output:

```text
== paw-managed-agents proof ==
Installing app bundle...
Creating managed environment...
Adding environment packages...
Creating managed agent...
Updating managed agent...
Adding a built-in tool row...
Adding explicit tool config rows...
Creating managed session...
Posting initial user event...
Starting session...
Checking bridged inner session and inner agent state...
Fetching emitted events...
Event kinds: ['user.message', 'session.status_running', 'agent.message', 'session.status_idle']
Posting follow-up user event...
Resuming session...
Fetching resumed events...
Resumed event kinds: ['user.message', 'session.status_running', 'agent.message', 'session.status_idle', 'user.message', 'session.status_running', 'agent.tool_use', 'agent.tool_result', 'agent.message', 'session.status_idle']
Terminating session...
Archiving session...
Checking terminated event semantics...
Negative check: bogus event kind should fail...
Constraint rejection observed as expected.
Negative check: archived session should block child rows...
Archive gate rejection observed as expected.
Negative check: archived agent should block new sessions...
Archived-agent session rejection observed as expected.
Proof completed successfully.
```

### Additional follow-up checks

- `os-apps/paw-managed-agents/wasm/environment_provisioner/` no longer exists on
  disk after cleanup
- the rebuilt `paw-agent` bundle now includes sandbox policy serialization and
  setup logic from `wasm-helpers/src/sandbox.rs`
- `paw-agent` CSDL now exposes a single copy of `SessionMode`,
  `PrePlanToolsEnabled`, and `ActivePlanId`, and `HandleToolResults` now includes
  `sandbox_provider`, `pending_tool_context`, and `pending_decision_id`

### What the proof explicitly verified

The proof exercised and confirmed all of the following:

- install `paw-managed-agents` into a fresh tenant
- create `ManagedEnvironment` with:
  - `NetworkingType = Limited`
  - `AllowedHostsJson = ["github.com"]`
  - `AllowMcpServers = true`
  - `AllowPackageManagers = false`
- create `EnvironmentPackage` rows for both `apt` and `pip`
- create and update a `ManagedAgent`
- confirm `ManagedAgent.Version` increments on update
- create `AgentTool` / `AgentToolConfig` rows
- create a `ManagedSession`
- submit a first `user.message`
- `StartSession`
- confirm bridged inner `Session` received:
  - `SandboxNetworkingType`
  - `SandboxAllowedHostsJson`
  - `SandboxAllowMcpServers`
  - `SandboxAllowPackageManagers`
  - `SandboxPackagesJson`
- confirm bridged inner `Agent.ToolsEnabled == {"bash", "temper_get"}`
- confirm emitted lifecycle events:
  - `session.status_running`
  - `agent.message`
  - `session.status_idle`
- submit a second `user.message`
- `ResumeSession`
- confirm tool event flow:
  - `agent.tool_use`
  - `agent.tool_result`
- `TerminateSession`
- confirm `session.status_terminated` stores `TerminationReason`
- archive the managed session using the advertised OData action target
- reject invalid `SessionEvent.Kind` with `409`
- reject new `SessionResource` rows after archive with `409`
- reject new `ManagedSession` rows for an archived `ManagedAgent` with `409`

### Runtime evidence from server logs

During the negative checks, the rebuilt runtime emitted real constraint
violations at the OData boundary, confirming the platform fix was active:

- field-invariant rejection for bogus `SessionEvent.Kind`
- cross-invariant rejection for child rows against a terminal/archived
  `ManagedSession`
- cross-invariant rejection for new sessions against an archived
  `ManagedAgent`

## Notes

- `ManagedEnvironment` no longer creates or binds a `Paw.Compute.Computer`.
- Sandbox provisioning remains lazy and delegated to `paw-agent`, which now
  receives the managed-environment template fields through the bridged inner
  session.
- The proof harness now uses a configurable request timeout so cold tenant
  installs do not fail spuriously during local verification.
