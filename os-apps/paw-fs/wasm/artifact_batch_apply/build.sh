#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR"
cargo build --target wasm32-unknown-unknown --release

cp target/wasm32-unknown-unknown/release/artifact_batch_apply.wasm "$SCRIPT_DIR/artifact_batch_apply.wasm"
cp target/wasm32-unknown-unknown/release/artifact_batch_apply.wasm "$SCRIPT_DIR/../artifact_batch_apply.wasm"
echo "Built: target/wasm32-unknown-unknown/release/artifact_batch_apply.wasm"
