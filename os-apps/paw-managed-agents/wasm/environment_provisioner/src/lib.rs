#[path = "../../common.rs"]
mod common;

use common::{create_entity, entity_id, field_string, post_absolute_action, system_json_headers};
use temper_wasm_sdk::prelude::*;
use wasm_helpers::resolve_temper_api_url;

#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        let fields = ctx.entity_state.get("fields").cloned().unwrap_or_else(|| json!({}));
        let base_url = resolve_temper_api_url(&ctx, &fields);
        let headers = system_json_headers(&ctx, &ctx.tenant, &fields);

        let existing = field_string(&fields, &["ComputerId", "computer_id"]);
        if !existing.is_empty() {
            temper_wasm_sdk::set_success_result("BindComputer", &json!({ "computer_id": existing }));
            return Ok(());
        }

        let packages = ctx
            .http_call(
                "GET",
                &format!(
                    "{base_url}/tdata/EnvironmentPackages?$filter=EnvironmentId%20eq%20'{}'&$orderby=Name%20asc",
                    ctx.entity_id.replace('\'', "''")
                ),
                &headers,
                "",
            )
            .ok()
            .and_then(|resp| serde_json::from_str::<Value>(&resp.body).ok())
            .and_then(|parsed| parsed.get("value").and_then(Value::as_array).cloned())
            .unwrap_or_default();

        let tools_installed = packages
            .iter()
            .map(|item| {
                let manager = field_string(item, &["Manager", "manager"]);
                let name = field_string(item, &["Name", "name"]);
                let version = field_string(item, &["Version", "version"]);
                if version.is_empty() {
                    format!("{manager}:{name}")
                } else {
                    format!("{manager}:{name}@{version}")
                }
            })
            .collect::<Vec<_>>()
            .join(",");

        let networking_type = field_string(&fields, &["NetworkingType", "networking_type"]);
        let allowed_hosts = field_string(&fields, &["AllowedHostsJson", "allowed_hosts_json"]);
        let network_allow = match networking_type.as_str() {
            "Limited" => allowed_hosts,
            "Disabled" => "disabled".to_string(),
            _ => "*".to_string(),
        };

        let created = create_entity(&ctx, &base_url, &headers, "Computers", &json!({}))?;
        let computer_id =
            entity_id(&created).ok_or("create Computers did not return an entity id")?;
        let computer_name = {
            let name = field_string(&fields, &["Name", "name"]);
            if name.is_empty() {
                format!("managed-env-{}", ctx.entity_id)
            } else {
                name
            }
        };

        let configure_body = json!({
            "name": computer_name,
            "description": field_string(&fields, &["Description", "description"]),
            "provider": "managed-agents",
            "cpu_cores": 2,
            "memory_gb": 4,
            "storage_gb": 50,
            "base_image": "ubuntu-24.04",
            "tools_installed": tools_installed,
            "credentials_scoped": "managed-agents",
            "network_allow": network_allow,
            "project_harness_id": ctx.entity_id,
        });

        post_absolute_action(
            &ctx,
            &headers,
            &format!("{base_url}/tdata/Computers('{computer_id}')/Paw.Compute.Configure"),
            &configure_body,
            "configure computer",
        )?;
        post_absolute_action(
            &ctx,
            &headers,
            &format!("{base_url}/tdata/Computers('{computer_id}')/Paw.Compute.Provision"),
            &json!({}),
            "provision computer",
        )?;
        post_absolute_action(
            &ctx,
            &headers,
            &format!("{base_url}/tdata/Computers('{computer_id}')/Paw.Compute.ProvisionComplete"),
            &json!({
                "machine_id": computer_id,
                "sandbox_url": "",
                "ssh_host": "",
            }),
            "complete computer provisioning",
        )?;

        temper_wasm_sdk::set_success_result("BindComputer", &json!({ "computer_id": computer_id }));
        Ok(())
    })();

    if let Err(error) = result {
        temper_wasm_sdk::set_error_result(&error);
    }
    0
}
