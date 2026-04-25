#!/usr/bin/env bash
# Build all WASM modules for the paw-agent app.
# Usage: cd os-apps/paw-agent/wasm && ./build.sh
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# Standard modules (wasm32-unknown-unknown)
# Build failures are non-fatal — some modules may not compile on all targets.
FAILED_MODULES=""
copy_artifact() {
    local module="$1"
    local target="$2"
    local source_file="$SCRIPT_DIR/$module/target/$target/release/${module/-/_}.wasm"
    if [ ! -f "$source_file" ]; then
        source_file="$SCRIPT_DIR/$module/target/$target/release/$(echo "$module" | tr '_' '-').wasm"
    fi
    if [ -f "$source_file" ]; then
        cp "$source_file" "$SCRIPT_DIR/$module/$module.wasm"
    fi
}

for module in context_preparer provider_caller provider_response_applier sandbox_provisioner workspace_provisioner context_compactor steering_checker session_link_monitor coding_agent_runner cron_compute_next workspace_restorer agent_reply emit_ots_trajectory request_approval request_plan_review capability_installer plan_approval_handler plan_review_feedback_handler openai_codex_auth; do
    echo "Building $module..."
    if (cd "$SCRIPT_DIR/$module" && cargo build --target wasm32-unknown-unknown --release 2>&1); then
        copy_artifact "$module" "wasm32-unknown-unknown"
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
        copy_artifact "$module" "wasm32-wasip1"
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
for module in context_preparer provider_caller provider_response_applier sandbox_provisioner workspace_provisioner context_compactor steering_checker session_link_monitor coding_agent_runner cron_compute_next workspace_restorer agent_reply emit_ots_trajectory request_approval request_plan_review capability_installer plan_approval_handler plan_review_feedback_handler openai_codex_auth; do
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
