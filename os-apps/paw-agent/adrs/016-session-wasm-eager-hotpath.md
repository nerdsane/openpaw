# ADR-016: Eager Load Session Hot-Path WASM

- Status: Proposed
- Date: 2026-05-17

## Context

PERF-013 removed the no-op direct terminal delivery module for direct Sessions,
but the retained production trace for `perf-013-direct-noreply-20260517185016`
on version `df66cbc1a19c496e2db5aa0ce34823a824da57c4` showed a new first-turn
shape:

- `Session.workflow`: `4082 ms`
- `wasm:workspace_provisioner`: `607 ms`
- `wasm:provider_response_applier`: `574 ms`
- `blob.transport.get` for `workspace_provisioner`: `121 ms`
- `blob.transport.get` for `provider_response_applier`: `192 ms`
- logs inside both modules: `WASM module compiled and cached` and
  `lazy-compiled persisted WASM module on first use`

The warm phase metrics are much lower than the cold trace envelope:

- `workspace_provisioner` phase: about `172 ms`
- `provider_response_applier` invocation: about `101 ms`
- Postgres transaction p95 in the proof window: below `25 ms`

That means the next measured bottleneck is not primarily database work or the
SessionEntry data model. The immediate miss is that the modules on the normal
provider-only Session path are discovered as bundled artifacts but are not
declared in `paw-agent/app.toml`, so Temper gives them the default
`startup_loading = "lazy"` policy. On deployment or restart, the first live
request can pay object-store blob fetch and WASM compile before user-visible
work begins.

## Decision

Declare the latency-critical Session WASM modules in `paw-agent/app.toml` and
mark them eager:

- `workspace_provisioner`
- `context_preparer`
- `provider_auth_gate`
- `provider_caller`
- `provider_response_applier`
- `agent_reply`
- `emit_ots_trajectory`

Keep large, rare, or background modules lazy, including `monty_repl`,
`session_link_monitor`, and `session_recoverer`.

This is an app-manifest contract change, not a state-machine change. Temper's
existing OS app installer already honors `startup_loading = "eager"` by
compiling and caching declared modules when the app bundle is installed.

## Semantics

The Session flow is unchanged:

`Created -> ProvisionWorkspace -> WorkspaceReady -> ContextReady/AuthSkipped -> ProviderResponseReady -> RecordResult/RecordResultNoReply -> MarkTrajectoryEmitted`

The change only moves WASM compilation and persisted blob fetch earlier in the
process lifecycle. It does not remove Cedar checks, entity events, projection
writes, SessionEntry read-after-write verification, trajectory emission, or
tenant isolation.

For direct no-route Sessions, `agent_reply` is no longer invoked after
PERF-013, but keeping it eager protects channel-bound Sessions from paying a
cold terminal delivery module on the first routed reply after deploy.

## Consequences

Positive:

- First live Session after deploy should no longer pay cold compile/blob costs
  for the normal provider-only path.
- Datadog traces should stop showing `lazy-compiled persisted WASM module on
  first use` for these modules after a clean deploy/install.
- The change is low semantic risk because it affects startup work placement,
  not business behavior.

Tradeoffs:

- Startup/install does more CPU work up front.
- If the app reconcile path skips WASM because the bundle digest is unchanged
  after a process restart, persisted-module recovery may still require a later
  Temper runtime change to eagerly warm persisted modules based on stored
  manifest policy. This ADR intentionally starts with the app contract because
  the PERF-013 deployment trace showed missing manifest declarations.
- Eager loading every module would improve cold-path coverage but waste startup
  work on tool/sandbox paths that are not always needed.

## Verification

- Add a manifest test that every provider-only Session hot-path module is
  declared with `startup_loading = "eager"`.
- Add a guard that `monty_repl` stays lazy.
- Run focused startup/session architecture tests.
- Build the affected WASM artifacts if code changes are otherwise needed.
- Live proof after deploy:
  - direct mock Session completes successfully and retains a valid
    SessionEntry chain plus trajectory;
  - routed/channel Session still sends a reply;
  - Datadog current-version traces for the first post-deploy proof do not show
    lazy compile/blob fetch spans for the declared hot-path modules.

## Rollback

Set the hot-path module declarations back to `startup_loading = "lazy"` or
remove the manifest declarations. That restores the prior lazy first-use
behavior without changing Session specs or stored data.
