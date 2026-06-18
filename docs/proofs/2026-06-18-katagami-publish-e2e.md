# Katagami Publish Path E2E Proof

- Date: 2026-06-18
- Linear: ARN-51, ARN-54, ARN-55

## Local Verification

- `cargo test -p temperpaw --test paw_fs_hot_path`
  - Result: 15 passed.
- `cargo build --target wasm32-wasip1 --release`
  - Path: `os-apps/paw-agent/wasm/monty_repl`
  - Result: passed with pre-existing warnings.
- `./build.sh`
  - Path: `os-apps/paw-fs/wasm/artifact_batch_apply`
  - Result: passed.

## Genesis Publish And Install

Published and installed:

- `temperpaw/paw-fs@8cc9c1a0c3959ba0555a6eac5446db76de747817`
  - Genesis latest verified at publish time.
  - Production install status: 200.
  - Closure: `genesis:temperpaw/paw-fs@8cc9c1a0c3959ba0555a6eac5446db76de747817:8cc9c1a0c3959ba0555a6eac5446db76de747817`
  - Materialized path: `/root/.local/share/temperpaw/genesis-app-cache/temperpaw-paw-fs-8cc9c1a0c3959ba0555a6eac5446db76de747817`
  - WASM: `artifact_batch_apply`

Published but not installed:

- `temperpaw/paw-agent`: blocked.
  - Authenticated `ls-remote` with `GENESIS_TOKEN` works.
  - Full clone/fetch reproducibly stalls around 77-78 percent of received objects.
  - Shallow clone is unsupported.
  - Partial clone is unsupported.
  - Force publish is correctly denied by Genesis policy.
  - Low-level fast-forward push without fetching the parent is rejected because
    receive-pack cannot traverse the missing parent object.

## Residual Blocker

The agent-side `temper.publish_app` / `temper.update_app` auth helper is
implemented and tested locally, but cannot be live-installed until Genesis
upload-pack/fetch for `temperpaw/paw-agent` is repaired or a local clone with
the current parent object is available.
