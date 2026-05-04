#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

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

for module in skill_installer; do
    echo "Building $module..."
    (cd "$SCRIPT_DIR/$module" && cargo build --target wasm32-unknown-unknown --release)
    copy_artifact "$module" "wasm32-unknown-unknown"
    echo "  -> $module built successfully"
done
