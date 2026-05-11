use temper_wasm_sdk::prelude::*;

use crate::output::truncate_output;

pub(crate) fn emit_tool_call_telemetry(
    ctx: &Context,
    tool_name: &str,
    tool_call_id: &str,
    tool_arguments_json: &str,
    result: &Result<Value, String>,
    duration_ms: u64,
) -> Value {
    let success = result.is_ok();
    let result_content = match result {
        Ok(value) => {
            // Don't log full base64 image data in telemetry.
            if value
                .get("__temperpaw_image")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                let path = value
                    .get("source_path")
                    .and_then(Value::as_str)
                    .unwrap_or("?");
                let size = value
                    .get("base64_data")
                    .and_then(Value::as_str)
                    .map(|s| s.len())
                    .unwrap_or(0);
                format!("[image from {path}, base64_bytes={size}]")
            } else {
                truncate_output(&value.to_string())
            }
        }
        Err(message) => truncate_output(message),
    };
    // Successful tool dispatches are superseded by the `tool.<name>`
    // span (ADR-0037) and emitted at debug to reduce log volume.
    // Failed dispatches stay at warn so on-call still sees them in
    // default log streams.
    let log_level = if success { "debug" } else { "warn" };
    ctx.log(
        log_level,
        &format!(
            "tool dispatch complete tool_name={tool_name} tool_call_id={tool_call_id} duration_ms={duration_ms} success={success} result_preview={result_content}"
        ),
    );

    json!({
        "tool_name": tool_name,
        "tool_call_id": tool_call_id,
        "arguments": truncate_output(tool_arguments_json),
        "result": result_content,
        "duration_ms": duration_ms,
        "is_error": !success,
    })
}
