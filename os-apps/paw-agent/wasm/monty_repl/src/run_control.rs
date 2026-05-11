use temper_wasm_sdk::prelude::*;

// Thread-local flag set by `dispatch_success` / `dispatch_error` whenever
// this invocation has handed the Session a follow-up action to run. Read
// by `run()`'s outer match to enforce the "every WASM exit dispatches an
// action on the Session" invariant (openpaw ADR-0039 Sub-Decision 3a).
//
// Reset at the top of each `run()` call so re-used WASM instances don't
// carry state between invocations.
thread_local! {
    static ACTION_DISPATCHED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

pub(crate) const INVARIANT_VIOLATION_MSG: &str = "monty_repl exited without dispatching any Session action — invariant violation (ADR-0039 Sub-Decision 3a)";

pub(crate) fn reset_action_dispatched() {
    ACTION_DISPATCHED.with(|flag| flag.set(false));
}

pub(crate) fn action_dispatched() -> bool {
    ACTION_DISPATCHED.with(|flag| flag.get())
}

pub(crate) fn dispatch_success(action: &str, params: &Value) {
    ACTION_DISPATCHED.with(|flag| flag.set(true));
    set_success_result(action, params);
}

pub(crate) fn dispatch_error(error: &str) {
    ACTION_DISPATCHED.with(|flag| flag.set(true));
    set_error_result(error);
}

/// Classifies the outcome of `run()`'s body closure into a WASM exit code,
/// flagging the invariant-violation case ("closure returned Ok but no
/// Session action was dispatched") as an error so the integration's
/// `on_failure = "Fail"` hook fires instead of leaving the Session stuck
/// in its intermediate state.
///
/// Pure helper, unit-testable. The actual FFI `dispatch_error` call for
/// the violation path is wired in `run()` after consulting this function.
pub(crate) fn classify_run_outcome(
    closure_result: &Result<(), String>,
    action_dispatched: bool,
) -> RunOutcome {
    match (closure_result, action_dispatched) {
        (Ok(()), true) => RunOutcome::Success,
        (Ok(()), false) => RunOutcome::InvariantViolation,
        (Err(_), _) => RunOutcome::PropagateError,
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum RunOutcome {
    /// Closure returned Ok AND at least one action was dispatched.
    Success,
    /// Closure returned Ok with no action dispatched — structural
    /// invariant violation. Caller must dispatch a synthesized
    /// `dispatch_error` so `on_failure="Fail"` fires.
    InvariantViolation,
    /// Closure returned Err; caller dispatches `dispatch_error(&msg)`.
    PropagateError,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ToolProgressBoundary {
    Start,
    End,
}

pub(crate) fn run_with_tool_progress<T>(
    mut emit_progress: impl FnMut(ToolProgressBoundary),
    run_tool: impl FnOnce() -> T,
) -> T {
    emit_progress(ToolProgressBoundary::Start);
    let result = run_tool();
    emit_progress(ToolProgressBoundary::End);
    result
}

pub(crate) fn batch_window_len(
    start_index: usize,
    total_calls: usize,
    checkpoint_every_n: usize,
) -> usize {
    let remaining = total_calls.saturating_sub(start_index);
    let next_boundary = ((start_index / checkpoint_every_n) + 1) * checkpoint_every_n;
    let until_boundary = if next_boundary < total_calls {
        next_boundary.saturating_sub(start_index)
    } else {
        remaining
    };
    remaining.min(until_boundary.max(1))
}
