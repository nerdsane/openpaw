#!/usr/bin/env bash
# Build all WASM modules for the paw-compute app.
# Usage: cd os-apps/paw-compute/wasm && ./build.sh
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

for module in computer_exec; do
    echo "Building $module..."
    (cd "$SCRIPT_DIR/$module" && cargo build --target wasm32-unknown-unknown --release)
    copy_artifact "$module" "wasm32-unknown-unknown"
    echo "  -> $module built successfully"
done
