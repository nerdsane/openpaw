//! Heartbeat Typing — sends a Discord typing indicator on each heartbeat.
//!
//! Keeps the "typing..." indicator alive while the agent is working.
//! Best-effort: silently ignores all errors.

use temper_wasm_sdk::prelude::*;
use wasm_helpers::{resolve_temper_api_url, send_typing_indicator};

#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let Ok(ctx) = Context::from_host() else {
        return 0;
    };
    let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));
    let temper_api_url = resolve_temper_api_url(&ctx, &fields);
    send_typing_indicator(&ctx, &temper_api_url, &ctx.tenant, &ctx.entity_id);
    0
}
