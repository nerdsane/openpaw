#!/usr/bin/env bash
# Build all WASM modules for the paw-agent app.
# Usage: cd os-apps/paw-agent/wasm && ./build.sh
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# Standard modules (wasm32-unknown-unknown)
# Build failures are non-fatal — some modules may not compile on all targets.
FAILED_MODULES=""
for module in llm_caller sandbox_provisioner context_compactor steering_checker coding_agent_runner heartbeat_scan heartbeat_scheduler cron_compute_next cron_scheduler_check cron_scheduler_heartbeat workspace_restorer agent_reply request_approval capability_installer; do
    echo "Building $module..."
    if (cd "$SCRIPT_DIR/$module" && cargo build --target wasm32-unknown-unknown --release 2>&1); then
        echo "  -> $module built successfully"
    else
        echo "  -> $module FAILED (skipping)"
        FAILED_MODULES="$FAILED_MODULES $module"
    fi
done

# WASI modules (wasm32-wasip1) — require WASI support in Temper engine
for module in monty_repl; do
    echo "Building $module (wasip1)..."
    if (cd "$SCRIPT_DIR/$module" && cargo build --target wasm32-wasip1 --release 2>&1); then
        echo "  -> $module built successfully"
    else
        echo "  -> $module FAILED (skipping)"
        FAILED_MODULES="$FAILED_MODULES $module"
    fi
done

if [ -n "$FAILED_MODULES" ]; then
    echo ""
    echo "WARNING: These modules failed to build:$FAILED_MODULES"
fi

echo ""
echo "All WASM modules built. Binaries at:"
for module in llm_caller sandbox_provisioner context_compactor steering_checker coding_agent_runner heartbeat_scan heartbeat_scheduler cron_compute_next cron_scheduler_check cron_scheduler_heartbeat workspace_restorer agent_reply request_approval capability_installer; do
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
