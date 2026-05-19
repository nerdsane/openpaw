# ADR-025: Headerless First-Turn SessionEntry Materialization

## Status

Proposed for PERF-026.

## Context

PERF-023 moved first-turn SessionEntry creation out of `workspace_provisioner`
and into `provider_response_applier`. PERF-024 compressed materialization
read-back from per-entry reads to one session-scoped read. PERF-025 then showed
that context-preparer prompt metadata batching was safe, but not a full-span
latency win.

The current production baseline on
`dae25573e50a752dd105ea3c0986c3bdc5f5b770` points back at
first-turn materialization:

- `Session.ProviderResponseReady.integrations`: average about `620.4 ms`
- `wasm:provider_response_applier`: average about `620.2 ms`
- `provider_response_applier` `append_session_tree` logs: `284`, `306`,
  `387`, `358`, `384`, `398`, and `364 ms` (`354.4 ms` average)

For a virtual first turn, `provider_response_applier` currently materializes
three `SessionEntries` after the provider returns:

1. a version-only header row;
2. the initial user message;
3. the assistant response.

The user and assistant rows are conversational truth. The header row is a
metadata root inherited from the JSONL session-tree shape. `SessionTree` already
supports walking from a root message whose `parentId` is empty/null, and
context building skips header rows anyway. For the hot first-turn path, creating
the header row adds a third OData entity create, actor spawn, projection upsert,
and row to read back without adding prompt content.

## Decision

Use a headerless first-turn materialization shape for virtual hot SessionEntry
sessions:

```text
user message      ParentEntryId = ""              Sequence = 1
assistant reply   ParentEntryId = user EntryId    Sequence = 2
```

Keep legacy/headered trees readable. Do not change `SessionTree::new`, legacy
TemperFS JSONL sessions, or existing persisted sessions. The change only affects
the post-provider materialization helper used when `session_entries_materialized`
is still false.

The `session_leaf_id` after materialization remains the assistant entry. Future
turns append from that assistant leaf exactly as before. The single-entry append
path remains unchanged and keeps its existing per-entry read-back verification.

## Correctness Rules

1. The initial user message must be visible as a `SessionEntry` before the
   Session claims `session_entries_materialized=true`.
2. The assistant response must be visible as a child of that user entry before
   the Session claims `session_entries_materialized=true`.
3. The materialization read-back remains mandatory and must verify all expected
   EntryIds in one session-scoped read.
4. `SessionTree` context walking must produce the same user/assistant prompt
   messages for headerless first turns as it does for headered first turns.
5. Legacy headered sessions remain accepted; no migration is required.

## Observability And Verification

Before evidence:

- Version `dae25573e50a752dd105ea3c0986c3bdc5f5b770`
- Traces `e7afb8b251389da3e7d252d562518432`,
  `48358a5f80fd62301588797d23bc4475`,
  `04c9a8dd34f0ee58752ae3112c699e68`,
  `d0606750f139b99738d15312219603c9`, and
  `820318da6aca8b1af741ee3122c8c6bc`
- `append_session_tree` log samples listed above
- Existing live read-back shape: header/user/assistant `0/1/2`

Acceptance requires:

- local tests proving headerless user/assistant materialization and
  `SessionTree` context walking;
- existing provider-response applier and wasm-helper tests;
- Session architecture tests updated to assert the new materialization contract;
- release WASM builds for affected modules;
- PR, merge, Docker, Railway deploy, and version probe;
- live Session proof whose `SessionEntries` read back as user/assistant with
  the assistant child linked to the user root;
- after Datadog span/log window comparing
  `Session.ProviderResponseReady.integrations`, `wasm:provider_response_applier`,
  and `append_session_tree` against the baseline.

## Rollback

Revert the helper/test changes. The rollback restores the header/user/assistant
materialization shape while keeping PERF-023 through PERF-025 intact.
