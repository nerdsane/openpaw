# Proof 065: Orphaned Session Recovery Default And Recoverer Packaging

Date: 2026-05-07
Branch: codex/session-recovery-default-and-module

## Failure

Production restarted while source-search sessions were in progress. Startup recovery was disabled by default, so parent curation jobs stayed in `Researching`. Enabling recovery through Railway reached the orphaned sessions, but `Session.RecoverFromRestart` failed because the `session_recoverer` WASM module was referenced by `session.ioa.toml` but not declared in `os-apps/paw-agent/app.toml` or built by `os-apps/paw-agent/wasm/build.sh`.

## Red

The new tests failed before the implementation change:

```text
cargo test -p temperpaw orphaned_session_recovery_is_enabled_by_default
startup::tests::orphaned_session_recovery_is_enabled_by_default ... FAILED
left: None
right: Some(25)
```

```text
cargo test -p temperpaw paw_agent_manifest_declares_terminal_session_wasm_modules
startup::tests::paw_agent_manifest_declares_terminal_session_wasm_modules ... FAILED
paw-agent app.toml must declare terminal Session module session_recoverer
```

```text
cargo test -p temperpaw paw_agent_build_script_builds_session_recoverer
startup::tests::paw_agent_build_script_builds_session_recoverer ... FAILED
paw-agent wasm build.sh must build session_recoverer
```

After the production `SessionLink.Configure` denial was isolated, I added a
manifest guard for the monitor WASM used by that path. It failed before the
manifest update:

```text
cargo test -p temperpaw paw_agent_manifest_declares_terminal_session_wasm_modules
startup::tests::paw_agent_manifest_declares_terminal_session_wasm_modules ... FAILED
paw-agent app.toml must declare session lifecycle module session_link_monitor
```

## Green

`TEMPERPAW_ORPHANED_SESSION_RECOVERY` now defaults to enabled and can still be explicitly disabled with `false`, `0`, `no`, `off`, `disabled`, or `none`.

`session_recoverer` and `session_link_monitor` are now app-required paw-agent
WASM modules. `session_recoverer` is also included in the paw-agent WASM build
script.

Verified:

```text
cargo test -p temperpaw orphaned_session_recovery
2 passed
```

```text
cargo test -p temperpaw paw_agent_manifest_declares_terminal_session_wasm_modules
1 passed
```

```text
cargo test -p temperpaw paw_agent_build_script_builds_session_recoverer
1 passed
```

```text
cargo fmt --check
passed
```

```text
bash -n os-apps/paw-agent/wasm/build.sh
passed
```

```text
cd os-apps/paw-agent/wasm/session_recoverer
cargo build --target wasm32-unknown-unknown --release
Finished release profile
```

```text
cargo test -p temperpaw
57 unit tests, 5 native skill tests, 2 paw-fs tests, 40 paw-patrol tests,
4 session lifecycle tests, and 13 session architecture tests passed.
```

## Production Hot Load

Set Railway production variables:

```text
TEMPERPAW_ORPHANED_SESSION_RECOVERY=true
TEMPERPAW_ORPHANED_SESSION_RECOVERY_MAX=1000
```

Hot-loaded the built WASM module into production:

```text
POST /api/wasm/modules/session_recoverer
module_name=session_recoverer
sha256_hash=2aef2194d515b18f99e08a4472828f2a51e4db8f5fe118a36050d3f285841b6b
size_bytes=361750
```

Verified the module is cached:

```text
GET /observe/wasm/modules
module_name=session_recoverer
cached=true
sha256_hash=2aef2194d515b18f99e08a4472828f2a51e4db8f5fe118a36050d3f285841b6b
```

Verified the running Railway service is usable:

```text
GET /readyz
200
status=ready
discord.status=connected
discord.configured=true
discord.connected=true
discord.connection_state=Connected
```

Hot-loaded the missing production `SessionLink` Cedar policy that was causing
replacement query child-session setup to fail with HTTP 403:

```text
POST /api/tenants/default/policies/create
policy_id=paw-agent-session-link
status=created

GET /api/tenants/default/policies/list
policy_id=paw-agent-session-link
enabled=true
```

Verified a system principal can configure a `SessionLink`:

```text
POST /tdata/SessionLinks
POST /tdata/SessionLinks('{link_id}')/TemperPaw.Configure
result=2xx
```

Restarted the ten lost curation queries after the policy hot-load. Immediate
post-submit verification showed all ten active with no `SessionLink` 403s:

```text
10 CurationQueries submitted
10 CurationJobs Running source_search
10 child Sessions created
SessionLinks configured and Watching/Completed
```

Follow-up status check:

```text
7 CurationQueries Synthesizing
3 CurationQueries Researching
0 CurationQueries Failed
```
