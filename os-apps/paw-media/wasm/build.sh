#!/usr/bin/env bash
# Build all paw-media WASM modules. Run from this directory:
#   cd os-apps/paw-media/wasm && bash build.sh

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

for module in openai_codex_image_generate fal_image_edit; do
    echo "Building $module..."
    (cd "$SCRIPT_DIR/$module" && cargo build --target wasm32-unknown-unknown --release)
    copy_artifact "$module" "wasm32-unknown-unknown"
    test -f "$SCRIPT_DIR/$module/$module.wasm"
    echo "  -> $module built successfully"
done

echo ""
echo "All paw-media WASM modules built. Binaries at:"
for module in openai_codex_image_generate fal_image_edit; do
    wasm_file="$SCRIPT_DIR/$module/target/wasm32-unknown-unknown/release/${module}.wasm"
    if [ ! -f "$wasm_file" ]; then
        wasm_file="$SCRIPT_DIR/$module/target/wasm32-unknown-unknown/release/$(echo "$module" | tr '_' '-').wasm"
    fi
    if [ -f "$wasm_file" ]; then
        size=$(wc -c < "$wasm_file" | tr -d ' ')
        echo "  $module: $(( size / 1024 ))KB"
    fi
done
