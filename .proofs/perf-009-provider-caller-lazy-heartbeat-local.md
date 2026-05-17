# PERF-009 Provider Caller Lazy Heartbeat Local Proof

- Date: 2026-05-17
- Branch: `codex/provider-caller-lazy-heartbeat-20260517`
- Worktree: `/Users/seshendranalla/Development/temperpaw-worktrees/provider-caller-lazy-heartbeat-20260517`
- ADR: `os-apps/paw-agent/adrs/012-provider-caller-lazy-heartbeat.md`
- Change:
  - `os-apps/paw-agent/specs/session.ioa.toml`
  - `os-apps/paw-agent/wasm/provider_caller/src/lib.rs`
  - `crates/temperpaw/tests/session_turn_architecture.rs`

## Evidence

PERF-008 removed the non-Codex provider auth gate from the hot path. A fresh
Datadog proof trace still showed the normal no-Codex Session workflow dispatching
an eager `Session.Heartbeat` between `ContextReadyAuthSkipped` and
`ProviderResponseReady`. The self-loop preserved liveness while the provider was
called, but it cost an additional actor dispatch, event write, projection update,
and state read on the latency-critical path before the provider response could be
applied.

PERF-009 makes that eager pre-provider heartbeat opt-in. The normal fast path
keeps the existing typing indicator, progress callback configuration, provider
timeout, and mock-hang timeout proof behavior.

## Red Step

Red tests were added before implementation:

- `initial_provider_heartbeat_is_opt_in_and_never_masks_mock_hang` failed because
  `should_send_initial_provider_heartbeat` did not exist.
- The architecture guard was added to assert the Session spec disables the eager
  provider heartbeat by default and provider caller no longer contains the old
  `if !mock_hang { send_heartbeat(...) }` hot-path pattern.

## Local Verification

Passed:

- `cargo test --manifest-path os-apps/paw-agent/wasm/provider_caller/Cargo.toml initial_provider_heartbeat_is_opt_in_and_never_masks_mock_hang`
- `cargo test --manifest-path os-apps/paw-agent/wasm/provider_caller/Cargo.toml`: `25/25`
- `cargo test -p temperpaw --test session_turn_architecture provider_caller_initial_heartbeat_is_opt_in`
- `cargo test -p temperpaw --test session_turn_architecture`: `15/15`
- `cargo test -p temperpaw --test session_lifecycle_and_config`: `6/6`
- `cargo test -p temperpaw --test datadog_observability_contract`: `31/31`
- `cargo build --manifest-path os-apps/paw-agent/wasm/provider_caller/Cargo.toml --target wasm32-unknown-unknown --release`
- `bash build.sh` in `os-apps/paw-agent/wasm`
- `cargo check --workspace`
- `cargo clippy --locked -p temperpaw -p paw-codex-worker --all-targets -- -D warnings`
- `cargo check --locked -p temperpaw -p paw-codex-worker`
- `cargo test --locked -p temperpaw --quiet`
- `cargo test --locked -p paw-codex-worker --quiet`
- `cargo fmt --all -- --check`
- `git diff --check`

Non-blocking observation:

- The full paw-agent WASM bundle still reports existing unrelated warnings in
  `sandbox_provisioner` and `monty_repl`, matching the prior PERF-008 build.

## Expected Live Verification

After deploy, run a no-Codex mock-provider Session against production and confirm:

- the Session completes correctly;
- normal production traces no longer show `Session.Heartbeat` between
  `ContextReadyAuthSkipped` and `ProviderResponseReady`;
- Datadog `Session.workflow` and ordered dispatch spans show one fewer
  Session actor dispatch in the no-Codex provider hot path;
- the mock-hang proof path still exercises the `CallingProvider` timeout without
  an eager pre-provider heartbeat masking it;
- provider progress callbacks still work when explicitly enabled.
