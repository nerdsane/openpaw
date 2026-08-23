//! computer_exec — WASM module executing one shell command on a Computer's sandbox.
//!
//! Runs on the Exec entity's Run action. Resolves the target Computer row via
//! the Temper API loopback, builds a SandboxHandle from its recorded fields
//! (provider, sandbox_url, machine_id), and executes the command through the
//! shared sandbox provider abstraction in wasm-helpers. Output is truncated to
//! a bounded tail before it is written back to the entity, so the audit row
//! stays small regardless of what the command prints.
//!
//! Reports back to the state machine: RunSucceeded (exit_code, stdout_tail,
//! stderr_tail) or RunFailed (error, via on_failure).
//!
//! Build: `cargo build --target wasm32-unknown-unknown --release`

use temper_wasm_sdk::prelude::*;
use wasm_helpers::sandbox::{self, ExecResult, SandboxHandle, normalize_sandbox_provider};
use wasm_helpers::{bounded_reads, entity_field_str, odata_headers, resolve_temper_api_url};

/// Keep at most this many bytes of stdout/stderr on the Exec row.
const OUTPUT_TAIL_BYTES: usize = 8192;

#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));

        let computer_id = param_or_field(&ctx, &fields, "computer_id")
            .ok_or("computer_exec: missing computer_id")?;
        let command =
            param_or_field(&ctx, &fields, "command").ok_or("computer_exec: missing command")?;

        ctx.log(
            "info",
            &format!(
                "computer_exec: exec {} on computer {computer_id} ({} byte command)",
                ctx.entity_id,
                command.len()
            ),
        );

        let temper_api_url = resolve_temper_api_url(&ctx, &fields);
        let computer = fetch_computer(&ctx, &temper_api_url, &fields, &computer_id)?;
        let handle = sandbox_handle_from_computer(&computer)
            .map_err(|e| format!("computer_exec: computer {computer_id}: {e}"))?;

        let result = sandbox::sandbox_exec(&ctx, &handle, &command, "/")?;
        ctx.log(
            "info",
            &format!(
                "computer_exec: command exited {} (stdout {} bytes, stderr {} bytes)",
                result.exit_code,
                result.stdout.len(),
                result.stderr.len()
            ),
        );

        set_success_result("RunSucceeded", &success_params(&result, OUTPUT_TAIL_BYTES));
        Ok(())
    })();

    if let Err(e) = result {
        set_error_result(&e);
    }
    0
}

/// Read a value from the trigger params, falling back to entity state.
fn param_or_field(ctx: &Context, fields: &Value, key: &str) -> Option<String> {
    ctx.trigger_params
        .get(key)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .or_else(|| {
            entity_field_str(fields, &[key])
                .filter(|s| !s.is_empty())
                .map(str::to_string)
        })
}

/// Fetch the Computer row by id via the Temper API loopback.
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
    bounded_reads::get_json(ctx, temper_api_url, &path, &headers, "computer_exec")
}

/// Build a SandboxHandle from a Computer row's recorded fields.
///
/// The Computer must be Ready with a sandbox_url — refusing to exec against
/// an unprovisioned or sleeping computer fails fast with a clear message
/// instead of timing out inside the provider call.
fn sandbox_handle_from_computer(computer: &Value) -> Result<SandboxHandle, String> {
    let status = entity_field_str(computer, &["Status", "status"]).unwrap_or("");
    if !status.is_empty() && status != "Ready" {
        return Err(format!("computer is {status}, not Ready"));
    }

    let sandbox_url = entity_field_str(computer, &["SandboxUrl", "sandbox_url"])
        .map(str::trim)
        .unwrap_or("");
    if sandbox_url.is_empty() {
        return Err("no sandbox_url recorded — provision the computer first".to_string());
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

/// Build the RunSucceeded callback params, truncating output to a bounded tail.
fn success_params(result: &ExecResult, tail_bytes: usize) -> Value {
    json!({
        "exit_code": result.exit_code.to_string(),
        "stdout_tail": output_tail(&result.stdout, tail_bytes),
        "stderr_tail": output_tail(&result.stderr, tail_bytes),
    })
}

/// Keep the last `max_bytes` bytes of `text`, aligned to a char boundary,
/// with a marker noting how much was dropped.
fn output_tail(text: &str, max_bytes: usize) -> String {
    if text.len() <= max_bytes {
        return text.to_string();
    }
    let mut start = text.len() - max_bytes;
    while !text.is_char_boundary(start) {
        start += 1;
    }
    format!(
        "[... {} bytes truncated ...]\n{}",
        start,
        &text[start..]
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ready_computer() -> Value {
        json!({
            "Id": "computer-1",
            "Status": "Ready",
            "fields": {
                "name": "dd-computer",
                "provider": "tensorlake",
                "machine_id": "sbx-abc123",
                "sandbox_url": "https://sbx-abc123.sandbox.tensorlake.ai",
            }
        })
    }

    #[test]
    fn handle_from_ready_computer() {
        let handle = sandbox_handle_from_computer(&ready_computer()).unwrap();
        assert_eq!(
            handle.sandbox_url,
            "https://sbx-abc123.sandbox.tensorlake.ai"
        );
        assert_eq!(handle.sandbox_id, "sbx-abc123");
        assert_eq!(handle.provider, "tensorlake");
    }

    #[test]
    fn handle_normalizes_provider_alias() {
        let mut computer = ready_computer();
        computer["fields"]["provider"] = json!("tl");
        let handle = sandbox_handle_from_computer(&computer).unwrap();
        assert_eq!(handle.provider, "tensorlake");
    }

    #[test]
    fn handle_defaults_provider_to_tensorlake() {
        let mut computer = ready_computer();
        computer["fields"]["provider"] = json!("");
        let handle = sandbox_handle_from_computer(&computer).unwrap();
        assert_eq!(handle.provider, "tensorlake");
    }

    #[test]
    fn handle_falls_back_to_name_when_machine_id_missing() {
        let mut computer = ready_computer();
        computer["fields"]["machine_id"] = json!("");
        let handle = sandbox_handle_from_computer(&computer).unwrap();
        assert_eq!(handle.sandbox_id, "dd-computer");
    }

    #[test]
    fn handle_rejects_missing_sandbox_url() {
        let mut computer = ready_computer();
        computer["fields"]["sandbox_url"] = json!("");
        let err = sandbox_handle_from_computer(&computer).err().unwrap();
        assert!(err.contains("no sandbox_url"), "unexpected error: {err}");
    }

    #[test]
    fn handle_rejects_non_ready_computer() {
        let mut computer = ready_computer();
        computer["Status"] = json!("Sleeping");
        let err = sandbox_handle_from_computer(&computer).err().unwrap();
        assert!(err.contains("Sleeping"), "unexpected error: {err}");
    }

    #[test]
    fn tail_keeps_short_output_intact() {
        assert_eq!(output_tail("hello", 8192), "hello");
    }

    #[test]
    fn tail_truncates_long_output_with_marker() {
        let long = "x".repeat(10_000);
        let tail = output_tail(&long, 8192);
        assert!(tail.starts_with("[... 1808 bytes truncated ...]\n"));
        assert!(tail.ends_with('x'));
        assert_eq!(tail.len(), "[... 1808 bytes truncated ...]\n".len() + 8192);
    }

    #[test]
    fn tail_respects_char_boundaries() {
        // 3-byte chars; a byte cut would land mid-char without alignment.
        let long = "€".repeat(4000);
        let tail = output_tail(&long, 8192);
        assert!(tail.contains("bytes truncated"));
        assert!(tail.ends_with('€'));
    }

    #[test]
    fn success_params_carry_exit_code_and_tails() {
        let result = ExecResult {
            stdout: "ok\n".to_string(),
            stderr: String::new(),
            exit_code: 0,
        };
        let params = success_params(&result, 8192);
        assert_eq!(params["exit_code"], "0");
        assert_eq!(params["stdout_tail"], "ok\n");
        assert_eq!(params["stderr_tail"], "");
    }

    #[test]
    fn success_params_truncate_output() {
        let result = ExecResult {
            stdout: "y".repeat(20_000),
            stderr: String::new(),
            exit_code: 1,
        };
        let params = success_params(&result, 8192);
        assert_eq!(params["exit_code"], "1");
        let stdout_tail = params["stdout_tail"].as_str().unwrap();
        assert!(stdout_tail.contains("bytes truncated"));
        assert!(stdout_tail.len() < 20_000);
    }
}
