#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# Standard modules (wasm32-unknown-unknown)
for module in channel_connect send_reply; do
    echo "Building $module..."
    (cd "$SCRIPT_DIR/$module" && cargo build --target wasm32-unknown-unknown --release)
    echo "  -> $module built successfully"
done

# WASI modules (wasm32-wasip1) — require WASI support in Temper engine
for module in route_message; do
    echo "Building $module (WASI)..."
    (cd "$SCRIPT_DIR/$module" && cargo build --target wasm32-wasip1 --release)
    echo "  -> $module built successfully"
done

echo ""
echo "All Temper channel WASM modules built. Binaries at:"
for module in channel_connect send_reply; do
    wasm_file="$SCRIPT_DIR/$module/target/wasm32-unknown-unknown/release/${module}.wasm"
    if [ ! -f "$wasm_file" ]; then
        wasm_file="$SCRIPT_DIR/$module/target/wasm32-unknown-unknown/release/$(echo "$module" | tr '_' '-').wasm"
    fi
    if [ -f "$wasm_file" ]; then
        size=$(wc -c < "$wasm_file" | tr -d ' ')
        echo "  $module: $(( size / 1024 ))KB"
    fi
done
for module in route_message; do
    wasm_file="$SCRIPT_DIR/$module/target/wasm32-wasip1/release/${module}.wasm"
    if [ ! -f "$wasm_file" ]; then
        wasm_file="$SCRIPT_DIR/$module/target/wasm32-wasip1/release/$(echo "$module" | tr '_' '-').wasm"
    fi
    if [ -f "$wasm_file" ]; then
        size=$(wc -c < "$wasm_file" | tr -d ' ')
        echo "  $module (WASI): $(( size / 1024 ))KB"
    fi
done
