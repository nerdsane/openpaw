//! animate_dwellers — living worlds: casting, traversals, stories
//! (ADR-004 C4). Skeleton: phases land with C4.
//!
//! Build: `cargo build --target wasm32-unknown-unknown --release`

use temper_wasm_sdk::prelude::*;

/// Entry point.
#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        ctx.log(
            "warn",
            &format!(
                "animate_dwellers: skeleton — trigger {} acknowledged, no animation yet",
                ctx.trigger_action
            ),
        );
        // A successful run with nothing to dispatch must still set a result:
        // the host treats an empty result as failure.
        set_success_result("", &json!({}));
        Ok(())
    })();
    if let Err(e) = result {
        set_error_result(&e);
    }
    0
}
