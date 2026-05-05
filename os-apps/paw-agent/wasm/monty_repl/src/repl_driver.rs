use std::collections::BTreeMap;

use monty::{
    ExcType, ExtFunctionResult, LimitedTracker, MontyException, MontyObject, MontyRepl,
    PrintWriter, ReplProgress,
};
use temper_wasm_sdk::prelude::*;

use crate::convert;
use crate::dispatch;
use crate::output::BoundedOutputCollector;
use crate::run_control::run_with_tool_progress;
use crate::session;
use crate::telemetry::emit_tool_call_telemetry;
use crate::tool_results::format_monty_exception;

/// Drive the Monty REPL event loop to completion.
///
/// Accepts a print buffer (from the initial `feed_start()` call) and continues
/// collecting `print()` output throughout execution. Returns:
/// - expression result (Ok/Err)
/// - the repl (for state persistence)
pub(crate) fn drive_repl_loop(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    sandbox_url: &str,
    workdir: &str,
    mut progress: ReplProgress<LimitedTracker>,
    print_buf: &mut BoundedOutputCollector,
) -> (
    Result<String, String>,
    MontyRepl<LimitedTracker>,
    Vec<Value>,
) {
    let mut pending_results: BTreeMap<u32, ExtFunctionResult> = BTreeMap::new();
    let mut tool_span_events = Vec::new();

    loop {
        match progress {
            ReplProgress::Complete { repl, value } => {
                let json_value = convert::monty_object_to_json(&value);
                let result = serde_json::to_string(&json_value)
                    .map_err(|e| format!("failed to serialize result: {e}"));
                return (result, repl, tool_span_events);
            }

            ReplProgress::FunctionCall(call) => {
                if !call.method_call {
                    let msg = format!(
                        "sandbox denied function call '{}'. Only temper.<method> and sandbox.<method> calls are allowed.",
                        call.function_name
                    );
                    let ext_result = ExtFunctionResult::Error(MontyException::new(
                        ExcType::RuntimeError,
                        Some(msg.clone()),
                    ));
                    let print = PrintWriter::Callback(print_buf);
                    match call.resume(ext_result, print) {
                        Ok(p) => {
                            progress = p;
                            continue;
                        }
                        Err(e) => return (Err(msg), e.repl, tool_span_events),
                    }
                }

                let fn_name = call.function_name.clone();
                let args = call.args.clone();
                let kwargs = call.kwargs.clone();

                let (obj_name, user_args) = classify_method_call(&args);

                let json_args: Vec<Value> = user_args
                    .iter()
                    .map(convert::monty_object_to_json)
                    .collect();
                let json_kwargs: Vec<(Value, Value)> = kwargs
                    .iter()
                    .map(|(k, v)| {
                        (
                            convert::monty_object_to_json(k),
                            convert::monty_object_to_json(v),
                        )
                    })
                    .collect();
                let tool_name = format!("{obj_name}.{fn_name}");
                let tool_call_id = call.call_id.to_string();
                let tool_arguments_json = if json_kwargs.is_empty() {
                    serde_json::to_string(&json_args).unwrap_or_default()
                } else {
                    serde_json::to_string(&json!({
                        "args": json_args,
                        "kwargs": json_kwargs,
                    }))
                    .unwrap_or_default()
                };
                let started_ms = Context::get_time_millis();

                let result = run_with_tool_progress(
                    |boundary| {
                        ctx.log(
                            "debug",
                            &format!(
                                "monty_repl: tool progress boundary={boundary:?} tool_name={tool_name} tool_call_id={tool_call_id}"
                            ),
                        );
                        session::send_progress(ctx, temper_api_url, tenant);
                    },
                    || {
                        dispatch::dispatch(
                            ctx,
                            temper_api_url,
                            tenant,
                            sandbox_url,
                            workdir,
                            &obj_name,
                            &fn_name,
                            Some(tool_call_id.as_str()),
                            &json_args,
                            &json_kwargs,
                        )
                    },
                );
                let duration_ms = (Context::get_time_millis() - started_ms).max(0) as u64;

                tool_span_events.push(emit_tool_call_telemetry(
                    ctx,
                    &tool_name,
                    &tool_call_id,
                    &tool_arguments_json,
                    &result,
                    duration_ms,
                ));

                let ext_result = match result {
                    Ok(value) => ExtFunctionResult::Return(convert::json_to_monty_object(&value)),
                    Err(message) => {
                        // Tool-disabled errors must surface to the LLM even if Python
                        // code catches the exception — store in DISPATCH_OUTPUT so the
                        // REPL always includes it in the tool result.
                        if message.contains("is not enabled for this session")
                            || message.contains("is not configured for this session")
                        {
                            dispatch::set_dispatch_output(&message);
                        }
                        ExtFunctionResult::Error(MontyException::new(
                            ExcType::RuntimeError,
                            Some(message),
                        ))
                    }
                };

                // Resume with the result directly — we have it now, no need for
                // the async resume_pending() → ResolveFutures roundtrip.
                let print = PrintWriter::Callback(print_buf);
                match call.resume(ext_result, print) {
                    Ok(p) => progress = p,
                    Err(e) => {
                        return (
                            Err(format_monty_exception(&e.error)),
                            e.repl,
                            tool_span_events,
                        );
                    }
                }
            }

            ReplProgress::ResolveFutures(state) => {
                let mut ready: Vec<(u32, ExtFunctionResult)> = Vec::new();
                for call_id in state.pending_call_ids() {
                    if let Some(result) = pending_results.remove(call_id) {
                        ready.push((*call_id, result));
                    }
                }

                let print = PrintWriter::Callback(print_buf);
                match state.resume(ready, print) {
                    Ok(p) => progress = p,
                    Err(e) => {
                        return (
                            Err(format_monty_exception(&e.error)),
                            e.repl,
                            tool_span_events,
                        );
                    }
                }
            }

            ReplProgress::NameLookup(lookup) => {
                let print = PrintWriter::Callback(print_buf);
                match lookup.resume(monty::NameLookupResult::Undefined, print) {
                    Ok(p) => progress = p,
                    Err(e) => {
                        return (
                            Err(format_monty_exception(&e.error)),
                            e.repl,
                            tool_span_events,
                        );
                    }
                }
            }

            ReplProgress::OsCall(os_call) => {
                let ext_result = ExtFunctionResult::Error(MontyException::new(
                    ExcType::RuntimeError,
                    Some(
                        "sandbox blocked OS access. Use sandbox.bash() for shell commands.".into(),
                    ),
                ));
                let print = PrintWriter::Callback(print_buf);
                match os_call.resume(ext_result, print) {
                    Ok(p) => progress = p,
                    Err(e) => {
                        return (
                            Err(format_monty_exception(&e.error)),
                            e.repl,
                            tool_span_events,
                        );
                    }
                }
            }
        }
    }
}

fn classify_method_call(args: &[MontyObject]) -> (String, Vec<MontyObject>) {
    if args.is_empty() {
        return ("unknown".to_string(), vec![]);
    }
    let obj_name = match &args[0] {
        MontyObject::Dataclass { name, .. } => match name.as_str() {
            "Temper" => "temper",
            "Sandbox" => "sandbox",
            "Json" => "json",
            _ => "unknown",
        },
        _ => "unknown",
    };
    (obj_name.to_string(), args[1..].to_vec())
}
