//! Gate Verifier — WASM module for running harness checks in a sandbox.
//!
//! Reads gate_level from integration config, maps it to a set of verification
//! commands, executes each in the SWE's sandbox via the Tensorlake process API,
//! and dispatches Report* actions on the WorkCycle entity for each passing check.
//!
//! Build: `cargo build --target wasm32-unknown-unknown --release`

use temper_wasm_sdk::prelude::*;

/// Level-1 gate checks: migrations, typecheck, unit tests, response models, API tests, frontend e2e.
const LEVEL1_CHECKS: &[(&str, &str, &str)] = &[
    (
        "migrations",
        "ReportMigrations",
        "./scripts/check-migrations.sh",
    ),
    (
        "typecheck",
        "ReportTypecheck",
        "cd platform && bun run typecheck",
    ),
    (
        "unit_tests",
        "ReportUnitTests",
        "cd platform/backend && pytest tests/ -x -q --ignore=tests/simulation",
    ),
    (
        "response_models",
        "ReportResponseModels",
        "python scripts/check_response_model_coverage.py --check",
    ),
    (
        "api_tests",
        "ReportApiTests",
        "python scripts/check_api_test_coverage.py --check",
    ),
    (
        "frontend_e2e",
        "ReportFrontendE2e",
        "python scripts/check_frontend_e2e_mapping.py --check",
    ),
];

/// Level-2 gate checks: DST simulation, DST coverage, frontend tests.
const LEVEL2_CHECKS: &[(&str, &str, &str)] = &[
    (
        "dst_simulation",
        "ReportDstSimulation",
        "cd platform/backend && pytest tests/simulation/ -x --hypothesis-seed=0",
    ),
    (
        "dst_coverage",
        "ReportDstCoverage",
        "python scripts/check_dst_coverage.py --check",
    ),
    (
        "frontend_tests",
        "ReportFrontendTests",
        "cd platform && bun run test:run",
    ),
];

#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        ctx.log("info", "gate_verifier: starting");

        let fields = ctx
            .entity_state
            .get("fields")
            .cloned()
            .unwrap_or(json!({}));

        // 1. Read gate_level from integration config ("1" or "2")
        let gate_level = ctx
            .config
            .get("gate_level")
            .cloned()
            .unwrap_or_else(|| "1".to_string());

        // 2. Read sandbox_url — prefer entity field, fall back to trigger_params
        let sandbox_url = fields
            .get("sandbox_url")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .or_else(|| {
                ctx.trigger_params
                    .get("sandbox_url")
                    .and_then(|v| v.as_str())
            })
            .unwrap_or("");

        if sandbox_url.is_empty() {
            return Err("gate_verifier: sandbox_url is required but not set".to_string());
        }

        // 3. Read temper_api_url from config
        let temper_api_url = ctx
            .config
            .get("temper_api_url")
            .filter(|s| !s.is_empty() && !s.contains("{secret:"))
            .cloned()
            .unwrap_or_else(|| "http://127.0.0.1:3000".to_string());

        // Tensorlake API key for sandbox auth
        let api_key = ctx
            .config
            .get("tensorlake_api_key")
            .filter(|s| !s.is_empty() && !s.contains("{secret:"))
            .cloned()
            .unwrap_or_default();

        let entity_id = ctx
            .entity_state
            .get("entity_id")
            .and_then(|v| v.as_str())
            .unwrap_or(&ctx.entity_id);

        let tenant = &ctx.tenant;

        // OData headers for Temper API calls
        let odata_headers = vec![
            ("content-type".to_string(), "application/json".to_string()),
            ("x-tenant-id".to_string(), tenant.to_string()),
            (
                "x-temper-principal-kind".to_string(),
                "agent".to_string(),
            ),
            (
                "x-temper-principal-id".to_string(),
                ctx.entity_id.clone(),
            ),
            ("x-temper-agent-type".to_string(), "system".to_string()),
        ];

        // 4. Select checks based on gate level
        let checks: &[(&str, &str, &str)] = match gate_level.as_str() {
            "2" => LEVEL2_CHECKS,
            _ => LEVEL1_CHECKS,
        };

        ctx.log(
            "info",
            &format!(
                "gate_verifier: running level-{gate_level} checks ({} commands) on sandbox {sandbox_url}",
                checks.len()
            ),
        );

        let mut failures: Vec<String> = Vec::new();

        // 5. Execute each check command in the sandbox
        for (name, action, command) in checks {
            ctx.log("info", &format!("gate_verifier: running check '{name}'"));

            let (exit_code, stdout, stderr) =
                run_sandbox_command(&ctx, sandbox_url, &api_key, command)?;

            let summary = build_summary(&stdout, &stderr, exit_code);

            if exit_code != 0 {
                ctx.log(
                    "warn",
                    &format!("gate_verifier: check '{name}' FAILED (exit code {exit_code})"),
                );
                failures.push(format!("{name}: exit code {exit_code}\n{summary}"));
                // Continue running remaining checks to report all failures
                continue;
            }

            ctx.log(
                "info",
                &format!("gate_verifier: check '{name}' PASSED"),
            );

            // 6. Dispatch Report* action on the WorkCycle
            let action_url = format!(
                "{temper_api_url}/tdata/WorkCycles('{entity_id}')/OpenPaw.Harness.{action}"
            );
            let action_body = json!({
                "ok": "true",
                "summary": summary,
            });
            let action_resp = ctx.http_call(
                "POST",
                &action_url,
                &odata_headers,
                &action_body.to_string(),
            )?;
            if action_resp.status < 200 || action_resp.status >= 300 {
                ctx.log(
                    "warn",
                    &format!(
                        "gate_verifier: {action} dispatch returned HTTP {} (non-fatal)",
                        action_resp.status
                    ),
                );
            }
        }

        // 7. If any failures, return error so on_failure = "Fail" triggers
        if !failures.is_empty() {
            let msg = format!(
                "gate_verifier: {} of {} checks failed:\n{}",
                failures.len(),
                checks.len(),
                failures.join("\n---\n")
            );
            return Err(msg);
        }

        set_success_result(
            "",
            &json!({
                "gate_level": gate_level,
                "checks_passed": checks.len(),
            }),
        );

        ctx.log("info", "gate_verifier: all checks passed");
        Ok(())
    })();

    if let Err(e) = result {
        set_error_result(&e);
    }
    0
}

/// Execute a command in the Tensorlake sandbox and wait for completion.
/// Returns (exit_code, stdout, stderr).
fn run_sandbox_command(
    ctx: &Context,
    sandbox_url: &str,
    api_key: &str,
    command: &str,
) -> Result<(i64, String, String), String> {
    let run_id = short_id();
    let out_path = format!("/tmp/.paw-gate-out-{run_id}");
    let err_path = format!("/tmp/.paw-gate-err-{run_id}");
    let rc_path = format!("/tmp/.paw-gate-rc-{run_id}");

    // Wrap command to capture stdout, stderr, and exit code
    let wrapped = format!(
        "({command}) > {out_path} 2> {err_path}; echo $? > {rc_path}",
    );

    let start_url = format!("{sandbox_url}/api/v1/processes");
    let start_headers = sandbox_headers(api_key, Some("application/json"));
    let body = json!({
        "command": "bash",
        "args": ["-c", &wrapped],
        "cwd": "/workspace",
        "env": {},
    });

    let resp = ctx.http_call("POST", &start_url, &start_headers, &body.to_string())?;
    if resp.status < 200 || resp.status >= 300 {
        return Err(format!(
            "sandbox process start failed (HTTP {}): {}",
            resp.status,
            &resp.body[..resp.body.len().min(300)]
        ));
    }

    // Poll for exit code file (network latency provides natural backoff)
    let rc_url = format!(
        "{sandbox_url}/api/v1/files?path={}",
        url_encode(&rc_path)
    );
    let read_headers = sandbox_headers(api_key, None);
    let mut exit_code: i64 = -1;
    let mut found = false;

    // 600 attempts ~ 300s with network latency as backoff
    for attempt in 0..600 {
        let rc_resp = ctx.http_call("GET", &rc_url, &read_headers, "");
        match rc_resp {
            Ok(r) if r.status >= 200 && r.status < 300 => {
                exit_code = r.body.trim().parse::<i64>().unwrap_or(-1);
                found = true;
                break;
            }
            _ => {
                if attempt % 50 == 0 && attempt > 0 {
                    ctx.log(
                        "info",
                        &format!(
                            "gate_verifier: still waiting for command (attempt {attempt})..."
                        ),
                    );
                }
            }
        }
    }

    if !found {
        cleanup_sandbox_files(ctx, sandbox_url, api_key, &out_path, &err_path, &rc_path);
        return Err("sandbox command timed out after ~300s".to_string());
    }

    // Read stdout and stderr
    let stdout = read_sandbox_file(ctx, sandbox_url, api_key, &out_path).unwrap_or_default();
    let stderr = read_sandbox_file(ctx, sandbox_url, api_key, &err_path).unwrap_or_default();

    // Cleanup temp files (best effort)
    cleanup_sandbox_files(ctx, sandbox_url, api_key, &out_path, &err_path, &rc_path);

    Ok((exit_code, stdout, stderr))
}

/// Read a file from the sandbox via the Tensorlake files API.
fn read_sandbox_file(
    ctx: &Context,
    sandbox_url: &str,
    api_key: &str,
    path: &str,
) -> Result<String, String> {
    let url = format!(
        "{sandbox_url}/api/v1/files?path={}",
        url_encode(path)
    );
    let headers = sandbox_headers(api_key, None);
    let resp = ctx.http_call("GET", &url, &headers, "")?;
    if resp.status >= 200 && resp.status < 300 {
        Ok(resp.body)
    } else {
        Err(format!(
            "read sandbox file failed (HTTP {})",
            resp.status
        ))
    }
}

/// Delete a file from the sandbox (best effort).
fn delete_sandbox_file(ctx: &Context, sandbox_url: &str, api_key: &str, path: &str) {
    let url = format!(
        "{sandbox_url}/api/v1/files?path={}",
        url_encode(path)
    );
    let headers = sandbox_headers(api_key, None);
    let _ = ctx.http_call("DELETE", &url, &headers, "");
}

/// Cleanup all temp files for a command run.
fn cleanup_sandbox_files(
    ctx: &Context,
    sandbox_url: &str,
    api_key: &str,
    out_path: &str,
    err_path: &str,
    rc_path: &str,
) {
    delete_sandbox_file(ctx, sandbox_url, api_key, out_path);
    delete_sandbox_file(ctx, sandbox_url, api_key, err_path);
    delete_sandbox_file(ctx, sandbox_url, api_key, rc_path);
}

/// Build sandbox request headers with optional Bearer auth.
fn sandbox_headers(api_key: &str, content_type: Option<&str>) -> Vec<(String, String)> {
    let mut headers = Vec::new();
    if !api_key.is_empty() {
        headers.push(("authorization".to_string(), format!("Bearer {api_key}")));
    }
    if let Some(ct) = content_type {
        headers.push(("content-type".to_string(), ct.to_string()));
    }
    headers
}

/// Minimal URL encoding for path parameters.
fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 2);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'/' => {
                out.push(b as char);
            }
            _ => {
                out.push_str(&format!("%{b:02X}"));
            }
        }
    }
    out
}

/// Generate a short random-ish ID for temp file paths.
fn short_id() -> String {
    // Use entity_id hash + a counter-like approach.
    // In WASM we don't have proper randomness, so use a simple approach.
    use std::sync::atomic::{AtomicU32, Ordering};
    static COUNTER: AtomicU32 = AtomicU32::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("gv{n:04x}")
}

/// Build a truncated summary from stdout/stderr for reporting.
fn build_summary(stdout: &str, stderr: &str, exit_code: i64) -> String {
    let max_len = 2048;
    let mut summary = String::new();
    if !stdout.is_empty() {
        let truncated = if stdout.len() > max_len {
            &stdout[stdout.len() - max_len..]
        } else {
            stdout
        };
        summary.push_str(truncated);
    }
    if !stderr.is_empty() {
        if !summary.is_empty() {
            summary.push('\n');
        }
        summary.push_str("STDERR: ");
        let truncated = if stderr.len() > max_len {
            &stderr[stderr.len() - max_len..]
        } else {
            stderr
        };
        summary.push_str(truncated);
    }
    if exit_code != 0 {
        summary.push_str(&format!("\n(exit code: {exit_code})"));
    }
    summary
}
