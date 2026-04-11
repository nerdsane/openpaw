//! Coding Agent Runner — WASM module for spawning coding agent CLI processes.
//!
//! Maps agent_type to CLI commands and executes them in the sandbox.
//! Supports claude-code, codex, pi, and opencode.
//! Uses wasm-helpers sandbox abstraction for provider-agnostic execution.

use temper_wasm_sdk::prelude::*;
use wasm_helpers::sandbox::{self, SandboxHandle};

#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        ctx.log("info", "coding_agent_runner: starting");

        let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));
        let sandbox_url = fields.get("sandbox_url").and_then(|v| v.as_str()).unwrap_or("");
        let sandbox_id = fields.get("sandbox_id").and_then(|v| v.as_str()).unwrap_or("");
        let sandbox_provider = fields.get("sandbox_provider").and_then(|v| v.as_str()).unwrap_or("");
        let workdir = fields.get("workdir").and_then(|v| v.as_str()).unwrap_or("/workspace");

        if sandbox_url.is_empty() {
            return Err("coding_agent_runner: sandbox_url is empty".to_string());
        }

        let provider = if sandbox_provider.is_empty() {
            sandbox::resolve_sandbox_provider(&ctx, &fields)?
        } else {
            sandbox_provider.to_string()
        };

        let handle = SandboxHandle {
            sandbox_url: sandbox_url.to_string(),
            sandbox_id: sandbox_id.to_string(),
            provider,
        };

        // Read tool input from trigger params
        let input = ctx.trigger_params.get("input").cloned().unwrap_or(json!({}));
        let agent_type = input.get("agent_type").and_then(|v| v.as_str()).unwrap_or("claude-code");
        let task = input.get("task").and_then(|v| v.as_str()).unwrap_or("");
        let task_workdir = input.get("workdir").and_then(|v| v.as_str()).unwrap_or(workdir);

        if task.is_empty() {
            return Err("coding_agent_runner: task is empty".to_string());
        }

        // Map agent_type to CLI command
        let command = match agent_type {
            "claude-code" => format!("claude --permission-mode bypassPermissions --print '{}'", escape_single_quotes(task)),
            "codex" => format!("codex exec '{}'", escape_single_quotes(task)),
            "pi" => format!("pi -p '{}'", escape_single_quotes(task)),
            "opencode" => format!("opencode run '{}'", escape_single_quotes(task)),
            other => return Err(format!("coding_agent_runner: unsupported agent_type: {other}")),
        };

        ctx.log("info", &format!("coding_agent_runner: running {agent_type} via {} provider: {}", handle.provider, &command[..command.len().min(100)]));

        let result = sandbox::sandbox_exec(&ctx, &handle, &command, task_workdir)?;

        // Format output
        let mut output = String::new();
        if !result.stdout.is_empty() { output.push_str(&result.stdout); }
        if !result.stderr.is_empty() {
            if !output.is_empty() { output.push('\n'); }
            output.push_str("STDERR: ");
            output.push_str(&result.stderr);
        }
        if result.exit_code != 0 {
            output.push_str(&format!("\n(exit code: {})", result.exit_code));
        }

        // Return the output as a tool result
        set_success_result("HandleToolResults", &json!({
            "pending_tool_calls": json!([{
                "type": "tool_result",
                "tool_use_id": input.get("tool_use_id").and_then(|v| v.as_str()).unwrap_or("unknown"),
                "content": output,
            }]).to_string(),
        }));

        Ok(())
    })();

    if let Err(e) = result {
        set_error_result(&e);
    }
    0
}

fn escape_single_quotes(s: &str) -> String {
    s.replace('\'', "'\\''")
}
