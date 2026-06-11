//! evidence_ingest — corridor engine module (ADR-002).
//!
//! Stub: the corridor WASM implementations land in the A3 phase under TDD.
//! Dispatching the owning action before then fails loudly by design.
//!
//! Build: `cargo build --target wasm32-unknown-unknown --release`

use temper_wasm_sdk::prelude::*;

/// Entry point.
#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        ctx.log("error", "evidence_ingest: not implemented (corridor engine A3 pending)");
        Err("evidence_ingest is not implemented yet".to_string())
    })();

    if let Err(e) = result {
        set_error_result(&e);
    }
    0
}
