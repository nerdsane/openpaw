#[path = "../../common.rs"]
mod common;

use common::{
    create_entity, entity_id, field_bool, field_string, post_absolute_action, system_json_headers,
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

        let existing = field_string(&fields, &["ComputerId", "computer_id"]);
        if !existing.is_empty() {
            temper_wasm_sdk::set_success_result("BindComputer", &json!({ "ComputerId": existing }));
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
        let allow_mcp_servers = field_bool(&fields, &["AllowMcpServers", "allow_mcp_servers"]);
        let allow_package_managers = field_bool(
            &fields,
            &["AllowPackageManagers", "allow_package_managers"],
        );
        let network_allow = build_network_allow(
            &networking_type,
            &allowed_hosts,
            allow_mcp_servers,
            allow_package_managers,
        );

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

        temper_wasm_sdk::set_success_result("BindComputer", &json!({ "ComputerId": computer_id }));
        Ok(())
    })();

    if let Err(error) = result {
        temper_wasm_sdk::set_error_result(&error);
    }
    0
}

fn build_network_allow(
    networking_type: &str,
    allowed_hosts_json: &str,
    allow_mcp_servers: bool,
    allow_package_managers: bool,
) -> String {
    if networking_type != "Limited" {
        return "*".to_string();
    }

    let allowed_hosts = serde_json::from_str::<Value>(allowed_hosts_json)
        .ok()
        .filter(|value| value.is_array())
        .unwrap_or_else(|| json!([]));

    serde_json::to_string(&json!({
        "allowed_hosts": allowed_hosts,
        "allow_mcp_servers": allow_mcp_servers,
        "allow_package_managers": allow_package_managers,
    }))
    .unwrap_or_else(|_| {
        r#"{"allowed_hosts":[],"allow_mcp_servers":false,"allow_package_managers":false}"#
            .to_string()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_network_allow_for_limited_networking_includes_all_anthropic_subfields() {
        let network_allow = build_network_allow(
            "Limited",
            "[\"github.com\"]",
            true,
            false,
        );

        assert_eq!(
            serde_json::from_str::<Value>(&network_allow).unwrap(),
            json!({
                "allowed_hosts": ["github.com"],
                "allow_mcp_servers": true,
                "allow_package_managers": false,
            })
        );
    }

    #[test]
    fn build_network_allow_for_unrestricted_networking_is_wildcard() {
        assert_eq!(build_network_allow("Unrestricted", "[]", false, false), "*");
    }
}
