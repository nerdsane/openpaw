#[path = "../../common.rs"]
mod common;

use common::{
    create_session_event, get_entity, is_terminal_status, next_session_event_sequence,
    post_absolute_action, status_of, system_json_headers,
};
use temper_wasm_sdk::prelude::*;
use wasm_helpers::resolve_temper_api_url;

#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        let fields = ctx.entity_state.get("fields").cloned().unwrap_or_else(|| json!({}));
        let base_url = resolve_temper_api_url(&ctx, &fields);
        let headers = system_json_headers(&ctx, &ctx.tenant, &fields);

        let inner_session_id = common::field_string(&fields, &["InnerSessionId", "inner_session_id"]);
        if !inner_session_id.is_empty() {
            let inner = get_entity(&ctx, &base_url, &headers, "Sessions", &inner_session_id)?;
            if !is_terminal_status(&status_of(&inner)) {
                let _ = post_absolute_action(
                    &ctx,
                    &headers,
                    &format!("{base_url}/tdata/Sessions('{inner_session_id}')/OpenPaw.Cancel"),
                    &json!({}),
                    "cancel inner session",
                )?;
            }
        }

        let stop_reason = ctx
            .trigger_params
            .get("TerminationReason")
            .and_then(Value::as_str)
            .or_else(|| {
                ctx.trigger_params
                    .get("termination_reason")
                    .and_then(Value::as_str)
            })
            .unwrap_or("cancelled")
            .to_string();

        let sequence = next_session_event_sequence(&ctx, &base_url, &headers, &ctx.entity_id)?;
        let _ = create_session_event(
            &ctx,
            &base_url,
            &headers,
            &ctx.entity_id,
            sequence,
            "session.status_terminated",
            json!({ "TerminationReason": stop_reason }),
        )?;

        temper_wasm_sdk::set_success_result(
            "",
            &json!({
                "status": "terminated",
                "TerminationReason": stop_reason,
            }),
        );
        Ok(())
    })();

    if let Err(error) = result {
        temper_wasm_sdk::set_error_result(&error);
    }
    0
}
