# PERF-034 Context Preparer Inline Artifact Local Proof

- Date: 2026-05-20
- Worktree: `/Users/seshendranalla/Development/temperpaw-worktrees/context-preparer-inline-artifact-threshold-20260520`
- Branch: `codex/context-preparer-inline-artifact-threshold-20260520`
- Base: `221f008266b90c794f7af6c2de0c06bd247f1254`

## Before Evidence

Datadog production trace `4dddabfa615bd6bdba39caf4230d0fe7` on
`service.version=3c1b32f4301f30d6e01208dd49e03ac087e400c4` showed
`HandleToolResults` context preparation paying File `$value` IO for a medium
prepared-context artifact:

- `GET /tdata/Files(...)/$value`: about `284 ms`, `response_bytes=39701`
- `PUT /tdata/Files(...)/$value`: about `432 ms`, `request_bytes=45714`

The artifact was around `45 KiB`; the previous default inline budget was
`32 KiB`.

## Change

ADR-026 raises the default prepared-context inline budget to `128 KiB`, keeps
the existing runtime/config override, preserves File-backed storage for larger
artifacts, and adds Datadog-visible artifact metrics:

- `temper_session_prepared_context_artifact_bytes`
- `temper_session_prepared_context_artifact_storage_total`

Both metrics include `mode=inline|file`.

## Local Verification

Passed:

```text
cargo fmt --all -- --check
git diff --check
cargo test --locked -p temperpaw --test session_turn_architecture context_preparer_keeps_medium_artifacts_inline_and_measurable -- --nocapture
cargo test --locked -p temperpaw --test session_turn_architecture -- --nocapture
cargo test --manifest-path os-apps/paw-agent/wasm/context_preparer/Cargo.toml -- --nocapture
cargo build --target wasm32-unknown-unknown --release
cargo check --locked -p temperpaw
cargo clippy --locked -p temperpaw --all-targets -- -D warnings
```

Observed counts:

- Session architecture focused guard: `1/1`
- Session architecture suite: `22/22`
- Context-preparer unit tests: `14/14`

## Remaining Proof Gates

This is not a shipped speed win yet. Remaining gates:

- Open PR and pass CI.
- Merge and deploy to production.
- Run a live Session/tool-result proof.
- Confirm after-Datadog evidence on the fixed version:
  - medium artifacts emit `mode=inline`;
  - matching context-preparer File `$value` read/write spans disappear for the
    same size class;
  - SessionEntry read-back and terminal Session correctness still pass.
