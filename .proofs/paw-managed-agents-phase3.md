# paw-managed-agents Proof

Date: 2026-04-15

## Scope

Implemented `os-apps/paw-managed-agents` as a Temper-native OpenPaw app with:

- `ManagedEnvironment`
- `ManagedAgent`
- `ManagedSession`
- child entities for tools, skills, MCP servers, resources, packages, and events
- WASM integrations for environment provisioning, session orchestration, event emission, and termination

This proof also includes the supporting Temper fixes required for:

- OData filtering on entities created or updated through plain OData writes
- correct IOA parsing when `[[field_invariant]]` or `[[agent_trigger]]` sections appear after `[[action]]`

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

Later, once resume was fixed, the proof exposed a deeper Temper runtime issue:

```text
POST /tdata/ManagedSessions('<id>')/Temper.Archive failed: HTTP 409
Unknown action: Archive
```

Inspection of `@odata.actions` showed that the runtime was advertising
`ArchivedAtRequiresTerminatedStatus` as an action instead of `Archive`.

### Green

Fixes applied:

1. `ManagedSession.IdleSession` now carries `stop_reason` at the transition boundary.
2. `session_orchestrator` now sets `stop_reason = user_input_required` when a bridged inner session completes.
3. `ManagedSession` field invariants now validate the actual automaton state fields (`stop_reason`, `archived_at`) instead of only the OData-style field names.
4. Temper’s hand-rolled IOA parser now treats `[[field_invariant]]` and `[[agent_trigger]]` as passthrough sections instead of letting them overwrite the last parsed action name.
5. Added Temper parser regressions that prove action names remain stable when those deferred sections follow an action.
6. The proof runner now asserts that terminated sessions advertise `Archive` and do not leak field-invariant names through `@odata.actions`.
7. The proof runner now waits for the asynchronous `session.status_terminated` event instead of racing the terminator integration.
8. Temper was patched so OData filters work for entities created and updated through the standard OData paths.

## Verification

### Managed-agents WASM build

```text
$ bash os-apps/paw-managed-agents/wasm/build.sh
-> session_orchestrator built successfully
-> event_emitter built successfully
-> environment_provisioner built successfully
-> session_terminator built successfully
-> managed_agent_updater built successfully
```

### Temper parser regressions

```text
$ cargo test -p temper-spec test_field_invariant_section_does_not_overwrite_previous_action -- --nocapture
test automaton::parser::tests::features::test_field_invariant_section_does_not_overwrite_previous_action ... ok

$ cargo test -p temper-spec test_agent_trigger_section_does_not_overwrite_previous_action -- --nocapture
test automaton::parser::tests::triggers::test_agent_trigger_section_does_not_overwrite_previous_action ... ok

$ cargo test -p temper-spec --quiet
running 180 tests
...
test result: ok. 180 passed; 0 failed
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

### OpenPaw build against patched Temper

```text
$ CARGO_TARGET_DIR=/tmp/openpaw-managed-agents-target cargo --config /tmp/openpaw-managed-agents-patch.toml build -p openpaw --release --bin openpaw-server
Finished `release` profile [optimized] target(s) in 4m 38s
```

The patch config pointed `openpaw`’s Temper git dependencies at the local
Temper worktree for verification only; no tracked Cargo files were changed for
this runtime smoke.

### End-to-end lifecycle proof

Server booted against a fresh local Turso database on a fresh port using the
patched `openpaw-server` binary. The proof ran against a fresh tenant with a
real managed-agent lifecycle and strict metadata assertions.

Proof command:

```text
$ OPENPAW_SERVER=http://127.0.0.1:3110 OPENPAW_TENANT=managed-agents-review-9 OPENPAW_API_KEY=managed-agents-review-8-secret python3 -u os-apps/paw-managed-agents/tests/prove_paw_managed_agents.py
== paw-managed-agents proof ==
Installing app bundle...
Creating managed environment...
Creating managed agent...
Updating managed agent...
Adding a built-in tool row...
Adding explicit tool config rows...
Creating managed session...
Posting initial user event...
Starting session...
Checking bound computer and inner agent state...
Fetching emitted events...
Event kinds: ['user.message', 'session.status_running', 'agent.message', 'session.status_idle']
Posting follow-up user event...
Resuming session...
Fetching resumed events...
Resumed event kinds: ['user.message', 'session.status_running', 'agent.message', 'session.status_idle', 'user.message', 'session.status_running', 'agent.message', 'session.status_idle']
Terminating session...
Archiving session...
Checking terminated event semantics...
Negative check: bogus event kind should fail...
Constraint rejection observed as expected.
Negative check: archived session should block child rows...
Archive gate rejection observed as expected.
Proof completed successfully.
```

## Notes

- The runtime now advertises `Archive` correctly in `@odata.actions`, and the
  proof explicitly rejects any leaked field-invariant names there.
- The managed-session bridge now validates and survives the full lifecycle:
  - start
  - first idle
  - resume
  - second idle
  - terminate
  - archive
  - archive-gated child rejection
