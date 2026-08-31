//! computer_exec_start — launch a command on a Computer's sandbox and return
//! immediately (ARN-443 D, async exec).
//!
//! Fires on Exec.Run. Resolves the target Computer (Ready-gated), wraps the
//! command in a long sandbox-side `timeout` (the async path has no 120s cap — the
//! Poll loop spans invocations), starts it via `sandbox_exec_start`, and reports
//! ExecStarted(run_id, started_at_ms). The Poll loop (computer_exec_poll) then
//! drives it to completion. On failure the trigger's on_failure routes to
//! RunFailed.
//!
//! (NOTE: the small computer-resolution helpers below are duplicated from
//! computer_exec / computer_exec_poll; DRY-ing them into a shared wasm-helpers
//! module is a tracked follow-up, kept out of this first async pass.)
//!
//! Build: `cargo build --target wasm32-wasip1 --release`.

use temper_wasm_sdk::prelude::*;
use wasm_helpers::sandbox::{self, SandboxHandle, normalize_sandbox_provider};
use wasm_helpers::{bounded_reads, entity_field_str, odata_headers, resolve_temper_api_url};

/// Hard wall-clock limit for an async command, enforced sandbox-side by `timeout`
/// so a runaway is killed even though the WASM never blocks on it. The poll loop
/// waits (across invocations) up to this long plus a margin.
const MAX_RUN_SECS: u64 = 1800;

#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));

        let computer_id = field(&fields, "computer_id")
            .ok_or("computer_exec_start: missing computer_id")?;
        let command = field(&fields, "command").ok_or("computer_exec_start: missing command")?;

        let temper_api_url = resolve_temper_api_url(&ctx, &fields);
        let computer = fetch_computer(&ctx, &temper_api_url, &fields, &computer_id)?;
        let handle = handle_from_computer(&computer)
            .map_err(|e| format!("computer_exec_start: computer {computer_id}: {e}"))?;

        let wrapped = format!(
            "timeout -k 5s {MAX_RUN_SECS}s bash -c {}",
            shell_single_quote(&command)
        );
        let run_id = sandbox::sandbox_exec_start(&ctx, &handle, &wrapped)?;
        let started_at_ms = Context::get_time_millis().to_string();

        ctx.log(
            "info",
            &format!(
                "computer_exec_start: started {} on {computer_id} (run_id {run_id})",
                ctx.entity_id
            ),
        );

        set_success_result(
            "ExecStarted",
            &json!({ "run_id": run_id, "started_at_ms": started_at_ms }),
        );
        Ok(())
    })();

    if let Err(e) = result {
        set_error_result(&e);
    }
    0
}

fn field(fields: &Value, key: &str) -> Option<String> {
    entity_field_str(fields, &[key])
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
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
    bounded_reads::get_json(ctx, temper_api_url, &path, &headers, "computer_exec_start")
}

/// Build a SandboxHandle from a Computer row. Fails CLOSED: only a Ready computer
/// with a sandbox_url is exec-able (a Leased copy is not Ready — the D-time gate
/// widening to accept Leased is a forward pointer noted in computer.ioa.toml).
fn handle_from_computer(computer: &Value) -> Result<SandboxHandle, String> {
    let status = entity_field_str(computer, &["Status", "status"]).unwrap_or("");
    if status != "Ready" {
        let shown = if status.is_empty() { "(no status)" } else { status };
        return Err(format!("computer is {shown}, not Ready"));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handle_requires_ready() {
        let c = json!({"Status": "Leased", "fields": {"sandbox_url": "https://x.sandbox.tensorlake.ai"}});
        assert!(handle_from_computer(&c).is_err());
        let c = json!({"Status": "Ready", "fields": {"sandbox_url": "https://x.sandbox.tensorlake.ai", "machine_id": "x"}});
        assert!(handle_from_computer(&c).is_ok());
    }

    #[test]
    fn quotes_command_for_bash() {
        assert_eq!(shell_single_quote("echo 'hi'"), r"'echo '\''hi'\'''");
    }
}
