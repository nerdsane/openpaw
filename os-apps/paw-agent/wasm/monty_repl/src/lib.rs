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

use std::borrow::Cow;
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
    PrintWriter, PrintWriterCallback, ReplProgress, ResourceLimits,
};

const MAX_TOOL_RESULT_BYTES: usize = 16 * 1024;

/// Captures `print()` output without allowing Monty to grow an unbounded host-side buffer.
///
/// The original implementation used `PrintWriter::Collect(String)` and only truncated after
/// execution completed. Large or pathological output therefore forced repeated reallocations
/// inside the daemon and could balloon RSS before we ever returned a tool result.
struct BoundedOutputCollector {
    buf: String,
    max_bytes: usize,
    total_bytes_seen: usize,
    truncated: bool,
}

impl BoundedOutputCollector {
    fn new(max_bytes: usize) -> Self {
        Self {
            buf: String::with_capacity(max_bytes.min(1024)),
            max_bytes,
            total_bytes_seen: 0,
            truncated: false,
        }
    }

    fn append_str(&mut self, output: &str) {
        self.total_bytes_seen += output.len();
        if self.buf.len() >= self.max_bytes {
            self.truncated = true;
            return;
        }

        let remaining = self.max_bytes - self.buf.len();
        if output.len() <= remaining {
            self.buf.push_str(output);
            return;
        }

        self.truncated = true;
        self.buf
            .push_str(prefix_at_char_boundary(output, remaining));
    }

    fn append_char(&mut self, ch: char) {
        self.total_bytes_seen += ch.len_utf8();
        if self.buf.len() >= self.max_bytes {
            self.truncated = true;
            return;
        }

        if self.buf.len() + ch.len_utf8() <= self.max_bytes {
            self.buf.push(ch);
        } else {
            self.truncated = true;
        }
    }

    fn into_string(self) -> String {
        if !self.truncated {
            return self.buf;
        }

        let captured_bytes = self.buf.len();
        let total_bytes_seen = self.total_bytes_seen;
        let buf = self.buf;
        format!(
            "{}...\n[print output truncated, captured {} of {} bytes]",
            buf, captured_bytes, total_bytes_seen,
        )
    }
}

impl PrintWriterCallback for BoundedOutputCollector {
    fn stdout_write(&mut self, output: Cow<'_, str>) -> Result<(), MontyException> {
        self.append_str(&output);
        Ok(())
    }

    fn stdout_push(&mut self, end: char) -> Result<(), MontyException> {
        self.append_char(end);
        Ok(())
    }
}

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

        // Load REPL state from TemperFS file (not from entity fields — avoids
        // context JSON bloat that causes WASM memory exhaustion after ~26 turns).
        let workspace_id = fields
            .get("workspace_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let repl_file_id = fields
            .get("repl_file_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let repl_state_b64 = if !repl_file_id.is_empty() {
            session::read_temperfs_file_safe(&ctx, &temper_api_url, tenant, repl_file_id)
                .unwrap_or_default()
        } else {
            // Fallback: check entity field for backward compatibility
            fields
                .get("repl_state")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string()
        };
        ctx.log(
            "info",
            &format!(
                "monty_repl: loaded repl state bytes={} from_file={}",
                repl_state_b64.len(),
                !repl_file_id.is_empty()
            ),
        );

        let mut repl = load_or_create_repl(&repl_state_b64, &ctx)?;

        // Execute each tool call
        let mut tool_results: Vec<Value> = Vec::new();

        for call in &tool_calls {
            let tool_id = call.get("id").and_then(|v| v.as_str()).unwrap_or("unknown");
            let input = call.get("input").cloned().unwrap_or(json!({}));
            let code = input.get("code").and_then(|v| v.as_str()).unwrap_or("");

            ctx.log(
                "info",
                &format!(
                    "monty_repl: executing code for {tool_id}, code_bytes={}",
                    code.len()
                ),
            );

            // Wrap code in async function
            let wrapped = wrap_user_code(code);

            // Execute via REPL start() using a bounded collector so pathological
            // print output cannot force runaway reallocations inside the daemon.
            let mut printed = BoundedOutputCollector::new(MAX_TOOL_RESULT_BYTES);
            let mut print = PrintWriter::Callback(&mut printed);
            let progress = match repl.start(&wrapped, &mut print) {
                Ok(p) => p,
                Err(e) => {
                    repl = e.repl;
                    let mut combined = printed.into_string();
                    let msg = format_monty_exception(&e.error);
                    if !combined.is_empty() {
                        combined.push('\n');
                    }
                    combined.push_str(&msg);
                    tool_results.push(make_tool_result(tool_id, &truncate_output(&combined), true));
                    continue;
                }
            };

            // Drive the event loop (continues collecting print output)
            let (result, returned_repl) = drive_repl_loop(
                &ctx,
                &temper_api_url,
                tenant,
                sandbox_url,
                workdir,
                progress,
                &mut printed,
            );
            repl = returned_repl;
            let printed = printed.into_string();

            // Combine print output + expression value
            let (content, is_error) = match result {
                Ok(expr_val) => {
                    let expr_len = expr_val.len();
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
                    let content = truncate_output(&combined);
                    ctx.log(
                        "info",
                        &format!(
                            "monty_repl: tool completed {tool_id}, printed_bytes={}, expr_bytes={}, result_bytes={}, is_error=false",
                            combined.len().saturating_sub(expr_len),
                            expr_len,
                            content.len()
                        ),
                    );
                    (content, false)
                }
                Err(e) => {
                    let error_len = e.len();
                    let mut combined = printed;
                    if !combined.is_empty() {
                        combined.push('\n');
                    }
                    combined.push_str(&e);
                    let content = truncate_output(&combined);
                    ctx.log(
                        "info",
                        &format!(
                            "monty_repl: tool completed {tool_id}, printed_bytes={}, error_bytes={}, result_bytes={}, is_error=true",
                            combined.len().saturating_sub(error_len),
                            error_len,
                            content.len()
                        ),
                    );
                    (content, true)
                }
            };

            tool_results.push(make_tool_result(tool_id, &content, is_error));
        }

        // Save REPL state to TemperFS file (not entity field)
        let saved_state = save_repl_state(&repl)?;
        ctx.log(
            "info",
            &format!(
                "monty_repl: saving repl state bytes={}, tool_results={}",
                saved_state.len(),
                tool_results.len()
            ),
        );
        let repl_file_id = session::save_repl_to_file(
            &ctx,
            &temper_api_url,
            tenant,
            workspace_id,
            repl_file_id,
            &saved_state,
        )?;

        // Heartbeat
        session::send_heartbeat(&ctx, &temper_api_url, tenant);

        // Check if agent signaled completion via temper.done(result)
        let done_result = dispatch::take_done_result();

        // Persist results (without repl_state in entity params)
        let params = session::persist_results(
            &ctx,
            &temper_api_url,
            tenant,
            &fields,
            &tool_results,
            &repl_file_id,
        )?;

        if let Some(result_text) = done_result {
            // Agent called temper.done() — complete the session
            let mut done_params = params.clone();
            done_params["result"] = json!(result_text);
            set_success_result("RecordResult", &done_params);
        } else {
            set_success_result("HandleToolResults", &params);
        }
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
        let (repl, _init_result) = MontyRepl::new(
            init_code,
            "init.py",
            input_names,
            inputs,
            tracker,
            &mut print,
        )
        .map_err(|e| format_monty_exception(&e))?;

        ctx.log(
            "info",
            "monty_repl: created fresh REPL with temper + sandbox objects",
        );
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
    print_buf: &mut BoundedOutputCollector,
) -> (Result<String, String>, MontyRepl<LimitedTracker>) {
    let mut pending_results: BTreeMap<u32, ExtFunctionResult> = BTreeMap::new();

    loop {
        match progress {
            ReplProgress::Complete { repl, value } => {
                let json_value = convert::monty_object_to_json(&value);
                let result = serde_json::to_string(&json_value)
                    .map_err(|e| format!("failed to serialize result: {e}"));
                return (result, repl);
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
                    let mut print = PrintWriter::Callback(print_buf);
                    match call.resume(ext_result, &mut print) {
                        Ok(p) => {
                            progress = p;
                            continue;
                        }
                        Err(e) => return (Err(msg), e.repl),
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
                    Ok(value) => ExtFunctionResult::Return(convert::json_to_monty_object(&value)),
                    Err(message) => ExtFunctionResult::Error(MontyException::new(
                        ExcType::RuntimeError,
                        Some(message),
                    )),
                };

                // Resume with the result directly — we have it now, no need for
                // the async resume_pending() → ResolveFutures roundtrip.
                let mut print = PrintWriter::Callback(print_buf);
                match call.resume(ext_result, &mut print) {
                    Ok(p) => progress = p,
                    Err(e) => return (Err(format_monty_exception(&e.error)), e.repl),
                }
            }

            ReplProgress::ResolveFutures(state) => {
                let mut ready: Vec<(u32, ExtFunctionResult)> = Vec::new();
                for call_id in state.pending_call_ids() {
                    if let Some(result) = pending_results.remove(call_id) {
                        ready.push((*call_id, result));
                    }
                }

                let mut print = PrintWriter::Callback(print_buf);
                match state.resume(ready, &mut print) {
                    Ok(p) => progress = p,
                    Err(e) => return (Err(format_monty_exception(&e.error)), e.repl),
                }
            }

            ReplProgress::NameLookup(lookup) => {
                let mut print = PrintWriter::Callback(print_buf);
                match lookup.resume(monty::NameLookupResult::Undefined, &mut print) {
                    Ok(p) => progress = p,
                    Err(e) => return (Err(format_monty_exception(&e.error)), e.repl),
                }
            }

            ReplProgress::OsCall(os_call) => {
                let ext_result = ExtFunctionResult::Error(MontyException::new(
                    ExcType::RuntimeError,
                    Some(
                        "sandbox blocked OS access. Use sandbox.bash() for shell commands.".into(),
                    ),
                ));
                let mut print = PrintWriter::Callback(print_buf);
                match os_call.resume(ext_result, &mut print) {
                    Ok(p) => progress = p,
                    Err(e) => return (Err(format_monty_exception(&e.error)), e.repl),
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
        let shown = prefix_at_char_boundary(output, MAX_TOOL_RESULT_BYTES);
        format!(
            "{}...\n[truncated, showing {} of {} bytes]",
            shown,
            shown.len(),
            output.len()
        )
    } else {
        output.to_string()
    }
}

fn prefix_at_char_boundary(input: &str, max_bytes: usize) -> &str {
    if input.len() <= max_bytes {
        return input;
    }
    if max_bytes == 0 {
        return "";
    }

    let mut end = 0;
    for (idx, ch) in input.char_indices() {
        let next = idx + ch.len_utf8();
        if next > max_bytes {
            break;
        }
        end = next;
    }
    &input[..end]
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
