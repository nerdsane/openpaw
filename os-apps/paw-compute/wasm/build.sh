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

# Target MUST be wasm32-wasip1 (the host wires wasi_snapshot_preview1).
# wasm32-unknown-unknown is FORBIDDEN: it links wasm-bindgen via chrono's
# wasmbind feature and fails host instantiation (__wbindgen_placeholder__ —
# the 2026-07-20 prod incident). A correct blob imports WASI and carries ZERO
# wbindgen strings — verified below before the artifact is copied.
TARGET="wasm32-wasip1"

verify_blob() {
    local wasm="$1"
    if command -v wasm-tools >/dev/null 2>&1; then
        local dump; dump="$(wasm-tools print "$wasm" 2>/dev/null)"
        local wasi wbind
        wasi="$(printf '%s' "$dump" | grep -c 'wasi_snapshot_preview1' || true)"
        wbind="$(printf '%s' "$dump" | grep -c 'wbindgen' || true)"
    else
        local wasi wbind
        wasi="$(strings "$wasm" | grep -c 'wasi_snapshot_preview1' || true)"
        wbind="$(strings "$wasm" | grep -c 'wbindgen' || true)"
    fi
    if [ "${wasi:-0}" -lt 1 ] || [ "${wbind:-1}" -ne 0 ]; then
        echo "  !! BAD BLOB: wasi_imports=$wasi wbindgen=$wbind (need wasi>=1, wbindgen==0)" >&2
        exit 1
    fi
    echo "  -> blob ok: wasi_imports=$wasi wbindgen=0"
}

for module in computer_exec computer_exec_start computer_exec_poll computer_copy_start computer_copy_poll computer_terminate; do
    echo "Building $module..."
    (cd "$SCRIPT_DIR/$module" && cargo build --target "$TARGET" --release)
    src="$SCRIPT_DIR/$module/target/$TARGET/release/${module}.wasm"
    [ -f "$src" ] || src="$SCRIPT_DIR/$module/target/$TARGET/release/$(echo "$module" | tr '_' '-').wasm"
    verify_blob "$src"
    copy_artifact "$module" "$TARGET"
    echo "  -> $module built successfully"
done
