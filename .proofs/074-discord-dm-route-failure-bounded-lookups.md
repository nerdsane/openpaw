# 074 - Discord DM Route Failure Bounded Lookups

Date: 2026-06-16

## Scope

Investigated Discord DMs that were received and dispatched but never replied.
The prior "missing Channel entity for Discord DM channel 1494" diagnosis did
not hold up: live logs showed `Channel.ReceiveMessage` succeeded on a Temper
`Channel` entity and invoked the `route_message` WASM integration. The failure
was after dispatch, ending in `Channel.RouteFailed`.

Live Datadog evidence for the 2026-06-16 22:00:19 UTC DM:

```text
discord message received author_id=1018228973869727785 channel_id=1494059279202779136 message_id=1516563098402685070
discord receive_message dispatched author_id=1018228973869727785 channel_id=1494059279202779136 message_id=1516563098402685070
trajectory.entry action=ReceiveMessage entity_type=Channel entity_id=en-019eba01-164d-7d60-ab4d-5e710a5e39d6 success=true from=Connected to=Connected
invoking WASM integration module route_message
trajectory.entry action=RouteFailed entity_type=Channel entity_id=en-019eba01-164d-7d60-ab4d-5e710a5e39d6 success=true
```

Same trace window also showed a slow catalog coverage query returning 77,527
rows while serving an OData entity-set read. That matches the unbounded
`route_message` active `ChannelSessions` lookup and the Temper fallback path
that performed coverage before enforcing the full-proof scan budget.

ADR judgement:

- TemperPaw route lookup change: no ADR. It is a narrow WASM reliability fix
  inside the existing `Channel.ReceiveMessage -> route_message` transition.
- Temper platform changes: ADRs added in the Temper worktree:
  - `docs/adrs/0143-bounded-source-cursor-coverage.md`
  - `docs/adrs/0144-idempotent-inline-cedar-loads.md`

## Changes

TemperPaw:

- Bounded `route_message` active `ChannelSessions` lookup with `$top=1`.
- Escaped OData string literals for channel/thread/author ids.
- Replaced full `AgentRoutes` scan with two bounded route queries:
  channel-specific route first, global fallback second.

Temper:

- Made inline Cedar policy loading idempotent so repeated loads do not append
  the same policy bundle over and over.
- Rejected oversized filtered/count/order source-cursor OData reads before
  catalog coverage materialization.

## Red-Green Evidence

Red tests failed before implementation:

```text
cargo test --manifest-path os-apps/paw-channels/wasm/route_message/Cargo.toml lookup
```

Failure:

```text
cannot find function `active_session_query`
cannot find function `agent_route_queries`
```

Temper red tests were added for:

```text
inline_cedar_policy_merge_deduplicates_repeated_bundle_text
source_cursor_catalog_coverage_is_skipped_when_candidate_set_exceeds_budget
```

## Verification

TemperPaw:

```text
cargo fmt --check
cargo test --manifest-path os-apps/paw-channels/wasm/route_message/Cargo.toml
cargo test -p temperpaw --test datadog_observability_contract
cargo test -p temperpaw --test paw_fs_hot_path
cargo check -p temperpaw
cargo clippy --locked -p temperpaw -p paw-codex-worker --all-targets -- -D warnings
git diff --check
```

Results:

```text
route_message: 25 passed
datadog_observability_contract: 32 passed
paw_fs_hot_path: 13 passed
cargo check --locked -p temperpaw -p paw-codex-worker: passed
cargo clippy --locked -p temperpaw -p paw-codex-worker --all-targets: passed
cargo fmt --check: passed
git diff --check: passed
old Temper rev 23c455fbcd1c1aa7d4f7b21c52b5cd94c9dd085a absent
```

Temper:

```text
cargo fmt --check
cargo test -p temper-server odata::query_plane_read::tests
cargo test -p temper-server --features observe inline_cedar_policy_merge
cargo check -p temper-server --features observe
git diff --check
```

Results:

```text
query_plane_read: 11 passed
inline_cedar_policy_merge: 2 passed
cargo check -p temper-server --features observe: passed
cargo fmt --check: passed
git diff --check: passed
```

Production readiness check:

```text
curl -fsS https://openpaw-production.up.railway.app/readyz
```

Result:

```json
{"status":"ready","discord":{"status":"connected","configured":true,"connected":true,"desired_state":"connected","connection_state":"Connected","last_error":null,"next_retry_at":null}}
```

## Deployment Status

Production entity mutation was not needed for the fix. Live logs showed the
Discord channel entity existed and `Channel.ReceiveMessage` reached
`route_message`; the failing step was the route-to-agent lookup, not channel
binding. The production bot was connected before this branch landed, so the
remaining rollout work is to merge the Temper and TemperPaw branches, publish
the merged image, and redeploy Railway with the updated `route_message` WASM
plus the pinned Temper platform fix.
