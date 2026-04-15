#!/usr/bin/env bash
# Build the workspace_fs WASM module.
#
# Requires: rustup target add wasm32-unknown-unknown
# Output:   target/wasm32-unknown-unknown/release/workspace_fs.wasm
set -euo pipefail

cd "$(dirname "$0")"
cargo build --target wasm32-unknown-unknown --release

# Copy the built .wasm to a predictable sibling location for discovery
cp target/wasm32-unknown-unknown/release/workspace_fs.wasm ../workspace_fs.wasm 2>/dev/null || true
echo "Built: target/wasm32-unknown-unknown/release/workspace_fs.wasm"
