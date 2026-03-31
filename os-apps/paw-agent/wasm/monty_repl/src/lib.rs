//! Monty REPL — WASM module embedding Pydantic's Monty Python sandbox.
//!
//! Replaces tool_runner with a true persistent REPL. Agents write Python
//! code using `temper.*` and `sandbox.*` objects; Monty interprets it and
//! dispatches method calls to the Temper API or sandbox via host functions.
//!
//! Build: `cargo build --target wasm32-wasip1 --release`

use std::collections::BTreeMap;
use std::time::Duration;

use temper_wasm_sdk::prelude::*;

mod convert;
mod dispatch;

use monty::{
    DictPairs, ExcType, ExtFunctionResult, LimitedTracker, MontyException, MontyObject, MontyRun,
    NameLookupResult, PrintWriter, ResourceLimits, RunProgress,
};

const MAX_TOOL_RESULT_BYTES: usize = 16 * 1024;

/// Entry point — invoked by the Temper WASM engine on the `run_tools` trigger.
#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        ctx.log("info", "monty_repl: starting");

        let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));

        let temper_api_url = ctx
            .config
            .get("temper_api_url")
            .cloned()
            .unwrap_or_else(|| "http://127.0.0.1:3000".to_string());
        let tenant = &ctx.tenant;
        let sandbox_url = fields
            .get("sandbox_url")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let workdir = fields
            .get("workdir")
            .and_then(|v| v.as_str())
            .unwrap_or("/workspace");

        // Read pending tool calls
        let tool_calls_json = ctx
            .trigger_params
            .get("pending_tool_calls")
            .and_then(|v| v.as_str())
            .unwrap_or("[]");

        let tool_calls: Vec<Value> = serde_json::from_str(tool_calls_json)
            .map_err(|e| format!("failed to parse pending_tool_calls: {e}"))?;

        ctx.log(
            "info",
            &format!("monty_repl: executing {} tool calls", tool_calls.len()),
        );

        // Execute each tool call
        let mut tool_results: Vec<Value> = Vec::new();

        for call in &tool_calls {
            let tool_id = call.get("id").and_then(|v| v.as_str()).unwrap_or("unknown");
            let input = call.get("input").cloned().unwrap_or(json!({}));

            // Extract Python code from the execute tool call
            let code = input
                .get("code")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            ctx.log("info", &format!("monty_repl: executing code for {tool_id}"));

            let result = execute_python(
                &ctx,
                &temper_api_url,
                tenant,
                sandbox_url,
                workdir,
                code,
            );

            let (content, is_error) = match result {
                Ok(output) => {
                    let truncated = if output.len() > MAX_TOOL_RESULT_BYTES {
                        format!(
                            "{}...\n[truncated, showing {MAX_TOOL_RESULT_BYTES} of {} bytes]",
                            &output[..MAX_TOOL_RESULT_BYTES],
                            output.len()
                        )
                    } else {
                        output
                    };
                    (truncated, false)
                }
                Err(e) => (e, true),
            };

            tool_results.push(json!({
                "type": "tool_result",
                "tool_use_id": tool_id,
                "content": content,
                "is_error": is_error,
            }));
        }

        // Build conversation update
        let results_json = serde_json::to_string(&tool_results)
            .map_err(|e| format!("failed to serialize tool results: {e}"))?;

        // TODO: Session tree persistence (reuse pattern from tool_runner)

        set_success_result(
            "HandleToolResults",
            &json!({
                "pending_tool_calls": "[]",
                "conversation": results_json,
            }),
        );
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

/// Execute Python code in the Monty sandbox with `temper` and `sandbox`
/// dataclass objects available.
fn execute_python(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    sandbox_url: &str,
    workdir: &str,
    code: &str,
) -> Result<String, String> {
    // Wrap user code in async function (same pattern as temper-sandbox)
    let program = wrap_user_code(code);

    // Create the MontyRun with parameter names for the injected objects
    let param_names = vec!["temper".to_string(), "sandbox".to_string()];
    let runner = MontyRun::new(program, "execute.py", param_names)
        .map_err(|e| format_monty_exception(&e))?;

    // Create dataclass objects for `temper` and `sandbox`
    let objects = vec![
        MontyObject::Dataclass {
            name: "Temper".to_string(),
            type_id: 1,
            field_names: vec![],
            attrs: DictPairs::from(Vec::<(MontyObject, MontyObject)>::new()),
            frozen: true,
        },
        MontyObject::Dataclass {
            name: "Sandbox".to_string(),
            type_id: 2,
            field_names: vec![],
            attrs: DictPairs::from(Vec::<(MontyObject, MontyObject)>::new()),
            frozen: true,
        },
    ];

    // Resource limits for this execution
    let limits = ResourceLimits::new()
        .max_duration(Duration::from_secs(300))
        .max_memory(64 * 1024 * 1024)
        .max_allocations(250_000);

    let tracker = LimitedTracker::new(limits);

    // Start the Monty interpreter
    let mut progress = {
        let mut print = PrintWriter::Disabled;
        runner
            .start(objects, tracker, &mut print)
            .map_err(|e| format_monty_exception(&e))?
    };

    // Pending function call results (for the Future/ResolveFutures pattern)
    let mut pending_results: BTreeMap<u32, ExtFunctionResult> = BTreeMap::new();

    // Event loop — drive Monty to completion
    loop {
        match progress {
            RunProgress::Complete(result) => {
                let value = convert::monty_object_to_json(&result);
                return serde_json::to_string(&value)
                    .map_err(|e| format!("failed to serialize result: {e}"));
            }

            RunProgress::FunctionCall(call) => {
                if !call.method_call {
                    return Err(format!(
                        "sandbox denied function call '{}'. Only temper.<method> and sandbox.<method> calls are allowed.",
                        call.function_name
                    ));
                }

                let call_id = call.call_id;
                let fn_name = call.function_name.clone();
                let args = call.args.clone();

                // Determine which object the method was called on
                // args[0] is self (the dataclass), remaining are user args
                let (obj_name, user_args) = classify_method_call(&args);

                // Convert MontyObject args to JSON for dispatch
                let json_args: Vec<Value> = user_args
                    .iter()
                    .map(|a| convert::monty_object_to_json(a))
                    .collect();

                // Dispatch synchronously via WASM host functions
                let result = dispatch::dispatch(
                    ctx,
                    temper_api_url,
                    tenant,
                    sandbox_url,
                    workdir,
                    &obj_name,
                    &fn_name,
                    &json_args,
                );

                let ext_result = match result {
                    Ok(value) => {
                        ExtFunctionResult::Return(convert::json_to_monty_object(&value))
                    }
                    Err(message) => ExtFunctionResult::Error(MontyException::new(
                        ExcType::RuntimeError,
                        Some(message),
                    )),
                };

                // Store result and resume with Future marker
                pending_results.insert(call_id, ext_result);
                let mut print = PrintWriter::Disabled;
                progress = call
                    .resume(ExtFunctionResult::Future(call_id), &mut print)
                    .map_err(|e| format_monty_exception(&e))?;
            }

            RunProgress::ResolveFutures(state) => {
                let mut ready: Vec<(u32, ExtFunctionResult)> = Vec::new();
                for call_id in state.pending_call_ids() {
                    if let Some(result) = pending_results.remove(call_id) {
                        ready.push((*call_id, result));
                    }
                }

                if ready.is_empty() {
                    return Err(format!(
                        "REPL waiting on unresolved calls: {:?}",
                        state.pending_call_ids()
                    ));
                }

                let mut print = PrintWriter::Disabled;
                progress = state
                    .resume(ready, &mut print)
                    .map_err(|e| format_monty_exception(&e))?;
            }

            RunProgress::NameLookup(lookup) => {
                let mut print = PrintWriter::Disabled;
                progress = lookup
                    .resume(NameLookupResult::Undefined, &mut print)
                    .map_err(|e| format_monty_exception(&e))?;
            }

            RunProgress::OsCall(os_call) => {
                return Err(format!(
                    "sandbox blocked OS access ({:?}). Use sandbox.bash() for shell commands.",
                    os_call.function
                ));
            }
        }
    }
}

/// Classify a method call to determine which dataclass it belongs to.
/// args[0] is self (the dataclass object), remaining are user arguments.
fn classify_method_call(args: &[MontyObject]) -> (String, Vec<MontyObject>) {
    if args.is_empty() {
        return ("unknown".to_string(), vec![]);
    }

    let obj_name = match &args[0] {
        MontyObject::Dataclass { name, .. } => match name.as_str() {
            "Temper" => "temper",
            "Sandbox" => "sandbox",
            _ => "unknown",
        },
        _ => "unknown",
    };

    (obj_name.to_string(), args[1..].to_vec())
}

/// Wrap user Python code into an async function for Monty execution.
fn wrap_user_code(code: &str) -> String {
    let mut out = String::from("async def __temper_user():\n");
    if code.trim().is_empty() {
        out.push_str("    return None\n");
    } else {
        for line in code.lines() {
            out.push_str("    ");
            out.push_str(line);
            out.push('\n');
        }
    }
    out.push_str("\nawait __temper_user()\n");
    out
}

/// Format a Monty exception for display.
fn format_monty_exception(exception: &MontyException) -> String {
    if exception.traceback().is_empty() {
        exception.summary()
    } else {
        exception.to_string()
    }
}
