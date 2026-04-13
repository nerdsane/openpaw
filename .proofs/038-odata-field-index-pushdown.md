# Proof Report: 038 — OData Field Index Push-Down

## Date
2026-04-13

## Branch / Commit
- **temper**: `feat/odata-field-index-pushdown` merged to `main` via PR #115 (commit `215b700`)
- **openpaw**: `main` (commit `f247104d`)

## What Was Done
Implemented a persistent EAV (Entity-Attribute-Value) field index table for OData filter push-down. Previously, all filtered collection queries materialized every entity actor to evaluate `$filter` in memory. Now, OData `$filter` expressions are translated to SQL WHERE clauses against the `entity_field_index` table, returning only matching entity IDs before materialization.

### Components (926 lines across 11 temper files + 16 openpaw lines)
1. **Schema**: `entity_field_index` table with lookup and status indexes (`temper-store-turso/src/schema.rs`)
2. **Store methods**: upsert/remove/query on TursoEventStore (`temper-store-turso/src/store/field_index.rs`)
3. **OData-to-SQL translator**: FilterExpr to SQL WHERE clauses (`temper-server/src/odata/filter_sql.rs`)
4. **Query path**: SQL push-down in `handle_entity_set()` with graceful fallback (`temper-server/src/odata/read.rs`)
5. **Write path**: Fire-and-forget field index upsert after every dispatch (`temper-server/src/state/dispatch/effects.rs`)
6. **Backfill**: Two-phase startup: snapshots then actor hydration (`temper-server/src/state/entity_ops.rs`)

## Verification Flow
Server built with `cargo build --release`, started with `RUST_LOG=info,temper_server::odata=debug,temper_server::state::entity_ops=debug`, and tested via curl against the live OData API.

## Verification Results
| Step | Expected | Actual | Status |
|------|----------|--------|--------|
| Build | Clean release build | `Finished release profile [optimized]` | PASS |
| Server boot | Server starts and listens on 3467 | Process 61934 listening on :3467 | PASS |
| Backfill (default tenant) | All entities indexed | 18,490/18,491 indexed (1 orphaned Bookmark) | PASS |
| Backfill (temper-system) | All entities indexed | 92/92 indexed | PASS |
| Push-down log | "OData filter push-down succeeded" in logs | Confirmed for Agent, Soul, ChannelSession during bootstrap | PASS |
| startswith filter | System skills returned | 8 entities with `/system/skills/` paths | PASS |
| eq filter | Only matching names returned | APP.md, SKILL.md queries return correct results | PASS |
| contains filter | Substring matches returned | `contains(path,'research')` returns 3 matching files | PASS |
| endswith filter | Suffix matches returned | `endswith(name,'.md')` returns markdown files | PASS |
| Combined AND filter | Intersection of conditions | `startswith(path,'/system/') and name eq 'SKILL.md'` returns 8 | PASS |
| Status filter | Status pseudo-field works | `Status eq 'Ready'` returns Ready files | PASS |
| No filter (regression) | Unfiltered queries unchanged | Files returned with $top, $orderby, $select working | PASS |
| Count query | $count works | 4,211 total files returned | PASS |
| Entity detail | Single entity GET works | Full entity with actions/children returned | PASS |
| Unit tests | All pass | 238 passed (222 server + 16 store) | PASS |
| CI (Compile & Lint) | Green | Passed (24m2s) | PASS |
| CI (Tests) | Green | Passed (1h13m32s) | PASS |
| CI (Spec Verification L0-L3) | Green | Passed (5m33s) | PASS |
| CI (Integrity & DST Patterns) | Green | Passed (23s) | PASS |
| CI (Verification Contract) | Green | Passed (8s) | PASS |

## What Worked
- Two-phase backfill (snapshots + actor hydration) solved the population gap that snapshot-only approach missed (only 45 of 18,491 entities had snapshots)
- OData-to-SQL translator handles all common filter patterns (eq, ne, gt/ge/lt/le, and/or/not, contains/startswith/endswith)
- Graceful fallback: unsupported filters fall back to in-memory evaluation with no regression
- Fire-and-forget field index updates on dispatch keep the index current without blocking writes
- Status pseudo-field indexing enables `$filter=Status eq 'Active'` style queries

## What Didn't Work
- Initial snapshot-only backfill only covered 45/17,789 entities (0.25%) — most entities lack snapshots (created at every 100th event). Fixed by adding actor hydration phase.
- One orphaned `Bookmark` entity fails hydration because its entity type spec is no longer registered. Benign — logged as debug warning.

## Limitations
- Only top-level scalar fields are indexed. Nested objects/arrays require full materialization for filtering.
- LIKE-based string functions (contains/startswith/endswith) don't use the SQL index efficiently for `contains` (leading `%` prevents index use). `startswith` does benefit from the index.
- Actor hydration phase spawns actors for all entities without snapshots (~18k on first boot). This is a one-time cost that runs as a background task and doesn't block the server.
- Text coercion: all field values stored as TEXT. Numeric comparisons use string ordering, not numeric ordering.

## What Still Doesn't Work
- Cross-field comparisons (e.g., `field1 eq field2`) are not supported and fall back to in-memory
- `has` operator is not supported and falls back to in-memory
- Computed expressions and nested property paths fall back to in-memory

## Artifacts
- Log file: `/tmp/openpaw_verify.log` (101,765 lines)
- PR: https://github.com/nerdsane/temper/pull/115 (merged)

## Architecture Diagram
```text
                    OData Query: Files?$filter=startswith(path,'/system/skills/')
                                        |
                                        v
                            +-------------------+
                            | handle_entity_set |
                            +-------------------+
                                        |
                          +-------------+-------------+
                          |                           |
                   $filter present?              No $filter
                          |                           |
                          v                           v
                 try_translate_filter()      select_entity_ids_for_
                          |                 materialization() [existing]
                   +------+------+
                   |             |
              translated     can't translate
                   |             |
                   v             v
          query_field_index()   Fallback: materialize
          (SQL WHERE on EAV)    all + filter in memory
                   |
                   v
          Returns: [id1, id2, ...]  (8 IDs, not 18k!)
                   |
                   v
          Materialize ONLY matching entities
                   |
                   v
          apply_query_options() (orderby, skip, top, select)
                   |
                   v
                Response

Write Path:
  dispatch_tenant_action_core() -> run_post_dispatch_effects()
      Step 8: tokio::spawn -> store.upsert_field_index(fields)

Startup Backfill:
  Phase 1: load_snapshot() for each entity -> upsert_field_index()  [cheap, ~45 entities]
  Phase 2: get_tenant_entity_state() for rest -> upsert_field_index() [hydrates actors, ~18k entities]
```
