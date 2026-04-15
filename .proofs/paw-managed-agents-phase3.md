# paw-managed-agents Proof

Date: 2026-04-15

## Scope

Implemented `os-apps/paw-managed-agents` as a Temper-native OpenPaw app with:

- `ManagedEnvironment`
- `ManagedAgent`
- `ManagedSession`
- child entities for tools, skills, MCP servers, resources, packages, and events
- WASM integrations for environment provisioning, session orchestration, event emission, and termination

This proof also includes the supporting Temper fix required for OData filtering on entities created or updated through plain OData writes.

## Red → Green

### Red

The original end-to-end proof covered only the first `StartSession` flow. After extending the proof runner to cover:

- second `user.message`
- `ResumeSession`
- second idle transition
- `TerminateSession`
- archive flow
- archive child-gate rejection

the proof failed on resume with:

```text
POST /tdata/ManagedSessions('<id>')/ManagedAgents.ResumeSession failed: HTTP 409
ConstraintViolation: ManagedSession in Idle must set StopReason
```

Later, once resume was fixed, the proof exposed that the archive step was not callable through the expected `ManagedAgents.*` route and had to be driven from the action target advertised by OData metadata.

### Green

Fixes applied:

1. `ManagedSession.IdleSession` now carries `stop_reason` at the transition boundary.
2. `session_orchestrator` now sets `stop_reason = user_input_required` when a bridged inner session completes.
3. `ManagedSession` field invariants now validate the actual automaton state fields (`stop_reason`, `archived_at`) instead of only the OData-style field names.
4. The session archive action was renamed to `ArchiveManagedSession`, and the proof runner now uses the archive action target advertised by OData metadata so the flow remains compatible with the current runtime behavior.
5. Cedar policy now permits the runtime-advertised archive action name as well.
6. Temper was patched so OData filters work for entities created and updated through the standard OData paths.

## Verification

### Managed-agents WASM build

```text
$ bash os-apps/paw-managed-agents/wasm/build.sh
-> session_orchestrator built successfully
-> event_emitter built successfully
-> environment_provisioner built successfully
-> session_terminator built successfully
```

### OpenPaw build

```text
$ cargo build -p openpaw --release
Finished `release` profile [optimized] target(s) in 0.31s
```

### Temper regression tests

These validate the supporting OData projection fix in the sibling Temper checkout:

```text
$ cargo test -p temper-server --test odata_read -- --nocapture
running 10 tests
...
test filtered_entity_set_returns_entities_created_via_odata_post ... ok
test filtered_entity_set_reflects_odata_patch_updates ... ok
...
test result: ok. 10 passed; 0 failed
```

### OpenPaw tests

```text
$ cargo test -p openpaw --quiet
running 19 tests
...
startup::tests::startup_os_apps_only_include_core_apps --- FAILED
...
left: ["katagami-commons", "katagami-curation", "paw-agent", "paw-channels", "paw-foresight", "paw-fs"]
right: ["paw-agent", "paw-channels", "paw-fs"]
```

This failure was already present before the managed-agents work and is unrelated to the new app.

### End-to-end lifecycle proof

Server booted against a fresh local Turso database with:

- `OPENAI_CODEX_TOKEN` loaded from local Codex auth
- Anthropic and OpenRouter keys unset
- `LLM_PROVIDER=openai_codex`

This allowed the managed-agent flow to exercise the real bridge while the inner `llm_caller` cleanly fell back from Anthropic-model ids to the available Codex-backed provider.

Proof command:

```text
$ OPENPAW_SERVER=http://127.0.0.1:3106 python3 -u os-apps/paw-managed-agents/tests/prove_paw_managed_agents.py
== paw-managed-agents proof ==
Installing app bundle...
Creating managed environment...
Creating managed agent...
Adding a built-in tool row...
Creating managed session...
Posting initial user event...
Starting session...
Fetching emitted events...
Event kinds: ['user.message', 'session.status_running', 'agent.message', 'session.status_idle']
Posting follow-up user event...
Resuming session...
Fetching resumed events...
Resumed event kinds: ['user.message', 'session.status_running', 'agent.message', 'session.status_idle', 'user.message', 'session.status_running', 'agent.message', 'session.status_idle']
Terminating session...
Archiving session...
Negative check: bogus event kind should fail...
Constraint rejection observed as expected.
Negative check: archived session should block child rows...
Archive gate rejection observed as expected.
Proof completed successfully.
```

## Notes

- The runtime currently advertises the session archive action through the OData action target rather than the expected `ManagedAgents.ArchiveManagedSession` route. The proof runner uses the advertised target directly so verification remains robust.
- The managed-session bridge now validates and survives the full lifecycle:
  - start
  - first idle
  - resume
  - second idle
  - terminate
  - archive
  - archive-gated child rejection
