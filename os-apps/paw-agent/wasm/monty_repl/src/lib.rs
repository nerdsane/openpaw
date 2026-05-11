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

use temper_wasm_sdk::prelude::*;

mod convert;
mod datadog;
mod dispatch;
mod entity_ops;
mod output;
mod railway;
mod repl_driver;
mod repl_state;
mod run_control;
mod session;
mod telemetry;
mod tool_results;
mod vercel;
mod wasm_random;

use monty::PrintWriter;
use output::{BoundedOutputCollector, MAX_TOOL_RESULT_BYTES, truncate_output};
use repl_driver::drive_repl_loop;
use repl_state::{
    load_or_create_repl, normal_repl_state_max_bytes, persist_tool_spans_file, read_repl_state_b64,
    save_repl_state,
};
use run_control::{
    INVARIANT_VIOLATION_MSG, RunOutcome, action_dispatched, batch_window_len, classify_run_outcome,
    dispatch_error, dispatch_success, reset_action_dispatched, run_with_tool_progress,
};
use telemetry::emit_tool_call_telemetry;
use tool_results::{
    extract_image_result, format_monty_exception, make_tool_result, make_tool_result_multimodal,
    push_batch_tool_result,
};

fn batchable_run_len(tool_calls: &[Value], start_index: usize, max_batch_len: usize) -> usize {
    let upper_bound = tool_calls
        .len()
        .min(start_index.saturating_add(max_batch_len));
    let mut run_len = 0usize;
    for call in &tool_calls[start_index..upper_bound] {
        let code = call
            .get("input")
            .and_then(|input| input.get("code"))
            .and_then(Value::as_str)
            .unwrap_or("");
        if dispatch::batchable_tool_plan_from_code(code).is_none() {
            break;
        }
        run_len += 1;
    }
    run_len
}

fn collect_batchable_tool_calls(
    tool_calls: &[Value],
    start_index: usize,
    run_len: usize,
) -> Vec<dispatch::BatchableToolCall> {
    tool_calls[start_index..start_index + run_len]
        .iter()
        .filter_map(|call| {
            let tool_call_id = call.get("id").and_then(Value::as_str)?;
            let code = call
                .get("input")
                .and_then(|input| input.get("code"))
                .and_then(Value::as_str)?;
            let plan = dispatch::batchable_tool_plan_from_code(code)?;
            Some(dispatch::BatchableToolCall {
                tool_call_id: tool_call_id.to_string(),
                plan,
            })
        })
        .collect()
}

/// Entry point — invoked by the Temper WASM engine on the `run_tools` trigger.
#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    // Reset the invariant flag. Must come before anything that could call
    // dispatch_success / dispatch_error so a prior invocation's state
    // doesn't leak into this one.
    reset_action_dispatched();
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
        let repl_state_b64 = read_repl_state_b64(&ctx, &fields, &temper_api_url, tenant);
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

        // Tool-batch checkpoint boundary (TemperPaw Track 1 Phase 3 / temperpaw#66).
        // Chunk size is fixed at 20; empirical fuel cost is ~400M per
        // `temper.get`, so 20 × 400M = 8B sits comfortably under the 120B
        // ceiling. Raise/lower by editing this constant; a future phase may
        // promote it to integration.config.
        const CHECKPOINT_EVERY_N: usize = 20;
        // Runaway guard: fail the session if checkpoints accumulate past this
        // threshold in a single turn (~1000 tool calls at chunk=20).
        const MAX_CHECKPOINTS_PER_TURN: u64 = 50;

        let mut i = 0usize;
        while i < tool_calls.len() {
            // Checkpoint check (before executing the i-th call). Skip on i=0
            // because we just entered; only fire at whole chunk boundaries and
            // only when there is actually more work after us.
            if i > 0 && i % CHECKPOINT_EVERY_N == 0 && i < tool_calls.len() {
                let current_ckpt: u64 = fields
                    .get("checkpoint_count")
                    .and_then(|v| {
                        v.as_u64()
                            .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
                    })
                    .unwrap_or(0);

                if current_ckpt >= MAX_CHECKPOINTS_PER_TURN {
                    ctx.log("error", &format!(
                        "monty_repl: checkpoint runaway guard tripped at count={current_ckpt}, failing session"
                    ));
                    dispatch_success(
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
                // Graceful: if save fails, skip this checkpoint — the agent
                // gets a fresh REPL on resume but the session stays alive.
                let saved_state = match save_repl_state(&repl) {
                    Ok(s) => s,
                    Err(e) => {
                        ctx.log(
                            "warn",
                            &format!(
                                "monty_repl: checkpoint repl save failed, skipping checkpoint: {e}"
                            ),
                        );
                        i += 1;
                        continue;
                    }
                };
                let new_repl_file_id = match session::save_repl_to_file(
                    &ctx,
                    &temper_api_url,
                    tenant,
                    workspace_id,
                    repl_file_id,
                    &saved_state,
                ) {
                    Ok(id) => id,
                    Err(e) => {
                        ctx.log(
                            "warn",
                            &format!(
                                "monty_repl: checkpoint file save failed, skipping checkpoint: {e}"
                            ),
                        );
                        i += 1;
                        continue;
                    }
                };

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

                dispatch_success("CheckpointToolBatch", &params);
                return Ok(());
            }

            let max_batch_len = batch_window_len(i, tool_calls.len(), CHECKPOINT_EVERY_N);
            let batch_len = batchable_run_len(&tool_calls, i, max_batch_len);
            if batch_len >= 2 {
                let batch_calls = collect_batchable_tool_calls(&tool_calls, i, batch_len);
                ctx.log(
                    "info",
                    &format!(
                        "monty_repl: batching {batch_len} read-only tool calls starting at i={i}"
                    ),
                );

                let batch_started_ms = Context::get_time_millis();
                let batch_results = run_with_tool_progress(
                    |boundary| {
                        ctx.log(
                            "debug",
                            &format!(
                                "monty_repl: tool progress boundary={boundary:?} tool_name=batch tool_call_id=batch:{i}"
                            ),
                        );
                        session::send_progress(&ctx, &temper_api_url, tenant);
                    },
                    || {
                        dispatch::execute_batchable_tool_calls(
                            &ctx,
                            &temper_api_url,
                            tenant,
                            &batch_calls,
                            || session::send_progress(&ctx, &temper_api_url, tenant),
                        )
                    },
                );

                for (offset, result) in batch_results.into_iter().enumerate() {
                    let call = &tool_calls[i + offset];
                    let tool_id = call.get("id").and_then(|v| v.as_str()).unwrap_or("unknown");
                    let input = call.get("input").cloned().unwrap_or(json!({}));
                    let duration_ms = (Context::get_time_millis() - batch_started_ms).max(0) as u64;
                    let tool_arguments_json = serde_json::to_string(&input).unwrap_or_default();

                    tool_span_events.push(emit_tool_call_telemetry(
                        &ctx,
                        &batch_calls[offset].plan.tool_name,
                        &batch_calls[offset].tool_call_id,
                        &tool_arguments_json,
                        &result,
                        duration_ms,
                    ));
                    push_batch_tool_result(&ctx, &mut tool_results, tool_id, &result);
                }

                i += batch_len;
                continue;
            }

            let call = &tool_calls[i];

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
                    i += 1;
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
                ctx.log(
                    "info",
                    "monty_repl: Cedar denial detected, pausing for approval",
                );

                // Parse the Cedar context to extract decision_id
                let cedar_ctx: Value = serde_json::from_str(&cedar_ctx_json)
                    .unwrap_or(json!({"decision_id": "unknown"}));
                let decision_id = cedar_ctx
                    .get("decision_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");

                // Save REPL state before pausing.
                // Graceful: if save fails, agent gets a fresh REPL on resume.
                let repl_file_id = match save_repl_state(&repl).and_then(|saved_state| {
                    session::save_repl_to_file(
                        &ctx,
                        &temper_api_url,
                        tenant,
                        workspace_id,
                        repl_file_id,
                        &saved_state,
                    )
                }) {
                    Ok(id) => id,
                    Err(e) => {
                        ctx.log("warn", &format!(
                            "monty_repl: Cedar pause repl save failed (agent gets fresh REPL on resume): {e}"
                        ));
                        repl_file_id.to_string()
                    }
                };

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

                dispatch_success("PauseForApproval", &params);
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

            i += 1;
        }

        // Merge prior results from Cedar resume (if any) with newly collected results
        if !prior_results.is_empty() {
            prior_results.append(&mut tool_results);
            tool_results = prior_results;
        }

        // Save small REPL states between provider turns, but do not route the
        // normal hot path through a large versioned TemperFS rewrite. Checkpoint
        // and approval pauses still persist state above because they need exact
        // mid-batch recovery.
        // Graceful: REPL state save failure should not kill the session.
        let repl_file_id = match save_repl_state(&repl) {
            Ok(saved_state) => {
                ctx.log(
                    "info",
                    &format!(
                        "monty_repl: saving repl state bytes={}, tool_results={}",
                        saved_state.len(),
                        tool_results.len()
                    ),
                );
                let max_normal_repl_state_bytes = normal_repl_state_max_bytes(&ctx);
                if max_normal_repl_state_bytes > 0
                    && saved_state.len() <= max_normal_repl_state_bytes
                {
                    match session::save_repl_to_file(
                        &ctx,
                        &temper_api_url,
                        tenant,
                        workspace_id,
                        repl_file_id,
                        &saved_state,
                    ) {
                        Ok(id) => id,
                        Err(e) => {
                            ctx.log(
                                "warn",
                                &format!("monty_repl: end-of-batch file save failed: {e}"),
                            );
                            repl_file_id.to_string()
                        }
                    }
                } else {
                    ctx.log(
                        "warn",
                        &format!(
                            "monty_repl: skipping normal repl state persist bytes={} max_bytes={}",
                            saved_state.len(),
                            max_normal_repl_state_bytes
                        ),
                    );
                    String::new()
                }
            }
            Err(e) => {
                ctx.log(
                    "warn",
                    &format!("monty_repl: end-of-batch repl save failed: {e}"),
                );
                repl_file_id.to_string()
            }
        };

        // ProgressMade: a tool batch completed — this is real forward progress,
        // so reset the Executing state_timeout (not just ping liveness).
        session::send_progress(&ctx, &temper_api_url, tenant);

        // Check if agent signaled completion via temper.done(result)
        let done_result = dispatch::take_done_result();

        // Persist results (without repl_state in entity params).
        // Graceful: fall back to inline JSON if file persistence fails.
        let mut params = match session::persist_results(
            &ctx,
            &temper_api_url,
            tenant,
            &fields,
            &tool_results,
            &repl_file_id,
        ) {
            Ok(p) => p,
            Err(e) => {
                ctx.log(
                    "warn",
                    &format!("monty_repl: persist_results failed, falling back to inline: {e}"),
                );
                let results_json = serde_json::to_string(&tool_results).unwrap_or_default();
                json!({"pending_tool_calls": results_json, "repl_file_id": repl_file_id})
            }
        };

        // If a sandbox was lazily provisioned during this invocation,
        // include it in the callback params so it persists to entity state (ADR-0022).
        if let Some((url, id, provider)) = dispatch::take_lazy_sandbox() {
            params["sandbox_url"] = json!(url);
            params["sandbox_id"] = json!(id);
            params["sandbox_provider"] = json!(provider);
        }
        if persist_tool_spans_file(&ctx) && !tool_span_events.is_empty() {
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
        } else if !tool_span_events.is_empty() {
            ctx.log(
                "debug",
                &format!(
                    "monty_repl: skipping tool_spans file persist events={}",
                    tool_span_events.len()
                ),
            );
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
            done_params["pending_tool_calls"] = json!("");
            done_params["pending_tool_context"] = json!("");
            done_params["pending_decision_id"] = json!("");
            dispatch_success("RecordResult", &done_params);
        } else {
            dispatch_success("HandleToolResults", &params);
        }
        Ok(())
    })();

    let dispatched = action_dispatched();
    match classify_run_outcome(&result, dispatched) {
        RunOutcome::Success => 0,
        RunOutcome::InvariantViolation => {
            // Closure returned Ok but never dispatched a Session action.
            // Under the current code this means a silent code-path slipped
            // through — fire on_failure so the Session transitions to
            // Failed rather than staying stuck in Executing.
            dispatch_error(INVARIANT_VIOLATION_MSG);
            1
        }
        RunOutcome::PropagateError => {
            let e = result.err().unwrap_or_default();
            dispatch_error(&e);
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_ok_with_dispatch_is_success() {
        assert_eq!(classify_run_outcome(&Ok(()), true), RunOutcome::Success);
    }

    #[test]
    fn classify_ok_without_dispatch_is_invariant_violation() {
        // This is the orphan-creating path: closure returned Ok but never
        // fired a Session action. run() must convert this to an error so
        // on_failure="Fail" transitions the Session out of Executing.
        assert_eq!(
            classify_run_outcome(&Ok(()), false),
            RunOutcome::InvariantViolation
        );
    }

    #[test]
    fn classify_err_always_propagates() {
        assert_eq!(
            classify_run_outcome(&Err("boom".to_string()), true),
            RunOutcome::PropagateError
        );
        assert_eq!(
            classify_run_outcome(&Err("boom".to_string()), false),
            RunOutcome::PropagateError
        );
    }

    #[test]
    fn invariant_message_mentions_adr() {
        // Belt-and-suspenders regression guard: the error message must stay
        // greppable for future debugging. If someone "helpfully" rewrites it
        // to something bland, this test fails.
        assert!(INVARIANT_VIOLATION_MSG.contains("monty_repl"));
        assert!(INVARIANT_VIOLATION_MSG.contains("Session action"));
        assert!(INVARIANT_VIOLATION_MSG.contains("ADR-0039"));
    }

    #[test]
    fn tool_progress_wrapper_emits_start_and_end_on_success() {
        let mut events = Vec::new();

        let result = run_with_tool_progress(|event| events.push(event), || Ok::<_, String>(42));

        assert_eq!(result, Ok(42));
        assert_eq!(
            events,
            vec![
                run_control::ToolProgressBoundary::Start,
                run_control::ToolProgressBoundary::End
            ]
        );
    }

    #[test]
    fn tool_progress_wrapper_emits_end_on_error() {
        let mut events = Vec::new();

        let result = run_with_tool_progress(
            |event| events.push(event),
            || Err::<(), _>("tool failed".to_string()),
        );

        assert_eq!(result, Err("tool failed".to_string()));
        assert_eq!(
            events,
            vec![
                run_control::ToolProgressBoundary::Start,
                run_control::ToolProgressBoundary::End
            ]
        );
    }

    #[test]
    fn batchable_run_len_stops_before_non_batchable_snippet() {
        let tool_calls = vec![
            json!({"input": {"code": "temper.web_search('terminal ui inspiration')"}}),
            json!({"input": {"code": "temper.web_fetch('https://example.com/guide')"}}),
            json!({"input": {"code": "result = temper.web_search(query)"}}),
            json!({"input": {"code": "temper.specs()"}}),
        ];

        assert_eq!(batchable_run_len(&tool_calls, 0, tool_calls.len()), 2);
    }

    #[test]
    fn batchable_run_len_respects_checkpoint_window_limit() {
        let tool_calls = vec![
            json!({"input": {"code": "temper.web_search('one')"}}),
            json!({"input": {"code": "temper.web_fetch('https://example.com/two')"}}),
            json!({"input": {"code": "temper.specs()"}}),
        ];

        assert_eq!(batchable_run_len(&tool_calls, 0, 2), 2);
    }
}
