# ADR-022: SessionEntry Sequence Ordering

## Status

Accepted for the SessionEntry ordering follow-up.

## Context

PERF-023 moved first-turn SessionEntry creation out of `workspace_provisioner`
and into verified provider-response materialization. The live proof preserved the
final header/user/assistant tree, but it also exposed a pre-existing ordering
bug: both the accepted PERF-022 baseline and the PERF-023 after proof stored the
first user entry and first assistant entry with `Sequence = 1`.

The API currently returned the rows in the expected order, but the reconstructed
SessionTree path sorts by `(Sequence, EntryId)`. With duplicate sequence values,
that fallback can order `a-*` before `u-*`, which risks corrupting future context
preparation even though the underlying entries are all present.

The cause is local ID parsing. The first user entry ID is shaped as
`u-<session_id>-0`, where the trailing `0` is the first user-turn index. The
helper that creates the next SessionEntry interpreted that suffix as the parent
sequence and produced assistant `Sequence = 1`.

## Decision

Keep the existing first user entry ID shape, including `u-<session_id>-0`, but
map that shape to a logical SessionEntry sequence before appending children:

- header: `Sequence = 0`
- initial user `u-<session_id>-0`: `Sequence = 1`
- first assistant child: `Sequence = 2`

Legacy compact IDs such as `u-1`, `a-17`, and `t-3` continue to use their numeric
suffix directly. The fix is scoped to generated initial user-turn IDs that start
with `u-ss-`.

## Correctness Rules

1. Never emit duplicate sequence numbers for the canonical first-turn
   header/user/assistant chain.
2. Preserve existing EntryId readability and compatibility for already-created
   SessionEntry rows.
3. Keep SessionTree reconstruction deterministic by preserving the existing
   `(Sequence, EntryId)` fallback sort while ensuring new canonical entries have
   distinct sequence values.
4. Do not weaken PERF-023 read-back verification or
   `session_entries_materialized=true` semantics.

## Observability And Verification

Local tests must prove that `u-ss-...-0` advances to assistant sequence `2`,
while legacy numeric IDs still advance by one. Production acceptance must include
a live read-back proof showing header/user/assistant sequences `0/1/2` and a
Datadog trace/log window for the fixed version.

## Rollback

Revert the helper change. Existing entries remain readable either way; rollback
only affects the sequence assigned to newly appended SessionEntry rows.
