# ADR-003: Materialize Virtual Session Continuations

- Status: Accepted
- Date: 2026-06-03

## Context

Channel continuations normally append the incoming user message to the previous
Session's durable `SessionEntry` tree before creating the next `Session`.

OpenAI Codex auth failures can happen before `provider_response_applier` runs.
Those failed first-turn Sessions can have:

- `session_file_id = session-entries:<session_id>`
- `session_leaf_id = u-<session_id>-0`
- `session_entries_materialized = false`
- no durable `SessionEntry` rows
- a valid `Session.user_message`

Before this ADR, `route_message` treated the missing parent entry as a
recoverable append failure and created a clean continuation. That made an
ongoing Discord thread look like every message was a new conversation whenever
the prior turn failed before SessionEntry materialization.

## Decision

When continuing from a prior Session whose `session_entries_materialized` field
is explicitly `false`, `route_message` must materialize the virtual first turn
from `Session.user_message` before appending the new user message.

The continuation path now:

1. Detects the explicit virtual marker.
2. Creates the initial header entry and prior user entry through the shared
   `create_initial_session_entries` helper.
3. Appends the new user message as sequence `2`, parented to the materialized
   prior user entry.
4. Configures the continuation Session with the carried `session_file_id`,
   advanced `session_leaf_id`, and `session_entries_materialized = true`.

Missing parent entries without the explicit virtual marker are no longer treated
as a safe reason to start clean. They remain hard continuity errors because a
silent reset would hide data-loss or corruption.

## Consequences

Positive:

- Failed first-turn provider/auth Sessions can still preserve thread context on
  the next channel message.
- Operators see an explicit routing failure for unexpected missing SessionEntry
  parents instead of a quiet context reset.
- The continuation Session's state reflects the durable reality after
  materialization.

Tradeoffs:

- `route_message` now depends on the batched SessionEntry creation host call via
  `create_initial_session_entries`.
- Native route-message tests need a non-WASM `host_http_call_batch` stub.

## Verification

- Unit tests cover that verification failures can still start a clean
  continuation, but a missing parent leaf no longer does.
- Unit tests cover that carried continuations mark
  `session_entries_materialized = true`.
- Production OData proof created a virtual failed prior Session, routed a
  follow-up through `Paw.Channel.ReceiveMessage`, and observed durable
  `SessionEntry` rows:
  - header `h-<prior_session_id>`
  - prior user `u-<prior_session_id>-0`
  - continuation user `u-2` parented to the prior user entry
- The spawned continuation reused the same `session-entries:<prior_session_id>`
  ref and prepared context contained both user messages.

## Rollback

Revert the `route_message` materialization branch and restore the previous clean
continuation fallback for `session entries continuation missing parent leaf`.
This rollback would reintroduce silent context resets after pre-materialization
auth failures, so it should only be used if SessionEntry materialization itself
is causing production write failures.
