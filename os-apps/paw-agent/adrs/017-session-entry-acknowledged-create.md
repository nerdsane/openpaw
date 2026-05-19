# ADR-017: Acknowledged SessionEntry Create Verification

- Status: Proposed
- Date: 2026-05-19

## Context

PERF-027 deployed Temper's generic data-only create fast path and proved that
`SessionEntry` no longer needs an actor spawn for ordinary turn materialization.
The live post-deploy proof on TemperPaw `sha-8dcbe10c` completed correctly, but
Datadog did not accept it as a latency win:

- baseline retained trace `11156646544924625715`:
  `wasm:provider_response_applier` about `291 ms`;
- post-deploy retained trace `958e12b4dd7faf4030bdc68bf4a48fdf`:
  `wasm:provider_response_applier` about `403 ms`;
- `entity.create_data_only_tenant_entity_fast_path` busy time was below `1 ms`
  for each sampled create, while span idle/wait time dominated.

The remaining `SessionEntry` append helper still performs a synchronous
read-after-write collection query after every successful `POST
/tdata/SessionEntries`. That read-back was introduced after an orphan-chain
incident where a `POST` returned `2xx` but the entry was not visible through the
projection. The platform contract has since changed: create handlers now return
non-`2xx` when the query projection write fails, and the data-only fast path
only returns `201` after the event journal append and query projection upsert
both succeed.

The defensive read-back is now paying latency on the hot path while also
depending on the exact filtered projection query path that the latency program
is trying to harden. Correctness still matters more than a cosmetic speedup, so
the replacement must keep a proof boundary instead of blindly trusting any HTTP
success.

## Decision

For `SessionEntry` creates, treat a successful OData create response as the
primary write acknowledgment when the response body proves that the server
accepted the exact `SessionId` and `EntryId` requested.

The helper will:

1. send the same governed `POST /tdata/SessionEntries` request with the same
   tenant and runtime headers;
2. require a `2xx` response;
3. parse the returned entity state and verify that the returned `SessionId` and
   `EntryId` match the request;
4. return success without issuing an immediate filtered OData read-back by
   default;
5. retain an explicit `session_entry_create_verify_readback` boolean knob in
   entity fields or WASM config that restores the old read-back verification
   for emergency diagnosis, rollback confidence, or canary comparison.

This does not bypass Temper. The write still goes through the OData handler,
write prechecks, Cedar/runtime identity headers, event journal, projection
acknowledgment, and the deployed data-only create fast path.

## Correctness Contract

The new hot-path correctness boundary is:

- HTTP status must be `2xx`;
- response JSON must contain the same `SessionId` and `EntryId` requested;
- Temper's create handler must continue to surface projection failures as
  create errors;
- live verification must still perform an independent query after the action
  completes to prove the final `SessionEntry` chain is visible and ordered.

If any of those assumptions breaks, enable
`session_entry_create_verify_readback=true` to restore the old immediate
read-after-write query while investigating.

## Consequences

Positive:

- Removes a synchronous filtered collection read from every hot `SessionEntry`
  create in the normal provider response path.
- Reduces coupling between the write path and the fragile projection filter
  path while projection correctness observability continues to improve.
- Keeps the existing OData/governance/event/projection write contract intact.

Tradeoffs:

- The immediate WASM helper no longer independently proves projection
  visibility unless the strict read-back knob is enabled.
- The platform-level guarantee that `201` means event plus projection
  acknowledgment becomes more important and must remain covered by Temper tests.
- If the production projection path regresses silently again, live/end-to-end
  checks and Datadog projection drift signals, not every hot write, should catch
  it.

## Verification

- Red-green unit tests in `wasm-helpers` for:
  - accepted response parsing with nested OData entity-state fields;
  - mismatch rejection for wrong `SessionId` or `EntryId`;
  - default read-back mode off and explicit config/field override on.
- Build/test the affected WASM helper and provider-response path.
- Live production proof after deploy:
  - create provider-only Sessions on the deployed version;
  - independently query `SessionEntries` after completion and verify exact
    user/assistant chain ordering;
  - compare Datadog before/after for
    `wasm:provider_response_applier`,
    `provider_response_applier append_session_tree`, and disappearance or
    reduction of read-back OData spans.

## Rollback

Set `session_entry_create_verify_readback=true` in the Session fields or WASM
config to restore the old immediate read-back behavior without reverting code.
If the change itself must be reverted, restore the helper to always perform the
filtered read-back after every successful create.
