use monty::MontyException;
use temper_wasm_sdk::prelude::*;

use crate::output::truncate_output;

pub(crate) fn push_batch_tool_result(
    ctx: &Context,
    tool_results: &mut Vec<Value>,
    tool_id: &str,
    result: &Result<Value, String>,
) {
    match result {
        Ok(value) => {
            let expr_val = serde_json::to_string(value).unwrap_or_else(|_| "null".to_string());
            let content = if expr_val == "null" || expr_val.is_empty() {
                "(no output)".to_string()
            } else {
                truncate_output(&expr_val)
            };
            ctx.log(
                "info",
                &format!(
                    "monty_repl: batched tool completed {tool_id}, expr_bytes={}, result_bytes={}, is_error=false",
                    expr_val.len(),
                    content.len()
                ),
            );
            tool_results.push(make_tool_result(tool_id, &content, false));
        }
        Err(error) => {
            let content = truncate_output(error);
            ctx.log(
                "info",
                &format!(
                    "monty_repl: batched tool completed {tool_id}, error_bytes={}, result_bytes={}, is_error=true",
                    error.len(),
                    content.len()
                ),
            );
            tool_results.push(make_tool_result(tool_id, &content, true));
        }
    }
}

pub(crate) fn format_monty_exception(exception: &MontyException) -> String {
    if exception.traceback().is_empty() {
        exception.summary()
    } else {
        exception.to_string()
    }
}

pub(crate) fn make_tool_result(tool_id: &str, content: &str, is_error: bool) -> Value {
    json!({
        "type": "tool_result",
        "tool_use_id": tool_id,
        "content": content,
        "is_error": is_error,
    })
}

/// Create a tool result with multimodal content (text + images).
pub(crate) fn make_tool_result_multimodal(
    tool_id: &str,
    text: &str,
    media_type: &str,
    base64_data: &str,
    is_error: bool,
) -> Value {
    let mut content_blocks: Vec<Value> = Vec::new();
    if !text.is_empty() {
        content_blocks.push(json!({
            "type": "text",
            "text": text
        }));
    }
    content_blocks.push(json!({
        "type": "image",
        "source": {
            "type": "base64",
            "media_type": media_type,
            "data": base64_data
        }
    }));
    json!({
        "type": "tool_result",
        "tool_use_id": tool_id,
        "content": content_blocks,
        "is_error": is_error,
    })
}

/// Check if a JSON-serialized expression value is an image result from dispatch.
pub(crate) fn extract_image_result(expr_val: &str) -> Option<(String, String, String)> {
    let v: Value = serde_json::from_str(expr_val).ok()?;
    if v.get("__temperpaw_image")?.as_bool()? {
        let media_type = v.get("media_type")?.as_str()?.to_string();
        let base64_data = v.get("base64_data")?.as_str()?.to_string();
        let source_path = v
            .get("source_path")
            .and_then(Value::as_str)
            .unwrap_or("(image)")
            .to_string();
        Some((media_type, base64_data, source_path))
    } else {
        None
    }
}

pub(crate) fn base64_encode(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(data)
}

pub(crate) fn base64_decode(input: &str) -> Result<Vec<u8>, String> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD
        .decode(input)
        .map_err(|e| format!("base64 decode error: {e}"))
}
