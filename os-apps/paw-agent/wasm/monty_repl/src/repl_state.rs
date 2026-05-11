use std::time::Duration;

use monty::{
    DictPairs, ExcType, ExtFunctionResult, LimitedTracker, MontyException, MontyObject, MontyRepl,
    PrintWriter, ReplProgress, ResourceLimits,
};
use temper_wasm_sdk::prelude::*;

use crate::session;
use crate::tool_results::{base64_decode, base64_encode, format_monty_exception};

// 512 KB default. Empirically the Monty dump after a typical katagami-style
// turn (specs lookup + a handful of entity reads) is ~180 KB, so 512 KB gives
// comfortable headroom. Persistence is not optional — without it, every
// `temper.execute` invocation re-creates the REPL from scratch and the agent
// loses every variable, import, and intermediate result, which turns a 5-turn
// task into a 50+ turn rediscovery loop.
const DEFAULT_NORMAL_REPL_STATE_MAX_BYTES: usize = 512 * 1024;

/// Load a persistent REPL from serialized state, or create a fresh one.
///
/// At c9802b5 (v0.0.9), MontyRepl::new() creates a bare REPL; globals are
/// injected via feed_start() which we drive to completion.
pub(crate) fn load_or_create_repl(
    repl_state_b64: &str,
    ctx: &Context,
) -> Result<MontyRepl<LimitedTracker>, String> {
    let limits = ResourceLimits::new()
        .max_duration(Duration::from_secs(300))
        .max_memory(64 * 1024 * 1024)
        .max_allocations(250_000);
    let tracker = LimitedTracker::new(limits);

    if repl_state_b64.is_empty() {
        // Create bare REPL, then inject temper + sandbox globals via feed_start.
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
            (
                "json".to_string(),
                MontyObject::Dataclass {
                    name: "Json".to_string(),
                    type_id: 3,
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
            "monty_repl: created fresh REPL with temper + sandbox + json objects",
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
pub(crate) fn save_repl_state(repl: &MontyRepl<LimitedTracker>) -> Result<String, String> {
    let bytes = repl
        .dump()
        .map_err(|e| format!("failed to serialize REPL state: {e}"))?;
    Ok(base64_encode(&bytes))
}

pub(crate) fn normal_repl_state_max_bytes(ctx: &Context) -> usize {
    // Treat an explicitly-zero config the same as missing — fall back to the
    // default. PR #162 shipped `"0"` as a temporary kill-switch and that
    // value is currently baked into the deployed spec; we don't want a stale
    // override to silently disable persistence (and the resulting agent
    // re-discovery loop) until the next Docker bake.
    let configured = ctx
        .config
        .get("normal_repl_state_max_bytes")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0);
    if configured > 0 {
        configured
    } else {
        DEFAULT_NORMAL_REPL_STATE_MAX_BYTES
    }
}

pub(crate) fn persist_tool_spans_file(ctx: &Context) -> bool {
    ctx.config
        .get("persist_tool_spans_file")
        .map(|value| matches!(value.as_str(), "1" | "true" | "yes" | "on"))
        .unwrap_or(false)
}

pub(crate) fn read_repl_state_b64(
    ctx: &Context,
    fields: &Value,
    temper_api_url: &str,
    tenant: &str,
) -> String {
    let repl_file_id = fields
        .get("repl_file_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if !repl_file_id.is_empty() {
        session::read_temperfs_file_safe(ctx, temper_api_url, tenant, repl_file_id)
            .unwrap_or_default()
    } else {
        fields
            .get("repl_state")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    }
}
