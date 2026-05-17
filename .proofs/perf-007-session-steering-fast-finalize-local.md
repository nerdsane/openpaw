# PERF-007 Session Steering Fast Finalize Local Proof

- Date: 2026-05-17
- Branch: `codex/session-steering-fast-finalize-20260517`
- Worktree: `/Users/seshendranalla/Development/temperpaw-worktrees/session-steering-fast-finalize-20260517`
- ADR: `os-apps/paw-agent/adrs/010-queued-steering-fast-finalize.md`
- Change: `os-apps/paw-agent/wasm/provider_response_applier/src/lib.rs`

## Evidence

Production runtime `1c6c9cf79a8b5c7a0fd4e01ed86eac0ae6a82278` completed a
mock-provider Session proof with trace `dae3dbe3e519c8632855a1cd5b0c8bdf`.
Datadog showed:

- `Session.workflow`: about `4.55 s`
- `wasm:provider_response_applier`: about `611 ms`
- `Session.CheckSteering.integrations`: about `702 ms`

The proof had no queued steering messages, so the `CheckSteering` stage was a
no-op on the ordinary no-steering terminal response path.

## Local Verification

Passed:

- `cargo test` in `os-apps/paw-agent/wasm/provider_response_applier`:
  `10/10`
- `cargo test -p temperpaw --test session_turn_architecture`: `13/13`
- `cargo test -p temperpaw --test session_lifecycle_and_config`: `6/6`
- `cargo test -p temperpaw --test datadog_observability_contract`: `31/31`
- `cargo build --target wasm32-unknown-unknown --release` in
  `os-apps/paw-agent/wasm/provider_response_applier`
- `cargo check --workspace`
- `cargo clippy --locked -p temperpaw -p paw-codex-worker --all-targets -- -D warnings`
- `cargo check --locked -p temperpaw -p paw-codex-worker`
- `cargo test --locked -p temperpaw --quiet`
- `cargo test --locked -p paw-codex-worker --quiet`
- `bash build.sh` in `os-apps/paw-agent/wasm`
- `cargo fmt --all -- --check`
- `git diff --check`

Non-blocking observation:

- `cargo clippy --workspace --all-targets -- -D warnings` reports existing
  `temperpaw-cli` style warnings outside the package set used by CI.

## Expected Live Verification

After deploy, run a no-steering mock-provider Session against production and
confirm:

- the Session completes correctly
- `Session.CheckSteering.integrations` is absent on the no-steering proof path
- `Session.workflow` drops by roughly the removed steering stage
- queued steering still routes through `CheckSteering`
