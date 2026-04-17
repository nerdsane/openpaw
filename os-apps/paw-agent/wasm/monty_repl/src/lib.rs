//! Monty REPL — WASM module embedding Pydantic's Monty Python sandbox.
//!
//! Replaces tool_runner with a true persistent REPL. Agents write Python
//! code using `temper.*` and `sandbox.*` objects; Monty interprets it and
//! dispatches method calls to the Temper API or sandbox via host functions.
//!
//! REPL state (heap, globals, intern table) persists across LLM turns via
//! Monty's dump()/load() serialization, stored in a `repl_file_id` TemperFS
//! file for the session.
//!
//! Build: `cargo build --target wasm32-wasip1 --release`

// getrandom 0.3+ requires an explicit backend for wasm32-unknown-unknown.
// ahash (via monty/jiter) uses it for hash randomisation — not crypto.
// We provide a simple LCG-based fallback so hash seeds are non-deterministic
// within a single WASM invocation.
use std::sync::atomic::{AtomicU64, Ordering};

static SEED_CTR: AtomicU64 = AtomicU64::new(0x5EED_C0DE_CAFE_F00D);

#[unsafe(no_mangle)]
unsafe extern "C" fn __getrandom_v03_custom(dest: *mut u8, len: usize) -> u32 {
    let buf = unsafe { core::slice::from_raw_parts_mut(dest, len) };
    let mut state = SEED_CTR.fetch_add(1, Ordering::Relaxed);
    for chunk in buf.chunks_mut(8) {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        let bytes = state.to_le_bytes();
        chunk.copy_from_slice(&bytes[..chunk.len()]);
    }
    0 // success
}

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
        let mut sandbox_url = fields
            .get("sandbox_url")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let workdir = fields
            .get("workdir")
            .and_then(|v| v.as_str())
            .unwrap_or("/workspace");

        // Cedar resume mode: if we're resuming after a Cedar approval, the
        // remaining tool calls and previously-completed results are stored in
        // the entity's `pending_tool_context` field (set during PauseForApproval).
        let pending_ctx_str = fields
            .get("pending_tool_context")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty());

        let is_cedar_resume = pending_ctx_str.is_some();
        let (tool_calls, mut prior_results): (Vec<Value>, Vec<Value>) =
            if let Some(ctx_json) = pending_ctx_str {
                ctx.log("info", "monty_repl: resuming from Cedar approval");
                let pause_ctx: Value = serde_json::from_str(ctx_json)
                    .map_err(|e| format!("failed to parse pending_tool_context: {e}"))?;
                let remaining = pause_ctx
                    .get("remaining_tool_calls")
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default();
                let completed = pause_ctx
                    .get("completed_results")
                    .and_then(|v| v.as_array())
                    .cloned()
                    .unwrap_or_default();
                (remaining, completed)
            } else {
                // Normal mode: read pending tool calls from trigger params
                let tool_calls_json = ctx
                    .trigger_params
                    .get("pending_tool_calls")
                    .and_then(|v| v.as_str())
                    .unwrap_or("[]");
                let calls: Vec<Value> = serde_json::from_str(tool_calls_json)
                    .map_err(|e| format!("failed to parse pending_tool_calls: {e}"))?;
                (calls, Vec::new())
            };

        ctx.log(
            "info",
            &format!(
                "monty_repl: executing {} tool calls (prior_results={})",
                tool_calls.len(),
                prior_results.len()
            ),
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
        let mut tool_span_events: Vec<Value> = Vec::new();

        // Tool-batch checkpoint boundary (OpenPaw Track 1 Phase 3 / openpaw#66).
        // Chunk size is fixed at 20; empirical fuel cost is ~400M per
        // `temper.get`, so 20 × 400M = 8B sits comfortably under the 120B
        // ceiling. Raise/lower by editing this constant; a future phase may
        // promote it to integration.config.
        const CHECKPOINT_EVERY_N: usize = 20;
        // Runaway guard: fail the session if checkpoints accumulate past this
        // threshold in a single turn (~1000 tool calls at chunk=20).
        const MAX_CHECKPOINTS_PER_TURN: u64 = 50;

        for (i, call) in tool_calls.iter().enumerate() {
            // Checkpoint check (before executing the i-th call). Skip on i=0
            // because we just entered; only fire at whole chunk boundaries and
            // only when there is actually more work after us.
            if i > 0 && i % CHECKPOINT_EVERY_N == 0 && i < tool_calls.len() {
                let current_ckpt: u64 = fields
                    .get("checkpoint_count")
                    .and_then(|v| v.as_u64().or_else(|| v.as_str().and_then(|s| s.parse().ok())))
                    .unwrap_or(0);

                if current_ckpt >= MAX_CHECKPOINTS_PER_TURN {
                    ctx.log("error", &format!(
                        "monty_repl: checkpoint runaway guard tripped at count={current_ckpt}, failing session"
                    ));
                    set_success_result(
                        "Fail",
                        &json!({
                            "error_message": format!(
                                "tool-call checkpoint budget exhausted ({current_ckpt} checkpoints in one turn; max {MAX_CHECKPOINTS_PER_TURN})"
                            ),
                        }),
                    );
                    return Ok(());
                }

                ctx.log(
                    "info",
                    &format!(
                        "monty_repl: tool-batch checkpoint at i={i}, completed={}, remaining={}, checkpoint_count={current_ckpt}",
                        prior_results.len() + tool_results.len(),
                        tool_calls.len() - i
                    ),
                );

                // Save REPL state to TemperFS so the re-entered invocation
                // restarts from the same interpreter heap.
                let saved_state = save_repl_state(&repl)?;
                let new_repl_file_id = session::save_repl_to_file(
                    &ctx,
                    &temper_api_url,
                    tenant,
                    workspace_id,
                    repl_file_id,
                    &saved_state,
                )?;

                // Build checkpoint context — same shape the Cedar pause path
                // uses, so the existing resume branch at the top of this
                // function reads it back transparently.
                let mut all_completed = prior_results.clone();
                all_completed.append(&mut tool_results);
                let remaining: Vec<Value> = tool_calls[i..].to_vec();
                let ckpt_ctx = json!({
                    "completed_results": all_completed,
                    "remaining_tool_calls": remaining,
                });

                let session_leaf_id = fields
                    .get("session_leaf_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                let params = json!({
                    "pending_tool_calls": serde_json::to_string(&remaining)
                        .unwrap_or_else(|_| "[]".to_string()),
                    "pending_tool_context": serde_json::to_string(&ckpt_ctx)
                        .unwrap_or_else(|_| "{}".to_string()),
                    "repl_file_id": new_repl_file_id,
                    "session_leaf_id": session_leaf_id,
                });

                set_success_result("CheckpointToolBatch", &params);
                return Ok(());
            }

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

            // Execute snippets directly in the shared Monty namespace so globals
            // and helper definitions survive across turns as intended.
            let snippet = code.trim();

            // Execute via REPL feed_start() using a bounded collector so pathological
            // print output cannot force runaway reallocations inside the daemon.
            let mut printed = BoundedOutputCollector::new(MAX_TOOL_RESULT_BYTES);
            let print = PrintWriter::Callback(&mut printed);
            let progress = match repl.feed_start(snippet, vec![], print) {
                Ok(p) => p,
                Err(e) => {
                    let e = *e;
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
            let (result, returned_repl, tool_events) = drive_repl_loop(
                &ctx,
                &temper_api_url,
                tenant,
                &sandbox_url,
                workdir,
                progress,
                &mut printed,
            );
            repl = returned_repl;
            tool_span_events.extend(tool_events);

            // If lazy sandbox was provisioned during this tool call, update
            // sandbox_url for subsequent tool calls in this invocation (ADR-0022).
            if let Some(url) = dispatch::peek_lazy_sandbox_url() {
                sandbox_url = url;
            }

            // --- Cedar denial: clean pause ---
            // Check the thread-local flag (set by dispatch even if Monty catches
            // the exception). If a Cedar denial occurred, save state and pause.
            if let Some(cedar_ctx_json) = dispatch::take_cedar_denial() {
                ctx.log("info", "monty_repl: Cedar denial detected, pausing for approval");

                // Parse the Cedar context to extract decision_id
                let cedar_ctx: Value = serde_json::from_str(&cedar_ctx_json)
                    .unwrap_or(json!({"decision_id": "unknown"}));
                let decision_id = cedar_ctx
                    .get("decision_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");

                // Save REPL state before pausing
                let saved_state = save_repl_state(&repl)?;
                let repl_file_id = session::save_repl_to_file(
                    &ctx,
                    &temper_api_url,
                    tenant,
                    workspace_id,
                    repl_file_id,
                    &saved_state,
                )?;

                // Build pause context: all completed results (prior + current) +
                // remaining tool calls (current denied call + any after it)
                let mut all_completed = prior_results.clone();
                all_completed.append(&mut tool_results);
                let remaining: Vec<Value> = tool_calls[i..].to_vec();

                let pause_ctx = json!({
                    "completed_results": all_completed,
                    "remaining_tool_calls": remaining,
                    "tool_context": cedar_ctx,
                });

                // Dispatch PauseForApproval — Session transitions to WaitingForApproval
                let mut params = json!({
                    "pending_decision_id": decision_id,
                    "pending_tool_context": serde_json::to_string(&pause_ctx)
                        .unwrap_or_else(|_| "{}".to_string()),
                    "repl_file_id": repl_file_id,
                });

                // Preserve sandbox state if lazily provisioned (ADR-0022)
                if let Some((url, id, provider)) = dispatch::take_lazy_sandbox() {
                    params["sandbox_url"] = json!(url);
                    params["sandbox_id"] = json!(id);
                    params["sandbox_provider"] = json!(provider);
                }

                set_success_result("PauseForApproval", &params);
                return Ok(());
            }

            let printed = printed.into_string();

            // Combine print output + expression value
            match result {
                Ok(expr_val) => {
                    // Check if the expression value is an image from sandbox.read()
                    if let Some((media_type, base64_data, source_path)) =
                        extract_image_result(&expr_val)
                    {
                        let mut text = printed;
                        if !text.is_empty() {
                            text.push('\n');
                        }
                        text.push_str(&format!("[Image read from {source_path}]"));
                        ctx.log(
                            "info",
                            &format!(
                                "monty_repl: tool completed {tool_id}, image from {source_path}, base64_bytes={}, is_error=false",
                                base64_data.len()
                            ),
                        );
                        tool_results.push(make_tool_result_multimodal(
                            tool_id,
                            &text,
                            &media_type,
                            &base64_data,
                            false,
                        ));
                    } else {
                        let expr_len = expr_val.len();
                        let mut combined = printed;
                        // Append expression value if it's not null/None
                        if expr_val != "null" && !expr_val.is_empty() {
                            if !combined.is_empty() {
                                combined.push('\n');
                            }
                            combined.push_str(&expr_val);
                        }
                        // If the dispatch function stored important output (e.g.
                        // submit_specs success message), always surface it — even
                        // if Python printed something, the dispatch message is the
                        // authoritative result the LLM needs to see.
                        if let Some(dispatch_msg) = dispatch::take_dispatch_output() {
                            if combined.is_empty() {
                                combined.push_str(&dispatch_msg);
                            } else {
                                combined.push('\n');
                                combined.push_str(&dispatch_msg);
                            }
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
                        tool_results.push(make_tool_result(tool_id, &content, false));
                    }
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
                    tool_results.push(make_tool_result(tool_id, &content, true));
                }
            };
        }

        // Merge prior results from Cedar resume (if any) with newly collected results
        if !prior_results.is_empty() {
            prior_results.append(&mut tool_results);
            tool_results = prior_results;
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
        let mut params = session::persist_results(
            &ctx,
            &temper_api_url,
            tenant,
            &fields,
            &tool_results,
            &repl_file_id,
        )?;

        // If a sandbox was lazily provisioned during this invocation,
        // include it in the callback params so it persists to entity state (ADR-0022).
        if let Some((url, id, provider)) = dispatch::take_lazy_sandbox() {
            params["sandbox_url"] = json!(url);
            params["sandbox_id"] = json!(id);
            params["sandbox_provider"] = json!(provider);
        }
        if !tool_span_events.is_empty() {
            let existing_tool_spans_file_id = fields
                .get("tool_spans_file_id")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            match session::append_tool_spans_to_file(
                &ctx,
                &temper_api_url,
                tenant,
                workspace_id,
                existing_tool_spans_file_id,
                &tool_span_events,
            ) {
                Ok(id) if !id.is_empty() => {
                    params["tool_spans_file_id"] = json!(id);
                }
                Ok(_) => {}
                Err(e) => ctx.log(
                    "warn",
                    &format!("monty_repl: tool_spans append failed: {e}"),
                ),
            }
        }

        // Clear Cedar approval state after successful resume so the next
        // run_tools invocation doesn't erroneously re-enter resume mode.
        if is_cedar_resume {
            params["pending_tool_context"] = json!("");
            params["pending_decision_id"] = json!("");
        }

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
/// At c9802b5 (v0.0.9), MontyRepl::new() creates a bare REPL; globals are
/// injected via feed_start() which we drive to completion.
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
        // Create bare REPL, then inject temper + sandbox globals via feed_start
        let repl = MontyRepl::new("init.py", tracker);

        let inputs = vec![
            (
                "temper".to_string(),
                MontyObject::Dataclass {
                    name: "Temper".to_string(),
                    type_id: 1,
                    field_names: vec![],
                    attrs: DictPairs::from(Vec::<(MontyObject, MontyObject)>::new()),
                    frozen: true,
                },
            ),
            (
                "sandbox".to_string(),
                MontyObject::Dataclass {
                    name: "Sandbox".to_string(),
                    type_id: 2,
                    field_names: vec![],
                    attrs: DictPairs::from(Vec::<(MontyObject, MontyObject)>::new()),
                    frozen: true,
                },
            ),
        ];

        let print = PrintWriter::Disabled;
        let progress = repl
            .feed_start("pass", inputs, print)
            .map_err(|e| format_monty_exception(&e.error))?;

        let repl = drive_init_to_completion(progress)?;

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

/// Drive a simple init snippet (just "pass") to completion, recovering the REPL.
fn drive_init_to_completion(
    mut progress: ReplProgress<LimitedTracker>,
) -> Result<MontyRepl<LimitedTracker>, String> {
    loop {
        match progress {
            ReplProgress::Complete { repl, .. } => return Ok(repl),
            ReplProgress::FunctionCall(call) => {
                let ext_result = ExtFunctionResult::Error(MontyException::new(
                    ExcType::RuntimeError,
                    Some("function calls not allowed during init".into()),
                ));
                let print = PrintWriter::Disabled;
                progress = call
                    .resume(ext_result, print)
                    .map_err(|e| format_monty_exception(&e.error))?;
            }
            ReplProgress::ResolveFutures(state) => {
                let print = PrintWriter::Disabled;
                progress = state
                    .resume(vec![], print)
                    .map_err(|e| format_monty_exception(&e.error))?;
            }
            ReplProgress::NameLookup(lookup) => {
                let print = PrintWriter::Disabled;
                progress = lookup
                    .resume(monty::NameLookupResult::Undefined, print)
                    .map_err(|e| format_monty_exception(&e.error))?;
            }
            ReplProgress::OsCall(os_call) => {
                let ext_result = ExtFunctionResult::Error(MontyException::new(
                    ExcType::RuntimeError,
                    Some("OS calls not allowed during init".into()),
                ));
                let print = PrintWriter::Disabled;
                progress = os_call
                    .resume(ext_result, print)
                    .map_err(|e| format_monty_exception(&e.error))?;
            }
        }
    }
}

/// Serialize the REPL state to base64 for storage in a TemperFS file.
fn save_repl_state(repl: &MontyRepl<LimitedTracker>) -> Result<String, String> {
    let bytes = repl
        .dump()
        .map_err(|e| format!("failed to serialize REPL state: {e}"))?;
    Ok(base64_encode(&bytes))
}

/// Drive the Monty REPL event loop to completion.
///
/// Accepts a print buffer (from the initial `feed_start()` call) and continues
/// collecting `print()` output throughout execution. Returns:
/// - expression result (Ok/Err)
/// - the repl (for state persistence)
fn drive_repl_loop(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    sandbox_url: &str,
    workdir: &str,
    mut progress: ReplProgress<LimitedTracker>,
    print_buf: &mut BoundedOutputCollector,
) -> (Result<String, String>, MontyRepl<LimitedTracker>, Vec<Value>) {
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

                let _call_id = call.call_id;
                let fn_name = call.function_name.clone();
                let args = call.args.clone();

                let (obj_name, user_args) = classify_method_call(&args);

                let json_args: Vec<Value> = user_args
                    .iter()
                    .map(|a| convert::monty_object_to_json(a))
                    .collect();
                let tool_name = format!("{obj_name}.{fn_name}");
                let tool_call_id = call.call_id.to_string();
                let tool_arguments_json = serde_json::to_string(&json_args).unwrap_or_default();
                let started_ms = Context::get_time_millis();

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
                        return (Err(format_monty_exception(&e.error)), e.repl, tool_span_events);
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
                        return (Err(format_monty_exception(&e.error)), e.repl, tool_span_events);
                    }
                }
            }

            ReplProgress::NameLookup(lookup) => {
                let print = PrintWriter::Callback(print_buf);
                match lookup.resume(monty::NameLookupResult::Undefined, print) {
                    Ok(p) => progress = p,
                    Err(e) => {
                        return (Err(format_monty_exception(&e.error)), e.repl, tool_span_events);
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
                        return (Err(format_monty_exception(&e.error)), e.repl, tool_span_events);
                    }
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

/// Create a tool result with multimodal content (text + images).
fn make_tool_result_multimodal(
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
fn extract_image_result(expr_val: &str) -> Option<(String, String, String)> {
    let v: Value = serde_json::from_str(expr_val).ok()?;
    if v.get("__openpaw_image")?.as_bool()? {
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

fn emit_tool_call_telemetry(
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
            // Don't log full base64 image data in telemetry
            if value
                .get("__openpaw_image")
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
    let log_level = if success { "info" } else { "warn" };
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
