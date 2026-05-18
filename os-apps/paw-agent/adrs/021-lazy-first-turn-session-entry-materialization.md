# ADR-021: Lazy First-Turn SessionEntry Materialization

## Status

Accepted for PERF-023 implementation.

## Context

After PERF-022, production trace `92681eb9bdb5c38e210d05989e9c4447`
shows the accepted inline CLI proof still spends about `1.47 s` before the
intentional `2 s` workflow drain. The largest product-path costs are:

- `wasm:workspace_provisioner`: about `461 ms`.
- `wasm:provider_response_applier`: about `403 ms`.
- `wasm:context_preparer`: about `323 ms`.
- `wasm:emit_ots_trajectory`: about `287 ms`.

The `workspace_provisioner` span includes two local `POST /tdata/SessionEntries`
calls for the initial header and user entries, around `150 ms` and `126 ms`, plus
batched read-back verification. This preserves correctness, but it happens
before the provider call even though the same user message is already persisted
on the Session by `Configure` and visible to `context_preparer`.

Removing read-back verification would be fast but unsafe. We previously needed
that verification to catch acknowledged-but-missing SessionEntry writes and avoid
orphan chains. The next slice should therefore move work off the pre-provider
path without weakening the final Session tree.

## Decision

For fresh hot SessionEntry-backed sessions, `workspace_provisioner` will create a
virtual Session tree reference instead of immediately writing the header and user
`SessionEntry` rows.

The Session will carry `session_entries_materialized = "false"` while the first
turn is virtual. `context_preparer` may prepare the first prompt from
`Session.user_message` when the referenced `SessionEntries` tree is empty. When
`provider_response_applier` appends the provider response, it must materialize
the header, user, and assistant entries together, then set
`session_entries_materialized = "true"` in the terminal or tool action params.

Existing continuation sessions, legacy TemperFS session files, configured
workspace legacy mode, and already-materialized SessionEntry trees keep the
current behavior.

## Correctness Rules

1. Do not drop the final Session tree. A successful provider response must leave
   the header, user, and assistant entries readable through `SessionEntries`.
2. Do not remove write verification. The materialization helper must still
   verify the created entries are visible before the Session advances.
3. Do not claim materialization on failure. If any create or read-back check
   fails, `provider_response_applier` must fail or fall back without setting
   `session_entries_materialized = "true"`.
4. Preserve existing routes:
   `RecordResult`, `RecordResultNoReply`, `RecordResultInlineReply`,
   `ProcessToolCalls`, and steering paths must still set the same result,
   token, pending-tool, system-prompt, provider-response, and OTS fields they set
   today.
5. Preserve Cedar governance and entity audit. This change only moves when
   SessionEntry entities are created; it does not bypass Session actions,
   Channel audit, OTS emission, tenant headers, or policy checks.

## Expected Latency Effect

The pre-provider hot path should stop paying the initial SessionEntry create and
read-back cost during `workspace_provisioner`. The provider response phase will
pay one larger verified materialization batch only after the provider succeeds.
The accepted trace suggests the first-turn critical path can reclaim roughly
`100-230 ms` from `workspace_provisioner`, with the exact net gain measured by
production traces because the assistant append phase becomes slightly larger.

## Observability

Keep existing `temper_session_phase_duration_ms` and
`temper_session_phase_step_duration_ms` metrics. Add explicit guest logs for:

- virtual first-turn SessionEntry setup in `workspace_provisioner`;
- virtual context preparation in `context_preparer`;
- successful or failed first-turn SessionEntry materialization in
  `provider_response_applier`.

Datadog acceptance must show:

- lower `workspace_provisioner/bootstrap_new_workspace` timing for a fresh hot
  Session;
- `provider_response_applier` materialization evidence;
- final Session state with `session_entries_materialized = "true"`;
- readable `SessionEntries` for header, user, and assistant;
- preserved `RecordResultInlineReply` or `RecordResultNoReply`/`RecordResult`
  behavior and OTS trajectory emission.

## Rollback

Revert the `session_entries_materialized` state field, restore eager
`create_initial_session_entries` calls in `workspace_provisioner`, and remove the
provider-side first-turn materialization path. Existing materialized sessions
remain readable because the SessionEntry schema is unchanged.
