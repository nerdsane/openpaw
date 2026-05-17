# PERF-012 Staged Session Hot Path Local Proof

Date: 2026-05-17
Branch: `codex/session-hot-path-reduction-20260517`

## Scope

ADR-014 first tranche for the staged Session hot path:

- batch fresh hot Session bootstrap `SessionEntry` header/user creates;
- batch the read-after-write verification for those two entries;
- keep the same SessionEntry ids, parent edge, sequence numbers, content, and
  fail-closed verification behavior;
- carry `reply_channel_entity_id` from `route_message` when the current Channel
  entity is the delivery route source.

## Local Verification

Passed:

- `cargo test --lib` in `os-apps/paw-agent/wasm/wasm-helpers`
  - `27/27`
- `cargo test --lib` in `os-apps/paw-channels/wasm/route_message`
  - `17/17`
- `cargo test --lib` in `os-apps/paw-agent/wasm/workspace_provisioner`
  - build/test target passed
- `cargo build --target wasm32-unknown-unknown --release` in
  `os-apps/paw-agent/wasm/workspace_provisioner`
- `cargo build --target wasm32-wasip1 --release` in
  `os-apps/paw-channels/wasm/route_message`
- `cargo clippy --lib -- -D warnings` in
  `os-apps/paw-agent/wasm/workspace_provisioner`
- `cargo fmt --all -- --check`
- `git diff --check`
- `cargo check --locked -p temperpaw -p paw-codex-worker`
- `cargo test -p temperpaw --test session_lifecycle_and_config`
  - `6/6`
- `cargo test -p temperpaw --test session_turn_architecture`
  - `15/15`
- `cargo test -p temperpaw --test datadog_observability_contract`
  - `31/31`
- `cargo test -p temperpaw --test temperpaw_identity_contract`
  - `9/9`
- `cargo clippy --locked -p temperpaw -p paw-codex-worker --all-targets -- -D warnings`
- `cargo test -p temperpaw`
  - all package suites passed

Known note:

- Standalone `route_message` clippy still emits pre-existing
  `too_many_arguments` warnings. Package clippy with `-D warnings` is green.

## Pending Remote Proof

After PR merge and deployment:

- direct mock/API-key Session live proof;
- channel-route live proof;
- Datadog current-version trace comparison for:
  - lower `workspace_provisioner/bootstrap_new_workspace`;
  - preserved valid SessionEntry chain;
  - no `agent_reply` `Channels` lookup when `reply_channel_entity_id` is present;
  - continued OTS trajectory/background WASM/error/projection observability.
