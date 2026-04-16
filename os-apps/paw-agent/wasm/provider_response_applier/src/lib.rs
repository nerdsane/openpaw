use temper_wasm_sdk::prelude::*;

#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    if let Err(err) = llm_caller::run_provider_response_applier() {
        set_error_result(&err);
    }
    0
}
