# ADR-014: Staged Session Hot Path Reduction

- Status: Proposed
- Date: 2026-05-17

## Context

PERF-011 made terminal delivery routing explicit and deployed successfully, but
the production proof changed the latency diagnosis. The old direct
`agent_reply` path did perform unnecessary `ChannelSessions` lookups, but the
retained post-deploy traces show that lookup was not the dominant remaining
cost.

Live proof `perf-011-direct-route-fast-path-warm2-20260517162405` completed
from the client polling loop in `1939 ms`. The retained Datadog trace
`b91ff7ad9a427641356cc1a4e2da0f55` on version
`b51e2846393302c8abeedbe2e7a5a8141bb3bd24` shows the warm direct mock path:

- `wasm:workspace_provisioner`: `602 ms`
- `wasm:context_preparer`: `345 ms`
- `wasm:provider_caller`: `312 ms` even with the deterministic mock provider
- `wasm:provider_response_applier`: `396 ms`
- `wasm:agent_reply`: `197 ms`, with no child HTTP and an explicit direct
  no-route skip
- `wasm:emit_ots_trajectory`: `243 ms`

The same trace exposes specific first-order work:

- `workspace_provisioner` spends `370 ms` in `bootstrap_new_workspace` even
  after ADR-006 removed PawFS from the hot path.
- That bootstrap creates two `SessionEntry` rows, header and user, one after
  the other.
- `create_session_entry` intentionally performs a read-after-write verification
  after each POST because a previous orphan-chain bug advanced
  `session_leaf_id` past a row that did not become queryable.
- `provider_response_applier` spends `178 ms` in `append_session_tree`.
- Channel-route proof `b482861b82ccd470dabb47d02664b64c` proves reply delivery
  remains correct, but terminal channel delivery still pays
  `Paw.Channel.SendReply 748 ms` plus a `Channels` lookup at `46 ms`.

The larger architecture issue is that a simple provider-only turn still crosses
several verified Session states and several WASM modules. That is part of
Temper's inspectability and correctness story, but not every internal operation
needs to pay sequential same-process HTTP and serial verification when the work
can be batched while preserving the event log.

## Decision

PERF-012 will start with the smallest high-confidence change on the measured
critical path: batch the fresh hot Session bootstrap's two `SessionEntry`
creates and their read-after-write checks.

Specifically:

1. Add a shared helper that creates the header and first user `SessionEntry`
   through `ctx.http_call_batch`.
2. Keep the read-after-write verification, but verify the two entries through a
   second batched call instead of verifying each entry serially.
3. Use the helper in the hot fresh-session path where ADR-006 already avoids
   PawFS files.
4. Keep the existing serial `create_session_entry` helper for single-entry
   appends, legacy paths, and rollback.
5. Complete the PERF-011 route snapshot by carrying the Channel entity id from
   `route_message` when the current Channel entity is the route source, so
   channel replies can skip the remaining `Channels` lookup.

This does not collapse the Session state machine yet. It keeps the same
`Configure -> ProvisionWorkspace -> WorkspaceReady -> ContextReadyAuthSkipped ->
ProviderResponseReady -> RecordResult` externally visible flow, and it keeps the
SessionEntry correctness check that protects against data drift.

## Architecture Boundary

This ADR deliberately avoids a full composite turn executor as the first move.
A composite executor may be the right future architecture for the no-tools,
provider-only path, but it crosses a larger semantic boundary:

- it would combine multiple spec-visible states;
- it would need a new proof that Cedar, event audit, projection updates,
  trajectory emission, and recovery semantics still explain the turn;
- it may belong in Temper runtime/host APIs rather than only TemperPaw WASM.

The batched bootstrap change is smaller and evidence-backed. It attacks a
measured `370 ms` serial section without hiding any state transition.

## Semantics

The Session tree remains a governed append-only tree. The header and user
entries are still separate `SessionEntry` entities with the same ids, sequence
numbers, parent edge, content, tokens, and extra metadata as before.

Batching changes transport shape, not data shape:

- before: `POST header -> GET header -> POST user -> GET user`;
- after: `POST header + POST user` in one host batch, then
  `GET header + GET user` in one host batch.

If either create or either verification fails, the helper returns an error and
the caller refuses to advance a `session_leaf_id`. It may leave a partially
created row behind, but it will not create an orphaned active chain or feed a
missing parent to the next stage. The serial helper remains available for
rollback and for single-entry appends.

Completing `reply_channel_entity_id` also changes transport shape only. The
route still comes from the same Channel that received the message; the Session
simply records the entity id while that identity is already known.

## Consequences

Positive:

- Fresh direct Sessions should spend less time in
  `workspace_provisioner/bootstrap_new_workspace`.
- The improvement should be visible in client completion latency because
  workspace provisioning is on the user-visible path.
- Read-after-write correctness is preserved.
- Channel replies should avoid the remaining `Channels` lookup when the route
  starts from the current Channel entity.
- The changes are narrow enough to verify with focused unit tests, existing
  Session architecture tests, live proof, and retained Datadog traces.

Tradeoffs:

- `ctx.http_call_batch` failure handling needs to be conservative: any partial
  failure falls back rather than trying to reason about half-created entries.
- Batched creation may make individual child HTTP spans less visually serial in
  traces, so the helper must keep clear log messages and metrics.
- This does not remove all staged WASM overhead. `context_preparer`,
  `provider_response_applier`, `agent_reply`, and `emit_ots_trajectory` remain
  future targets.

## Follow-Up Architecture Options

If PERF-012A proves the expected shape but the warm path remains too slow, the
next ADR should evaluate one of these larger changes:

- a verified provider-only composite turn executor for no-tools/no-sandbox
  Sessions;
- a Temper host API for same-process entity create/action/read calls that avoids
  OData loopback while preserving Cedar and event recording;
- a batched SessionEntry append path for assistant/tool entries;
- a prompt/context template cache for system-prompt assembly;
- a channel reply fast path that avoids a full local `Paw.Channel.SendReply`
  action when inline delivery is safe.

## Verification

- Unit tests for batched SessionEntry request construction, response parsing,
  successful verification, and conservative fallback on partial failure.
- Unit tests that `route_message` includes `reply_channel_entity_id` for the
  current Channel entity.
- Affected WASM tests and release builds:
  - `wasm-helpers`
  - `workspace_provisioner`
  - `route_message`
  - `agent_reply` if route interpretation changes.
- Existing package checks:
  - `cargo fmt --all -- --check`
  - `cargo check --locked -p temperpaw -p paw-codex-worker`
  - Datadog observability contract
  - Session lifecycle/architecture tests.
- Live proof:
  - direct mock Session completes and keeps a valid SessionEntry chain;
  - channel-route Session replies correctly;
  - Datadog current-version trace shows lower
    `workspace_provisioner/bootstrap_new_workspace` and no remaining
    `agent_reply` `Channels` lookup for channel-created Sessions with complete
    route snapshots.

## Rollback

Keep the existing serial `create_session_entry` path. If the batched helper
misbehaves or Datadog shows no useful improvement, switch hot bootstrap back to
serial creation and leave the route entity-id snapshot in place only if its
correctness proof remains clean.
