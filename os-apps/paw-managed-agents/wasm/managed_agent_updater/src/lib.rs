#[path = "../../common.rs"]
mod common;

use common::{
    field_i64, field_string, get_entity, managed_tools_enabled, patch_entity,
    post_absolute_action, system_json_headers,
};
use temper_wasm_sdk::prelude::*;
use wasm_helpers::resolve_temper_api_url;

#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        let fields = ctx.entity_state.get("fields").cloned().unwrap_or_else(|| json!({}));
        let base_url = resolve_temper_api_url(&ctx, &fields);
        let headers = system_json_headers(&ctx, &ctx.tenant, &fields);
        let next_version = next_version(&fields);

        let _ = patch_entity(
            &ctx,
            &base_url,
            &headers,
            "ManagedAgents",
            &ctx.entity_id,
            &json!({ "Version": next_version }),
        )?;

        let inner_agent_id = field_string(&fields, &["InnerAgentId", "inner_agent_id"]);
        if !inner_agent_id.is_empty() {
            let tool_rows = common::list_entities(
                &ctx,
                &base_url,
                &headers,
                &format!(
                    "/tdata/AgentTools?$filter=AgentId%20eq%20'{}'&$orderby=Kind%20asc",
                    ctx.entity_id.replace('\'', "''")
                ),
            )?;
            let tool_config_rows = common::list_entities(
                &ctx,
                &base_url,
                &headers,
                "/tdata/AgentToolConfigs?$orderby=ToolName%20asc",
            )?;
            let tools_enabled = managed_tools_enabled(&tool_rows, &tool_config_rows);
            let current_agent = get_entity(&ctx, &base_url, &headers, "ManagedAgents", &ctx.entity_id)?;
            let current_fields = current_agent
                .get("fields")
                .cloned()
                .unwrap_or_else(|| current_agent.clone());
            let model_id = {
                let value = field_string(&current_fields, &["ModelId", "model_id"]);
                if value.is_empty() {
                    "claude-sonnet-4-6".to_string()
                } else {
                    value
                }
            };
            let _ = post_absolute_action(
                &ctx,
                &headers,
                &format!("{base_url}/tdata/Agents('{inner_agent_id}')/OpenPaw.Update"),
                &json!({
                    "description": field_string(&current_fields, &["Description", "description"]),
                    "model": model_id,
                    "tools_enabled": tools_enabled,
                    "max_turns": "60",
                }),
                "update inner agent after managed-agent change",
            )?;
        }

        temper_wasm_sdk::set_success_result("", &json!({"updated": true}));
        Ok(())
    })();

    if let Err(error) = result {
        temper_wasm_sdk::set_error_result(&error);
    }
    0
}

fn next_version(fields: &Value) -> i64 {
    let current = field_i64(fields, &["Version", "version"]);
    if current < 1 {
        1
    } else {
        current + 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn updater_uses_canonical_managed_agent_fields_for_inner_agent_sync() {
        let fields = json!({
            "Description": "after desc",
            "ModelId": "claude-opus-4-1",
        });

        assert_eq!(field_string(&fields, &["Description", "description"]), "after desc");
        assert_eq!(field_string(&fields, &["ModelId", "model_id"]), "claude-opus-4-1");
    }

    #[test]
    fn next_version_advances_positive_versions() {
        assert_eq!(next_version(&json!({"Version": 1})), 2);
        assert_eq!(next_version(&json!({"Version": 7})), 8);
        assert_eq!(next_version(&json!({})), 1);
    }
}
