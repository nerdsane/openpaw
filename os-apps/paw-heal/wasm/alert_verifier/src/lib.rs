//! Alert Verifier — WASM module for AlertCycle.DeployDetected and CheckAlertResolution.
//!
//! Queries the Datadog API to check whether the original monitor alert has
//! resolved after deployment. Self-loops via CheckAlertResolution until the
//! monitor is OK / No Data, or max_checks is exhausted.
//!
//! Build: `cargo build --target wasm32-unknown-unknown --release`

use temper_wasm_sdk::prelude::*;

/// Entry point.
#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        ctx.log("info", "alert_verifier: starting");

        let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));

        // Read entity state
        let monitor_id = fields
            .get("monitor_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let check_count: u32 = fields
            .get("check_count")
            .and_then(|v| v.as_str())
            .unwrap_or("0")
            .parse()
            .unwrap_or(0);
        let max_checks: u32 = ctx
            .config
            .get("max_checks")
            .and_then(|v| v.parse().ok())
            .unwrap_or(5);

        // Read config
        let dd_api_key = ctx
            .config
            .get("dd_api_key")
            .filter(|s| !s.is_empty() && !s.contains("{secret:"))
            .cloned()
            .unwrap_or_default();
        let dd_app_key = ctx
            .config
            .get("dd_app_key")
            .filter(|s| !s.is_empty() && !s.contains("{secret:"))
            .cloned()
            .unwrap_or_default();
        let dd_site = ctx
            .config
            .get("dd_site")
            .filter(|s| !s.is_empty() && !s.contains("{secret:"))
            .cloned()
            .unwrap_or_else(|| "datadoghq.com".to_string());
        let temper_api_url = ctx
            .config
            .get("temper_api_url")
            .filter(|s| !s.is_empty() && !s.contains("{secret:"))
            .cloned()
            .unwrap_or_else(|| "http://127.0.0.1:3000".to_string());

        // If no monitor_id, skip verification
        if monitor_id.is_empty() {
            ctx.log(
                "info",
                "alert_verifier: no monitor_id on AlertCycle, resolving without DD verification",
            );
            set_success_result(
                "AlertResolved",
                &json!({ "diagnosis": "no monitor_id to verify; resolved after deployment" }),
            );
            return Ok(());
        }

        // Query Monitor entity to get dd_monitor_id
        let tenant = &ctx.tenant;
        let api_headers = vec![
            ("content-type".to_string(), "application/json".to_string()),
            ("x-tenant-id".to_string(), tenant.to_string()),
            ("x-temper-principal-kind".to_string(), "admin".to_string()),
        ];

        let monitor_url = format!("{temper_api_url}/tdata/Monitors('{monitor_id}')");
        let monitor_resp = ctx.http_call("GET", &monitor_url, &api_headers, "")?;
        let monitor: Value = if monitor_resp.status >= 200 && monitor_resp.status < 300 {
            serde_json::from_str(&monitor_resp.body).unwrap_or(json!({}))
        } else {
            json!({})
        };

        let dd_monitor_id = monitor
            .get("fields")
            .and_then(|f| f.get("dd_monitor_id"))
            .and_then(|v| v.as_str())
            .or_else(|| monitor.get("dd_monitor_id").and_then(|v| v.as_str()))
            .unwrap_or("");

        if dd_monitor_id.is_empty() {
            ctx.log(
                "info",
                &format!(
                    "alert_verifier: Monitor {monitor_id} has no dd_monitor_id, resolving"
                ),
            );
            set_success_result(
                "AlertResolved",
                &json!({ "diagnosis": format!("Monitor {monitor_id} has no Datadog monitor id; resolved after deployment") }),
            );
            return Ok(());
        }

        // Check DD API keys
        if dd_api_key.is_empty() || dd_app_key.is_empty() {
            set_success_result(
                "AlertPersists",
                &json!({ "diagnosis": format!("Datadog verification unavailable for monitor {dd_monitor_id}: DD API keys missing") }),
            );
            return Ok(());
        }

        // Query Datadog monitor
        let dd_url = format!("https://api.{dd_site}/api/v1/monitor/{dd_monitor_id}");
        let dd_headers = vec![
            ("DD-API-KEY".to_string(), dd_api_key),
            ("DD-APPLICATION-KEY".to_string(), dd_app_key),
            ("accept".to_string(), "application/json".to_string()),
        ];
        let dd_resp = ctx.http_call("GET", &dd_url, &dd_headers, "")?;

        if dd_resp.status >= 200 && dd_resp.status < 300 {
            let dd_body: Value = serde_json::from_str(&dd_resp.body)
                .map_err(|e| format!("alert_verifier: parse DD response: {e}"))?;
            let overall_state = dd_body
                .get("overall_state")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown");

            if matches!(overall_state, "OK" | "No Data") {
                ctx.log(
                    "info",
                    &format!(
                        "alert_verifier: DD monitor {dd_monitor_id} state={overall_state}, resolved"
                    ),
                );
                set_success_result(
                    "AlertResolved",
                    &json!({ "diagnosis": format!("Datadog monitor {dd_monitor_id} post-deploy state={overall_state}") }),
                );
                return Ok(());
            }

            // Still alerting
            ctx.log(
                "info",
                &format!(
                    "alert_verifier: DD monitor {dd_monitor_id} state={overall_state}, still alerting"
                ),
            );
        } else {
            ctx.log(
                "warn",
                &format!(
                    "alert_verifier: DD API returned HTTP {} for monitor {dd_monitor_id}",
                    dd_resp.status
                ),
            );
        }

        // Not resolved yet — retry or give up
        if check_count < max_checks {
            ctx.log(
                "info",
                &format!(
                    "alert_verifier: check {}/{}, retrying",
                    check_count + 1,
                    max_checks
                ),
            );
            set_success_result("CheckAlertResolution", &json!({}));
        } else {
            ctx.log("info", "alert_verifier: max checks exhausted");
            set_success_result(
                "AlertPersists",
                &json!({ "diagnosis": format!("Datadog monitor {dd_monitor_id} still alerting after {max_checks} checks") }),
            );
        }

        Ok(())
    })();

    if let Err(e) = result {
        set_error_result(&e);
    }
    0
}
