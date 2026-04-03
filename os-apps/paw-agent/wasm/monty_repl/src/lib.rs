//! Monty REPL — WASM module embedding Pydantic's Monty Python sandbox.
//!
//! Replaces tool_runner with a true persistent REPL. Agents write Python
//! code using `temper.*` and `sandbox.*` objects; Monty interprets it and
//! dispatches method calls to the Temper API or sandbox via host functions.
//!
//! REPL state (heap, globals, intern table) persists across LLM turns via
//! Monty's dump()/load() serialization, stored in the `repl_state` entity field.
//!
//! Build: `cargo build --target wasm32-wasip1 --release`

use std::collections::BTreeMap;
use std::time::Duration;

use temper_wasm_sdk::prelude::*;

mod convert;
mod datadog;
mod dispatch;
mod entity_ops;
mod railway;
mod session;
mod vercel;

use monty::{
    DictPairs, ExcType, ExtFunctionResult, LimitedTracker, MontyException, MontyObject, MontyRepl,
    PrintWriter, ReplProgress, ResourceLimits,
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

        // Load or create persistent REPL
        let repl_state_b64 = fields
            .get("repl_state")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let mut repl = load_or_create_repl(repl_state_b64, &ctx)?;

        // Execute each tool call
        let mut tool_results: Vec<Value> = Vec::new();

        for call in &tool_calls {
            let tool_id = call.get("id").and_then(|v| v.as_str()).unwrap_or("unknown");
            let input = call.get("input").cloned().unwrap_or(json!({}));
            let code = input
                .get("code")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            ctx.log("info", &format!("monty_repl: executing code for {tool_id}"));

            // Wrap code in async function
            let wrapped = wrap_user_code(code);

            // Execute via REPL start() — use Collect to capture print() output.
            let mut print = PrintWriter::Collect(String::new());
            let progress = match repl.start(&wrapped, &mut print) {
                Ok(p) => p,
                Err(e) => {
                    repl = e.repl;
                    let msg = format_monty_exception(&e.error);
                    tool_results.push(make_tool_result(tool_id, &msg, true));
                    continue;
                }
            };
            // Recover print buffer from start() call
            let start_output = match print {
                PrintWriter::Collect(buf) => buf,
                _ => String::new(),
            };

            // Drive the event loop (continues collecting print output)
            let (result, returned_repl, printed) = drive_repl_loop(
                &ctx,
                &temper_api_url,
                tenant,
                sandbox_url,
                workdir,
                progress,
                start_output,
            );
            repl = returned_repl;

            // Combine print output + expression value
            let (content, is_error) = match result {
                Ok(expr_val) => {
                    let mut combined = printed;
                    // Append expression value if it's not null/None
                    if expr_val != "null" && !expr_val.is_empty() {
                        if !combined.is_empty() {
                            combined.push('\n');
                        }
                        combined.push_str(&expr_val);
                    }
                    if combined.is_empty() {
                        combined.push_str("(no output)");
                    }
                    (truncate_output(&combined), false)
                }
                Err(e) => {
                    let mut combined = printed;
                    if !combined.is_empty() {
                        combined.push('\n');
                    }
                    combined.push_str(&e);
                    (combined, true)
                }
            };

            tool_results.push(make_tool_result(tool_id, &content, is_error));
        }

        // Save REPL state
        let saved_state = save_repl_state(&repl)?;

        // Heartbeat
        session::send_heartbeat(&ctx, &temper_api_url, tenant);

        // Persist results to session tree / conversation file / inline
        let params = session::persist_results(
            &ctx,
            &temper_api_url,
            tenant,
            &fields,
            &tool_results,
            &saved_state,
        )?;

        set_success_result("HandleToolResults", &params);
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

/// Load a persistent REPL from serialized state, or create a fresh one.
///
/// At bf7c7ef, MontyRepl::new() takes initial code + inputs and executes
/// immediately. For fresh REPLs, we run a setup snippet that makes
/// `temper` and `sandbox` objects available as globals.
fn load_or_create_repl(
    repl_state_b64: &str,
    ctx: &Context,
) -> Result<MontyRepl<LimitedTracker>, String> {
    let limits = ResourceLimits::new()
        .max_duration(Duration::from_secs(300))
        .max_memory(64 * 1024 * 1024)
        .max_allocations(250_000);
    let tracker = LimitedTracker::new(limits);

    if repl_state_b64.is_empty() {
        // Create fresh REPL with temper and sandbox objects injected
        let init_code = "pass".to_string(); // minimal init snippet
        let input_names = vec!["temper".to_string(), "sandbox".to_string()];
        let inputs = vec![
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

        let mut print = PrintWriter::Disabled;
        let (repl, _init_result) =
            MontyRepl::new(init_code, "init.py", input_names, inputs, tracker, &mut print)
                .map_err(|e| format_monty_exception(&e))?;

        ctx.log("info", "monty_repl: created fresh REPL with temper + sandbox objects");
        Ok(repl)
    } else {
        let bytes = base64_decode(repl_state_b64)?;
        MontyRepl::load(&bytes).map_err(|e| format!("failed to deserialize REPL state: {e}"))
    }
}

/// Serialize the REPL state to base64 for storage in an entity field.
fn save_repl_state(repl: &MontyRepl<LimitedTracker>) -> Result<String, String> {
    let bytes = repl
        .dump()
        .map_err(|e| format!("failed to serialize REPL state: {e}"))?;
    Ok(base64_encode(&bytes))
}

/// Drive the Monty REPL event loop to completion.
///
/// Accepts a print buffer (from the initial `start()` call) and continues
/// collecting `print()` output throughout execution. Returns:
/// - expression result (Ok/Err)
/// - the repl (for state persistence)
/// - accumulated print output string
fn drive_repl_loop(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    sandbox_url: &str,
    workdir: &str,
    mut progress: ReplProgress<LimitedTracker>,
    mut print_buf: String,
) -> (Result<String, String>, MontyRepl<LimitedTracker>, String) {
    let mut pending_results: BTreeMap<u32, ExtFunctionResult> = BTreeMap::new();

    loop {
        match progress {
            ReplProgress::Complete { repl, value } => {
                let json_value = convert::monty_object_to_json(&value);
                let result = serde_json::to_string(&json_value)
                    .map_err(|e| format!("failed to serialize result: {e}"));
                return (result, repl, print_buf);
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
                    let mut print = PrintWriter::Collect(std::mem::take(&mut print_buf));
                    match call.resume(ext_result, &mut print) {
                        Ok(p) => {
                            if let PrintWriter::Collect(s) = print { print_buf = s; }
                            progress = p; continue;
                        }
                        Err(e) => {
                            if let PrintWriter::Collect(s) = print { print_buf = s; }
                            return (Err(msg), e.repl, print_buf);
                        }
                    }
                }

                let call_id = call.call_id;
                let fn_name = call.function_name.clone();
                let args = call.args.clone();

                let (obj_name, user_args) = classify_method_call(&args);

                let json_args: Vec<Value> = user_args
                    .iter()
                    .map(|a| convert::monty_object_to_json(a))
                    .collect();

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

                // Resume with the result directly — we have it now, no need for
                // the async resume_pending() → ResolveFutures roundtrip.
                let mut print = PrintWriter::Collect(std::mem::take(&mut print_buf));
                match call.resume(ext_result, &mut print) {
                    Ok(p) => { if let PrintWriter::Collect(s) = print { print_buf = s; } progress = p; }
                    Err(e) => { if let PrintWriter::Collect(s) = print { print_buf = s; } return (Err(format_monty_exception(&e.error)), e.repl, print_buf); }
                }
            }

            ReplProgress::ResolveFutures(state) => {
                let mut ready: Vec<(u32, ExtFunctionResult)> = Vec::new();
                for call_id in state.pending_call_ids() {
                    if let Some(result) = pending_results.remove(call_id) {
                        ready.push((*call_id, result));
                    }
                }

                let mut print = PrintWriter::Collect(std::mem::take(&mut print_buf));
                match state.resume(ready, &mut print) {
                    Ok(p) => { if let PrintWriter::Collect(s) = print { print_buf = s; } progress = p; }
                    Err(e) => { if let PrintWriter::Collect(s) = print { print_buf = s; } return (Err(format_monty_exception(&e.error)), e.repl, print_buf); }
                }
            }

            ReplProgress::NameLookup(lookup) => {
                let mut print = PrintWriter::Collect(std::mem::take(&mut print_buf));
                match lookup.resume(monty::NameLookupResult::Undefined, &mut print) {
                    Ok(p) => { if let PrintWriter::Collect(s) = print { print_buf = s; } progress = p; }
                    Err(e) => { if let PrintWriter::Collect(s) = print { print_buf = s; } return (Err(format_monty_exception(&e.error)), e.repl, print_buf); }
                }
            }

            ReplProgress::OsCall(os_call) => {
                let ext_result = ExtFunctionResult::Error(MontyException::new(
                    ExcType::RuntimeError,
                    Some("sandbox blocked OS access. Use sandbox.bash() for shell commands.".into()),
                ));
                let mut print = PrintWriter::Collect(std::mem::take(&mut print_buf));
                match os_call.resume(ext_result, &mut print) {
                    Ok(p) => { if let PrintWriter::Collect(s) = print { print_buf = s; } progress = p; }
                    Err(e) => { if let PrintWriter::Collect(s) = print { print_buf = s; } return (Err(format_monty_exception(&e.error)), e.repl, print_buf); }
                }
            }
        }
    }
}

// --- Helpers ---

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

fn format_monty_exception(exception: &MontyException) -> String {
    if exception.traceback().is_empty() {
        exception.summary()
    } else {
        exception.to_string()
    }
}

fn truncate_output(output: &str) -> String {
    if output.len() > MAX_TOOL_RESULT_BYTES {
        format!(
            "{}...\n[truncated, showing {MAX_TOOL_RESULT_BYTES} of {} bytes]",
            &output[..MAX_TOOL_RESULT_BYTES],
            output.len()
        )
    } else {
        output.to_string()
    }
}

fn make_tool_result(tool_id: &str, content: &str, is_error: bool) -> Value {
    json!({
        "type": "tool_result",
        "tool_use_id": tool_id,
        "content": content,
        "is_error": is_error,
    })
}

fn base64_encode(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(data)
}

fn base64_decode(input: &str) -> Result<Vec<u8>, String> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD
        .decode(input)
        .map_err(|e| format!("base64 decode error: {e}"))
}
