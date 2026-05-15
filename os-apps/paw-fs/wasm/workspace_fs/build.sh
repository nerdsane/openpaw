#!/usr/bin/env bash
# Build the workspace_fs WASM module.
#
# Requires: rustup target add wasm32-unknown-unknown
# Output:   target/wasm32-unknown-unknown/release/workspace_fs.wasm
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
cd "$SCRIPT_DIR"
cargo build --target wasm32-unknown-unknown --release

# Copy the built .wasm to locations the OS-app loader and local tests can find
# after production images prune nested target directories.
cp target/wasm32-unknown-unknown/release/workspace_fs.wasm "$SCRIPT_DIR/workspace_fs.wasm"
cp target/wasm32-unknown-unknown/release/workspace_fs.wasm "$SCRIPT_DIR/../workspace_fs.wasm"
echo "Built: target/wasm32-unknown-unknown/release/workspace_fs.wasm"
