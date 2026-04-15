#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
FAILED_MODULES=""

copy_artifact() {
    local module="$1"
    local target="wasm32-unknown-unknown"
    local source_file="$SCRIPT_DIR/$module/target/$target/release/${module/-/_}.wasm"
    if [ ! -f "$source_file" ]; then
        source_file="$SCRIPT_DIR/$module/target/$target/release/$(echo "$module" | tr '_' '-').wasm"
    fi
    if [ -f "$source_file" ]; then
        cp "$source_file" "$SCRIPT_DIR/$module/$module.wasm"
    fi
}

for module in session_orchestrator event_emitter environment_provisioner session_terminator; do
    echo "Building $module..."
    if (cd "$SCRIPT_DIR/$module" && cargo build --target wasm32-unknown-unknown --release 2>&1); then
        copy_artifact "$module"
        echo "  -> $module built successfully"
    else
        echo "  -> $module FAILED"
        FAILED_MODULES="$FAILED_MODULES $module"
    fi
done

if [ -n "$FAILED_MODULES" ]; then
    echo ""
    echo "WARNING: These modules failed to build:$FAILED_MODULES"
fi
