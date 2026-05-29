#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/../../wasm-build-env.sh"

copy_artifact() {
    local module="$1"
    local target="$2"
    local source_file="$SCRIPT_DIR/$module/target/$target/release/${module}.wasm"
    if [ ! -f "$source_file" ]; then
        source_file="$SCRIPT_DIR/$module/target/$target/release/$(echo "$module" | tr '_' '-').wasm"
    fi
    if [ -f "$source_file" ]; then
        cp "$source_file" "$SCRIPT_DIR/$module/$module.wasm"
    fi
}

# Standard modules (wasm32-unknown-unknown)
for module in channel_connect send_reply transport_reconcile; do
    echo "Building $module..."
    (cd "$SCRIPT_DIR/$module" && cargo build --target wasm32-unknown-unknown --release)
    copy_artifact "$module" "wasm32-unknown-unknown"
    echo "  -> $module built successfully"
done

# WASI modules (wasm32-wasip1) — require WASI support in Temper engine
for module in route_message; do
    echo "Building $module (WASI)..."
    (cd "$SCRIPT_DIR/$module" && cargo build --target wasm32-wasip1 --release)
    copy_artifact "$module" "wasm32-wasip1"
    echo "  -> $module built successfully"
done

echo ""
echo "All Temper channel WASM modules built. Binaries at:"
for module in channel_connect send_reply transport_reconcile; do
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
