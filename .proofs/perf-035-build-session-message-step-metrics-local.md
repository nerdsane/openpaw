# PERF-035 Build Session Message Step Metrics Local Proof

## Before Evidence

Production Datadog trace `cf02563a7d213eb32a85d2d55e99e553` showed repeated
`wasm:build_session_message` spans in CurationJob submit bursts taking about
5.4-6.9 seconds. The spans had only about 0.27-0.64 seconds of guest busy time;
most of the duration was idle wait on host/OData work.

The existing trace split showed the module performs several stateful OData
boundaries in sequence:

- workspace resolution when no `workspace_id` is already present
- `Session` create
- `Session.Configure`
- `WikiJob.SessionSpawned`
- `SessionLink` create
- `SessionLink.Configure`

The trace did not expose which one was the dominant contributor, so a safe
speed change would have been guesswork.

## Change

Added `temper_wiki_build_session_message_step_duration_ms` histogram metrics to
`os-apps/paw-wiki/wasm/build_session_message` with tags:

- `job_type`
- `step`
- `result`

Measured steps:

- `ensure_workspace`
- `create_session`
- `configure_session`
- `session_spawned`
- `create_session_link`
- `configure_session_link`
- `total`

The change preserves the existing entity-first flow and visible failure
semantics. `SessionLink` setup still fails the parent WikiJob if it cannot be
established.

## Local Gates

- Red test:
  `cargo test --locked -p temperpaw --test session_lifecycle_and_config wiki_build_session_message_emits_step_metrics_for_spawn_path -- --nocapture`
  failed before the implementation because the metric contract was absent.
- Green targeted test:
  `cargo test --locked -p temperpaw --test session_lifecycle_and_config wiki_build_session_message_emits_step_metrics_for_spawn_path -- --nocapture`
- Full affected architecture suite:
  `cargo test --locked -p temperpaw --test session_lifecycle_and_config -- --nocapture`
  passed `7/7`.
- Formatting:
  `cargo fmt --all -- --check`
- Standalone WASM formatting:
  `cargo fmt -- --check`
  in `os-apps/paw-wiki/wasm/build_session_message`
- Whitespace:
  `git diff --check`
- WASM build:
  `cargo build --target wasm32-unknown-unknown --release`
  in `os-apps/paw-wiki/wasm/build_session_message`
- WASM lint:
  `cargo clippy --target wasm32-unknown-unknown --release -- -D warnings`
  in `os-apps/paw-wiki/wasm/build_session_message`
- TemperPaw package check:
  `cargo check --locked -p temperpaw`
- TemperPaw package lint:
  `cargo clippy --locked -p temperpaw --all-targets -- -D warnings`

## Acceptance

This slice is accepted only when the metrics are merged, deployed, a live
WikiJob/CurationJob path emits the new step metrics, and Datadog identifies the
dominant slow boundary. It is not counted as a speed win until the follow-on
optimization has before/after production evidence.
