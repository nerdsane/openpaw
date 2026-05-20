# PERF-034 Prepared Context Byte Total Metric Proof

Date: 2026-05-20
Branch: `codex/prepared-context-byte-count-metric-20260520`
Base: `origin/main` at `b30947f3`

## Purpose

The production proof for PERF-034 showed
`temper_session_prepared_context_artifact_storage_total{mode:inline}` arriving
in Datadog, while `temper_session_prepared_context_artifact_bytes` did not
appear in metric metadata or metric queries. The context preparer emits both
metrics on the same path, so this branch adds a count-style byte total companion
metric on the Datadog path that is already proven visible.

## ADR

- `os-apps/paw-agent/adrs/027-prepared-context-artifact-byte-count.md`

## Red-Green Evidence

Red test:

```text
cargo test --locked -p temperpaw --test session_turn_architecture \
  context_preparer_keeps_medium_artifacts_inline_and_measurable -- --nocapture

Result: failed as expected before implementation because
`temper_session_prepared_context_artifact_bytes_total` was absent.
```

Green test:

```text
cargo test --locked -p temperpaw --test session_turn_architecture \
  context_preparer_keeps_medium_artifacts_inline_and_measurable -- --nocapture

Result: passed.
```

## Verification

```text
cargo test --locked -p temperpaw --test session_turn_architecture -- --nocapture
Result: passed, 22 tests.

cargo fmt --all -- --check
Result: passed.

cargo check --locked -p temperpaw
Result: passed.

cargo clippy --locked -p temperpaw --all-targets -- -D warnings
Result: passed.

./os-apps/paw-agent/wasm/build.sh
Result: passed. All WASM modules built.
```

The full WASM build produced unrelated existing warnings in untouched modules:

- `sandbox_provisioner`: unused import `SandboxConfig`
- `monty_repl`: unused doc comment above a macro invocation

## Expected Live Proof After Deploy

After deployment, a mock-fast or real session turn should emit both:

- `temper_session_prepared_context_artifact_storage_total{mode:inline}`
- `temper_session_prepared_context_artifact_bytes_total{mode:inline}`

The first metric proves inline-vs-external artifact mode. The second gives a
Datadog-visible byte total that can be summed by `version`, `provider`, `model`,
`mode`, `trigger_action`, and `wasm_module`.
