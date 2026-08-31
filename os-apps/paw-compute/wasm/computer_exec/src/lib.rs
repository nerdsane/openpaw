//! computer_exec — WASM module executing one shell command on a Computer's sandbox.
//!
//! Runs on the Exec entity's Run action (and LatencyDiag's RunScan). Resolves the
//! target Computer row via the Temper API loopback, builds a SandboxHandle from
//! its recorded fields (provider, sandbox_url, machine_id), and executes the
//! command through the shared sandbox provider abstraction in wasm-helpers.
//! stdout and stderr are captured to separate per-exec log files on the sandbox;
//! a bounded tail of each is written back to the entity so the audit row stays
//! small regardless of what the command prints.
//!
//! Reports back to the state machine: RunSucceeded (exit_code, stdout_tail,
//! stderr_tail, stdout_path, stdout_bytes) or RunFailed (error + cleared result
//! fields, via `set_failure_result`).
//!
//! Build: `cargo build --target wasm32-wasip1 --release` (WASI target — the host
//! wires wasi_snapshot_preview1; wasm32-unknown-unknown is forbidden, it links
//! wasm-bindgen via chrono and fails host instantiation).

use temper_wasm_sdk::prelude::*;
use wasm_helpers::sandbox::{self, ExecResult, SandboxHandle, normalize_sandbox_provider};
use wasm_helpers::{bounded_reads, entity_field_str, odata_headers, resolve_temper_api_url};

/// Keep at most this many bytes of stdout/stderr on the row. The full output is
/// persisted to per-exec log files on the sandbox (see [`wrap_command`]); this
/// bounds only the tails carried back on the row.
const OUTPUT_TAIL_BYTES: usize = 262_144;

/// Hard wall-clock limit for the command on the sandbox, enforced by `timeout`
/// so a runaway command is killed and cannot outlive the exec (no orphans). Kept
/// just under the 120s WASM invocation cap (temper-wasm `WasmResourceLimits`), so
/// the timeout fires and its result is read within one synchronous invocation.
/// Longer runs need the async exec path (ARN-443 D), not a larger value here.
const EXEC_TIMEOUT_SECS: u64 = 110;

/// `timeout` exit code when the wall-clock limit is reached (coreutils).
const TIMEOUT_EXIT_CODE: i64 = 124;

/// Markers separating the wrapper's metadata / stdout tail / stderr tail.
const EXEC_LOG_MARKER: &str = "__EXEC_LOG_PATH";
const EXEC_ERR_MARKER: &str = "__EXEC_ERR_TAIL";

#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));

        let computer_id = field_or_param(&ctx, &fields, "computer_id")
            .ok_or("computer_exec: missing computer_id")?;
        let command =
            field_or_param(&ctx, &fields, "command").ok_or("computer_exec: missing command")?;

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

        let wrapped = wrap_command(&command, &ctx.entity_id);
        let result = sandbox::sandbox_exec(&ctx, &handle, &wrapped, "/")?;
        ctx.log(
            "info",
            &format!(
                "computer_exec: command exited {} (stdout {} bytes, stderr {} bytes)",
                result.exit_code,
                result.stdout.len(),
                result.stderr.len()
            ),
        );

        // A `timeout`-killed command is a FAILURE, not a result: mark Failed so
        // the row does not report a bogus exit code, and the process is already
        // gone (timeout killed its group on the sandbox — no orphan).
        if result.exit_code == TIMEOUT_EXIT_CODE {
            set_failure_result(&format!(
                "command exceeded the {EXEC_TIMEOUT_SECS}s limit and was terminated on the sandbox"
            ));
            return Ok(());
        }

        set_success_result("RunSucceeded", &success_params(&result, OUTPUT_TAIL_BYTES));
        Ok(())
    })();

    if let Err(e) = result {
        set_failure_result(&e);
    }
    0
}

/// Read a value from entity state, falling back to trigger params.
///
/// Entity fields WIN: a spec-pinned field (e.g. LatencyDiag's canned command and
/// pinned computer_id) can never be overridden by caller-supplied trigger params.
/// For Exec, the Run action writes its `command`/`computer_id` params onto the
/// entity fields before this trigger fires, so reading the field first still
/// yields the caller's command. Trigger params remain a fallback for any field
/// the state has left empty.
fn field_or_param(ctx: &Context, fields: &Value, key: &str) -> Option<String> {
    entity_field_str(fields, &[key])
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .or_else(|| {
            ctx.trigger_params
                .get(key)
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(str::to_string)
        })
}

/// Emit the on-failure callback with the error AND cleared result fields, so a
/// re-run that fails (e.g. LatencyDiag rescanning) never leaves a previous run's
/// exit code / output on the row. `success: false` routes it to the spec's
/// `on_failure` action (RunFailed); the cleared params overwrite stale values.
fn set_failure_result(error: &str) {
    let result = json!({
        "action": "callback",
        "params": {
            "error": error,
            "exit_code": "",
            "stdout_tail": "",
            "stderr_tail": "",
        },
        "success": false,
        "error": error,
    });
    let json = result.to_string();
    unsafe {
        temper_wasm_sdk::host::host_set_result(json.as_ptr() as i32, json.len() as i32);
    }
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
/// The Computer must be explicitly Ready with a sandbox_url. Fail CLOSED: any
/// status other than exactly "Ready" — including missing/empty — is refused, so a
/// half-provisioned or unknown-state computer never gets a command dispatched to
/// it.
fn sandbox_handle_from_computer(computer: &Value) -> Result<SandboxHandle, String> {
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

/// Wrap the user command so its full stdout and stderr are persisted to separate
/// per-exec log files on the sandbox while the row carries a bounded tail of each.
///
/// - The command runs under `timeout` in a CHILD `bash -c` (a subshell/child
///   process): the command's own `exit`/`exec` control flow cannot skip the
///   wrapper's epilogue, and `timeout -k` kills the whole child group if it runs
///   past `EXEC_TIMEOUT_SECS` (no orphaned process).
/// - stdout → `<id>.log`, stderr → `<id>.err` (separate streams, not conflated).
/// - `__rc=$?` preserves the child's exit code (124 = timed out).
/// - The wrapper prints, to its own stdout: the stdout byte count, a
///   `__EXEC_LOG_PATH` marker, the stdout tail, an `__EXEC_ERR_TAIL` marker, and
///   the stderr tail. `exit $__rc` surfaces the true status. The full logs stay
///   on the computer for follow-up Execs to grep/sed/page.
fn wrap_command(command: &str, exec_id: &str) -> String {
    let id = sanitize_exec_id(exec_id);
    let log = format!("~/.exec-out/{id}.log");
    let err = format!("~/.exec-out/{id}.err");
    let q = shell_single_quote(command);
    format!(
        "mkdir -p ~/.exec-out && \
         timeout -k 5s {EXEC_TIMEOUT_SECS}s bash -c {q} > {log} 2> {err} ; __rc=$? ; \
         wc -c < {log} ; echo \"{EXEC_LOG_MARKER} {log}\" ; tail -c {OUTPUT_TAIL_BYTES} {log} ; \
         echo ; echo \"{EXEC_ERR_MARKER}\" ; tail -c {OUTPUT_TAIL_BYTES} {err} ; exit $__rc"
    )
}

/// POSIX single-quote a string so it survives as one `bash -c` argument.
fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

/// Reduce an exec id to a filename-safe token so it cannot escape `~/.exec-out`.
/// Anything outside `[A-Za-z0-9._-]` becomes `_`.
fn sanitize_exec_id(exec_id: &str) -> String {
    let cleaned: String = exec_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '_'
            }
        })
        .collect();
    if cleaned.is_empty() {
        "exec".to_string()
    } else {
        cleaned
    }
}

/// The stdout byte count, stdout log path, stdout tail, and stderr tail parsed
/// out of a wrapped command's stdout.
struct CapturedOutput {
    path: Option<String>,
    bytes: Option<u64>,
    stdout_tail: String,
    stderr_tail: String,
}

/// Parse the wrapped-command stdout:
/// `<bytes>\n__EXEC_LOG_PATH <path>\n<stdout tail>\n__EXEC_ERR_TAIL\n<stderr tail>`.
/// If the markers are absent — e.g. the wrapper never ran (provider error text) —
/// the raw text is returned as the stdout tail so nothing is silently dropped.
fn parse_captured_output(stdout: &str) -> CapturedOutput {
    let mut parts = stdout.splitn(3, '\n');
    let first = parts.next().unwrap_or("");
    let second = parts.next().unwrap_or("");
    let rest = parts.next().unwrap_or("");

    let bytes = first.trim().parse::<u64>().ok();
    let path = second
        .strip_prefix(&format!("{EXEC_LOG_MARKER} "))
        .map(|s| s.trim().to_string());

    if bytes.is_none() && path.is_none() {
        return CapturedOutput {
            path: None,
            bytes: None,
            stdout_tail: stdout.to_string(),
            stderr_tail: String::new(),
        };
    }

    // Split the remainder into the stdout tail and the stderr tail on the err
    // marker's own line. If it is absent, everything is the stdout tail.
    let sep = format!("\n{EXEC_ERR_MARKER}\n");
    let (stdout_tail, stderr_tail) = match rest.split_once(&sep) {
        Some((out, err)) => (out.to_string(), err.to_string()),
        None => (rest.to_string(), String::new()),
    };

    CapturedOutput {
        path,
        bytes,
        stdout_tail,
        stderr_tail,
    }
}

/// Build the RunSucceeded callback params, truncating each stream to a bounded
/// tail and surfacing the full-stdout log path/size captured by [`wrap_command`].
fn success_params(result: &ExecResult, tail_bytes: usize) -> Value {
    let captured = parse_captured_output(&result.stdout);
    // Prefer the separated stderr from the wrapper; fall back to any stderr the
    // provider surfaced directly (e.g. when the wrapper never ran).
    let stderr = if captured.stderr_tail.is_empty() && !result.stderr.is_empty() {
        result.stderr.as_str()
    } else {
        captured.stderr_tail.as_str()
    };
    json!({
        "exit_code": result.exit_code.to_string(),
        "stdout_tail": output_tail(&captured.stdout_tail, tail_bytes),
        "stderr_tail": output_tail(stderr, tail_bytes),
        "stdout_path": captured.path.unwrap_or_default(),
        "stdout_bytes": captured.bytes.map(|b| b.to_string()).unwrap_or_default(),
    })
}

/// Keep the last `max_bytes` bytes of `text`, aligned to a char boundary, with a
/// marker noting how much was dropped. A leading replacement char left by a
/// byte-level `tail -c` on the sandbox is trimmed so the tail starts clean.
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
        assert_eq!(handle.sandbox_url, "https://sbx-abc123.sandbox.tensorlake.ai");
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
    fn handle_rejects_missing_status_fail_closed() {
        // Fail CLOSED: a computer with no Status must be refused, not treated
        // as Ready.
        let computer = json!({
            "Id": "c",
            "fields": { "sandbox_url": "https://x.sandbox.tensorlake.ai", "machine_id": "x" }
        });
        let err = sandbox_handle_from_computer(&computer).err().unwrap();
        assert!(err.contains("no status"), "unexpected error: {err}");
    }

    #[test]
    fn field_wins_over_trigger_param() {
        // Entity fields win: a spec-pinned command cannot be overridden by a
        // caller-supplied trigger param.
        let ctx_params = json!({ "command": "rm -rf /", "computer_id": "attacker" });
        let fields = json!({ "command": "echo pinned", "computer_id": "dsf" });
        // Emulate field_or_param's precedence directly against the two sources.
        let pick = |k: &str| {
            super::entity_field_str(&fields, &[k])
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .or_else(|| ctx_params.get(k).and_then(|v| v.as_str()).map(str::to_string))
        };
        assert_eq!(pick("command").as_deref(), Some("echo pinned"));
        assert_eq!(pick("computer_id").as_deref(), Some("dsf"));
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
        let long = "€".repeat(4000);
        let tail = output_tail(&long, 8192);
        assert!(tail.contains("bytes truncated"));
        assert!(tail.ends_with('€'));
    }

    #[test]
    fn tail_trims_leading_replacement_char() {
        // A byte-level `tail -c` on the sandbox can leave a leading partial char
        // (decoded to U+FFFD); it is trimmed so the tail starts clean.
        let text = format!("\u{FFFD}rest of the output");
        assert_eq!(output_tail(&text, 8192), "rest of the output");
    }

    #[test]
    fn default_tail_cap_is_256k() {
        assert_eq!(OUTPUT_TAIL_BYTES, 262_144);
    }

    // -- Command wrapping ----------------------------------------------------

    #[test]
    fn wrap_runs_command_in_a_timed_child_and_separates_streams() {
        let wrapped = wrap_command("echo hi", "exec-1");
        assert!(wrapped.contains("mkdir -p ~/.exec-out"));
        // Child process (bash -c) under a hard timeout with a kill grace.
        assert!(wrapped.contains("timeout -k 5s 110s bash -c 'echo hi'"), "{wrapped}");
        // Separate stdout and stderr files (not conflated).
        assert!(wrapped.contains("> ~/.exec-out/exec-1.log 2> ~/.exec-out/exec-1.err"));
        // Byte count + markers + bounded tails.
        assert!(wrapped.contains("wc -c < ~/.exec-out/exec-1.log"));
        assert!(wrapped.contains("__EXEC_LOG_PATH ~/.exec-out/exec-1.log"));
        assert!(wrapped.contains("tail -c 262144 ~/.exec-out/exec-1.log"));
        assert!(wrapped.contains("__EXEC_ERR_TAIL"));
        assert!(wrapped.contains("tail -c 262144 ~/.exec-out/exec-1.err"));
        // Original exit code preserved through the wrapper's epilogue.
        assert!(wrapped.contains("__rc=$?"));
        assert!(wrapped.trim_end().ends_with("exit $__rc"));
    }

    #[test]
    fn wrap_single_quotes_the_command() {
        // A command with a single quote is safely quoted for bash -c.
        let wrapped = wrap_command("echo 'hi'", "e");
        assert!(wrapped.contains(r"bash -c 'echo '\''hi'\'''"), "{wrapped}");
    }

    #[test]
    fn wrap_sanitizes_exec_id_into_filename() {
        let wrapped = wrap_command("true", "abc/../../etc 9");
        assert!(wrapped.contains("~/.exec-out/abc_.._.._etc_9.log"));
        assert!(!wrapped.contains("../../etc"));
    }

    #[test]
    fn parse_extracts_bytes_path_stdout_and_stderr() {
        let stdout = "1234\n__EXEC_LOG_PATH ~/.exec-out/e.log\nhello\nworld\n\n__EXEC_ERR_TAIL\noops\n";
        let cap = parse_captured_output(stdout);
        assert_eq!(cap.bytes, Some(1234));
        assert_eq!(cap.path.as_deref(), Some("~/.exec-out/e.log"));
        assert_eq!(cap.stdout_tail, "hello\nworld\n");
        assert_eq!(cap.stderr_tail, "oops\n");
    }

    #[test]
    fn parse_handles_absent_stderr_marker() {
        let stdout = "3\n__EXEC_LOG_PATH ~/.exec-out/e.log\nonly stdout\n";
        let cap = parse_captured_output(stdout);
        assert_eq!(cap.stdout_tail, "only stdout\n");
        assert_eq!(cap.stderr_tail, "");
    }

    #[test]
    fn parse_falls_back_to_raw_when_markers_absent() {
        let stdout = "some unexpected provider output";
        let cap = parse_captured_output(stdout);
        assert_eq!(cap.bytes, None);
        assert_eq!(cap.path, None);
        assert_eq!(cap.stdout_tail, "some unexpected provider output");
        assert_eq!(cap.stderr_tail, "");
    }

    #[test]
    fn success_params_carry_exit_code_stdout_and_stderr() {
        let result = ExecResult {
            stdout: "3\n__EXEC_LOG_PATH ~/.exec-out/e.log\nok\n\n__EXEC_ERR_TAIL\nwarn\n".to_string(),
            stderr: String::new(),
            exit_code: 0,
        };
        let params = success_params(&result, 8192);
        assert_eq!(params["exit_code"], "0");
        assert_eq!(params["stdout_tail"], "ok\n");
        assert_eq!(params["stderr_tail"], "warn\n");
        assert_eq!(params["stdout_path"], "~/.exec-out/e.log");
        assert_eq!(params["stdout_bytes"], "3");
    }

    #[test]
    fn success_params_truncate_output() {
        let body = "y".repeat(20_000);
        let result = ExecResult {
            stdout: format!("20000\n__EXEC_LOG_PATH ~/.exec-out/e.log\n{body}"),
            stderr: String::new(),
            exit_code: 1,
        };
        let params = success_params(&result, 8192);
        assert_eq!(params["exit_code"], "1");
        let stdout_tail = params["stdout_tail"].as_str().unwrap();
        assert!(stdout_tail.contains("bytes truncated"));
        assert!(stdout_tail.len() < 20_000);
        assert_eq!(params["stdout_bytes"], "20000");
    }
}
