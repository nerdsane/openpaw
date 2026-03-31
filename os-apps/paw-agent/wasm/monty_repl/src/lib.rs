//! Monty REPL — WASM module embedding Pydantic's Monty Python sandbox.
//!
//! Replaces tool_runner with a true persistent REPL. Agents write Python
//! code using `temper.*` and `sandbox.*` objects; Monty interprets it and
//! dispatches method calls to the Temper API or sandbox via host functions.
//!
//! Build: `cargo build --target wasm32-wasip1 --release`

use temper_wasm_sdk::prelude::*;

mod dispatch;

// Monty Python interpreter — used for REPL execution.
use monty::{MontyRun, LimitedTracker, ResourceLimits, RunProgress, PrintWriter};

/// Entry point — invoked by the Temper WASM engine on the `run_tools` trigger.
#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        ctx.log("info", "monty_repl: starting");

        // For now, just echo back that the REPL module loaded successfully.
        // Full dispatch implementation comes next.
        let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));
        let tool_calls_json = ctx
            .trigger_params
            .get("pending_tool_calls")
            .and_then(|v| v.as_str())
            .unwrap_or("[]");

        ctx.log("info", &format!("monty_repl: received tool calls: {}", tool_calls_json));

        // Verify Monty can create a program
        let limits = ResourceLimits {
            max_allocations: Some(100_000),
            max_memory: Some(64 * 1024 * 1024),
            max_duration: Some(std::time::Duration::from_secs(30)),
            max_recursion_depth: Some(100),
            gc_interval: Some(10_000),
        };
        let _tracker = LimitedTracker::new(limits);
        ctx.log("info", "monty_repl: Monty LimitedTracker created successfully");

        // TODO: Full implementation:
        // 1. Load REPL state from entity fields (dump/load)
        // 2. Parse code from pending_tool_calls
        // 3. Run in Monty with dispatch closure for temper.*/sandbox.*
        // 4. Save REPL state back
        // 5. Return HandleToolResults

        set_success_result("HandleToolResults", &json!({
            "pending_tool_calls": "[]",
            "conversation": "",
        }));
        Ok(())
    })();

    match result {
        Ok(()) => 0,
        Err(e) => {
            set_error_result(&e);
            1
        }
    }
}
