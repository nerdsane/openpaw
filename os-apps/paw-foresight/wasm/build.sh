#!/usr/bin/env bash
# Build all paw-foresight WASM modules. Run from this directory:
#   cd os-apps/paw-foresight/wasm && bash build.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
source "$SCRIPT_DIR/../../wasm-build-env.sh"

MODULES=(
    "seed_world:seed_world.wasm"
    "sample_endpoints:sample_endpoints.wasm"
    "decompose_endpoint:decompose_endpoint.wasm"
    "spawn_repairers:spawn_repairers.wasm"
    "spawn_adversaries:spawn_adversaries.wasm"
    "aggregate_costs:aggregate_costs.wasm"
    "evidence_ingest:evidence_ingest.wasm"
    "register_forecasts:register_forecasts.wasm"
    "render_artifacts:render_artifacts.wasm"
    "consistency_gate:consistency_gate.wasm"
    "grade_hindcast:grade_hindcast.wasm"
    "animate_dwellers:animate_dwellers.wasm"
    "adjudicate_nodes:adjudicate_nodes.wasm"
)

copy_artifact() {
    local module="$1"
    local artifact="$2"
    local target="wasm32-unknown-unknown"
    local source_file="$SCRIPT_DIR/$module/target/$target/release/${module}.wasm"
    if [ ! -f "$source_file" ]; then
        source_file="$SCRIPT_DIR/$module/target/$target/release/$(echo "$module" | tr '_' '-').wasm"
    fi
    if [ ! -f "$source_file" ]; then
        echo "missing compiled artifact for $module" >&2
        exit 1
    fi
    cp "$source_file" "$SCRIPT_DIR/$module/$artifact"
    test -f "$SCRIPT_DIR/$module/$artifact"
}

for entry in "${MODULES[@]}"; do
    module="${entry%%:*}"
    artifact="${entry#*:}"
    echo "Building $module..."
    (cd "$SCRIPT_DIR/$module" && cargo build --target wasm32-unknown-unknown --release)
    copy_artifact "$module" "$artifact"
    echo "  -> $module built successfully"
done

echo ""
echo "All paw-foresight WASM modules built. Binaries at:"
for entry in "${MODULES[@]}"; do
    module="${entry%%:*}"
    artifact="${entry#*:}"
    wasm_file="$SCRIPT_DIR/$module/$artifact"
    size=$(wc -c < "$wasm_file" | tr -d ' ')
    echo "  $module: $(( size / 1024 ))KB"
done
