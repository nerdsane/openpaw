//! Cron Trigger — WASM module for firing scheduled session runs.
//!
//! Creates a new Session entity with the cron job's configuration,
//! including template variable substitution.

use temper_wasm_sdk::prelude::*;
use wasm_helpers::{resolve_temper_api_url, runtime_headers};

#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        ctx.log("info", "cron_trigger: starting");

        let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));
        let temper_api_url = resolve_temper_api_url(&ctx, &fields);
        let tenant = &ctx.tenant;

        // Read cron job configuration
        let soul_id = fields.get("soul_id").and_then(|v| v.as_str()).unwrap_or("");
        let system_prompt = fields.get("system_prompt").and_then(|v| v.as_str()).unwrap_or("");
        let user_message_template = fields.get("user_message_template").and_then(|v| v.as_str()).unwrap_or("");
        let model = fields.get("model").and_then(|v| v.as_str()).unwrap_or("claude-sonnet-4-6");
        let provider = fields.get("provider").and_then(|v| v.as_str()).unwrap_or("anthropic");
        let tools_enabled = fields.get("tools_enabled").and_then(|v| v.as_str()).unwrap_or("read,write,edit,bash");
        let sandbox_url = fields.get("sandbox_url").and_then(|v| v.as_str()).unwrap_or("");
        let run_count = fields.get("run_count").and_then(|v| v.as_i64()).unwrap_or(0);
        let last_result = fields.get("last_result").and_then(|v| v.as_str()).unwrap_or("");

        // Template substitution
        let user_message = user_message_template
            .replace("{{run_count}}", &run_count.to_string())
            .replace("{{last_result}}", last_result)
            .replace("{{now}}", ""); // timestamp injected by cron_scheduler before trigger

        ctx.log("info", &format!("cron_trigger: creating agent for run #{}", run_count));

        let headers = runtime_headers(&ctx, tenant, &fields, Some("application/json"), None);

        // 1. Create Agent entity
        let create_url = format!("{temper_api_url}/tdata/Sessions");
        let create_resp = ctx.http_call("POST", &create_url, &headers, "{}")?;
        if create_resp.status < 200 || create_resp.status >= 300 {
            return Err(format!("Failed to create agent (HTTP {}): {}", create_resp.status, &create_resp.body[..create_resp.body.len().min(200)]));
        }

        let agent: Value = serde_json::from_str(&create_resp.body)
            .map_err(|e| format!("Failed to parse agent response: {e}"))?;
        let agent_id = agent
            .get("entity_id")
            .or_else(|| agent.get("Id"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if agent_id.is_empty() {
            return Err("Failed to extract created agent ID".to_string());
        }

        // 2. Configure the agent
        let configure_url = format!(
            "{temper_api_url}/tdata/Sessions('{agent_id}')/OpenPaw.Configure"
        );
        let configure_body = json!({
            "system_prompt": system_prompt,
            "user_message": user_message,
            "model": model,
            "provider": provider,
            "tools_enabled": tools_enabled,
            "sandbox_url": sandbox_url,
            "soul_id": soul_id,
        });
        let configure_resp = ctx.http_call("POST", &configure_url, &headers, &configure_body.to_string())?;
        if configure_resp.status < 200 || configure_resp.status >= 300 {
            return Err(format!("Failed to configure agent (HTTP {})", configure_resp.status));
        }

        // 3. Provision the agent
        let provision_url = format!(
            "{temper_api_url}/tdata/Sessions('{agent_id}')/OpenPaw.Provision"
        );
        let provision_resp = ctx.http_call("POST", &provision_url, &headers, "{}")?;
        if provision_resp.status < 200 || provision_resp.status >= 300 {
            return Err(format!("Failed to provision agent (HTTP {})", provision_resp.status));
        }

        ctx.log("info", &format!("cron_trigger: agent {} created and provisioned", agent_id));

        set_success_result("TriggerComplete", &json!({
            "last_session_id": agent_id,
            "last_result": "",
        }));

        Ok(())
    })();

    if let Err(e) = result {
        set_error_result(&e);
    }
    0
}
