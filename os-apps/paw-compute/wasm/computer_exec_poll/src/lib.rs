//! computer_exec_poll — poll a started async exec once (ARN-443 D).
//!
//! Fires on Exec.Poll (driven by the state_timeout on Running). Re-resolves the
//! Computer, polls the started process via `sandbox_exec_poll(run_id)`:
//! - finished  → RunSucceeded(exit_code, tails)  (exit 124 from the sandbox
//!               timeout → RunFailed "exceeded the run limit");
//! - running   → KeepRunning if before the safety deadline, else RunFailed;
//! The Poll transition itself re-arms the Running lease, so KeepRunning simply
//! lets the loop continue.
//!
//! (Resolution helpers duplicated from computer_exec_start — DRY follow-up.)
//!
//! Build: `cargo build --target wasm32-wasip1 --release`.

use temper_wasm_sdk::prelude::*;
use wasm_helpers::sandbox::{self, ExecResult, SandboxHandle, normalize_sandbox_provider};
use wasm_helpers::{bounded_reads, entity_field_str, odata_headers, resolve_temper_api_url};

const OUTPUT_TAIL_BYTES: usize = 262_144;
/// Safety deadline for the poll (the sandbox-side `timeout` in computer_exec_start
/// is the real command bound; this only guards a poll that never sees an rc file).
const MAX_RUN_MS: i64 = 1_800_000 + 60_000;
/// `timeout`'s exit status when it kills the command.
const TIMEOUT_EXIT_CODE: i64 = 124;

#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));

        let computer_id =
            field(&fields, "computer_id").ok_or("computer_exec_poll: missing computer_id")?;
        let run_id = field(&fields, "run_id").ok_or("computer_exec_poll: missing run_id")?;

        let temper_api_url = resolve_temper_api_url(&ctx, &fields);
        let computer = fetch_computer(&ctx, &temper_api_url, &fields, &computer_id)?;
        let handle = handle_from_computer(&computer)
            .map_err(|e| format!("computer_exec_poll: computer {computer_id}: {e}"))?;

        match sandbox::sandbox_exec_poll(&ctx, &handle, &run_id)? {
            Some(result) => {
                if result.exit_code == TIMEOUT_EXIT_CODE {
                    set_failure_result("command exceeded the run limit and was terminated");
                } else {
                    set_success_result("RunSucceeded", &success_params(&result));
                }
            }
            None => {
                let started = field(&fields, "started_at_ms")
                    .and_then(|s| s.parse::<i64>().ok())
                    .unwrap_or(0);
                if started != 0 && Context::get_time_millis() - started > MAX_RUN_MS {
                    set_failure_result("exec exceeded its deadline without completing");
                } else {
                    set_success_result("KeepRunning", &json!({}));
                }
            }
        }
        Ok(())
    })();

    if let Err(e) = result {
        set_error_result(&e);
    }
    0
}

fn success_params(result: &ExecResult) -> Value {
    json!({
        "exit_code": result.exit_code.to_string(),
        "stdout_tail": output_tail(&result.stdout, OUTPUT_TAIL_BYTES),
        "stderr_tail": output_tail(&result.stderr, OUTPUT_TAIL_BYTES),
        "stdout_path": "",
        "stdout_bytes": "",
    })
}

/// Emit RunFailed with the error and cleared result fields.
fn set_failure_result(error: &str) {
    let result = json!({
        "action": "callback",
        "params": { "error": error, "exit_code": "", "stdout_tail": "", "stderr_tail": "" },
        "success": false,
        "error": error,
    });
    let json = result.to_string();
    unsafe {
        temper_wasm_sdk::host::host_set_result(json.as_ptr() as i32, json.len() as i32);
    }
}

fn output_tail(text: &str, max_bytes: usize) -> String {
    let text = text.strip_prefix('\u{FFFD}').unwrap_or(text);
    if text.len() <= max_bytes {
        return text.to_string();
    }
    let mut start = text.len() - max_bytes;
    while !text.is_char_boundary(start) {
        start += 1;
    }
    format!("[... {} bytes truncated ...]\n{}", start, &text[start..])
}

fn field(fields: &Value, key: &str) -> Option<String> {
    entity_field_str(fields, &[key])
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn fetch_computer(
    ctx: &Context,
    temper_api_url: &str,
    fields: &Value,
    computer_id: &str,
) -> Result<Value, String> {
    let headers = odata_headers(ctx, &ctx.tenant, fields);
    let path = format!(
        "/tdata/Computers('{}')",
        bounded_reads::odata_escape(computer_id)
    );
    bounded_reads::get_json(ctx, temper_api_url, &path, &headers, "computer_exec_poll")
}

fn handle_from_computer(computer: &Value) -> Result<SandboxHandle, String> {
    let sandbox_url = entity_field_str(computer, &["SandboxUrl", "sandbox_url"])
        .map(str::trim)
        .unwrap_or("");
    if sandbox_url.is_empty() {
        return Err("no sandbox_url recorded".to_string());
    }
    let sandbox_id = entity_field_str(computer, &["MachineId", "machine_id"])
        .filter(|s| !s.is_empty())
        .or_else(|| entity_field_str(computer, &["Name", "name"]).filter(|s| !s.is_empty()))
        .unwrap_or("computer-sandbox");
    let provider = entity_field_str(computer, &["Provider", "provider"])
        .filter(|s| !s.is_empty())
        .map(normalize_sandbox_provider)
        .unwrap_or_else(|| "tensorlake".to_string());
    Ok(SandboxHandle {
        sandbox_url: sandbox_url.to_string(),
        sandbox_id: sandbox_id.to_string(),
        provider,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn success_params_truncates_and_carries_exit() {
        let r = ExecResult { stdout: "ok".into(), stderr: "warn".into(), exit_code: 0 };
        let p = success_params(&r);
        assert_eq!(p["exit_code"], "0");
        assert_eq!(p["stdout_tail"], "ok");
        assert_eq!(p["stderr_tail"], "warn");
    }

    #[test]
    fn tail_bounds_output() {
        let long = "x".repeat(300_000);
        let t = output_tail(&long, OUTPUT_TAIL_BYTES);
        assert!(t.contains("bytes truncated"));
        assert!(t.len() < 300_000);
    }
}
