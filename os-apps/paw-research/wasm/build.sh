#!/usr/bin/env bash
# Build all paw-research WASM modules. Run from this directory:
#   cd os-apps/paw-research/wasm && bash build.sh
#
# These modules are declared in ../app.toml; both files must stay in sync
# or the installer will silently skip them (see app.toml comment).

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

for module in web_search web_fetch; do
    echo "Building $module..."
    (cd "$SCRIPT_DIR/$module" && cargo build --target wasm32-unknown-unknown --release)
    copy_artifact "$module" "wasm32-unknown-unknown"
    echo "  -> $module built successfully"
done

echo ""
echo "All paw-research WASM modules built. Binaries at:"
for module in web_search web_fetch; do
    wasm_file="$SCRIPT_DIR/$module/target/wasm32-unknown-unknown/release/${module}.wasm"
    if [ ! -f "$wasm_file" ]; then
        wasm_file="$SCRIPT_DIR/$module/target/wasm32-unknown-unknown/release/$(echo "$module" | tr '_' '-').wasm"
    fi
    if [ -f "$wasm_file" ]; then
        size=$(wc -c < "$wasm_file" | tr -d ' ')
        echo "  $module: $(( size / 1024 ))KB"
    fi
done
