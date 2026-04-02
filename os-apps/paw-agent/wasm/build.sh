#!/usr/bin/env bash
# Build all WASM modules for the paw-agent app.
# Usage: cd os-apps/paw-agent/wasm && ./build.sh
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# Standard modules (wasm32-unknown-unknown)
for module in llm_caller sandbox_provisioner context_compactor steering_checker coding_agent_runner heartbeat_scan heartbeat_scheduler cron_trigger cron_scheduler_check cron_scheduler_heartbeat workspace_restorer agent_reply request_approval capability_installer; do
    echo "Building $module..."
    (cd "$SCRIPT_DIR/$module" && cargo build --target wasm32-unknown-unknown --release)
    echo "  -> $module built successfully"
done

# WASI modules (wasm32-wasip1) — require WASI support in Temper engine
for module in monty_repl; do
    echo "Building $module (wasip1)..."
    (cd "$SCRIPT_DIR/$module" && cargo build --target wasm32-wasip1 --release)
    echo "  -> $module built successfully"
done

echo ""
echo "All WASM modules built. Binaries at:"
for module in llm_caller sandbox_provisioner context_compactor steering_checker coding_agent_runner heartbeat_scan heartbeat_scheduler cron_trigger cron_scheduler_check cron_scheduler_heartbeat workspace_restorer agent_reply request_approval capability_installer; do
    wasm_file="$SCRIPT_DIR/$module/target/wasm32-unknown-unknown/release/${module/-/_}.wasm"
    if [ -f "$wasm_file" ]; then
        size=$(wc -c < "$wasm_file" | tr -d ' ')
        echo "  $module: $(( size / 1024 ))KB"
    else
        wasm_file="$SCRIPT_DIR/$module/target/wasm32-unknown-unknown/release/$(echo $module | tr '_' '-').wasm"
        if [ -f "$wasm_file" ]; then
            size=$(wc -c < "$wasm_file" | tr -d ' ')
            echo "  $module: $(( size / 1024 ))KB"
        fi
    fi
done
# WASI modules
for module in monty_repl; do
    wasm_file="$SCRIPT_DIR/$module/target/wasm32-wasip1/release/${module/-/_}.wasm"
    if [ -f "$wasm_file" ]; then
        size=$(wc -c < "$wasm_file" | tr -d ' ')
        echo "  $module (wasip1): $(( size / 1024 ))KB"
    fi
done
