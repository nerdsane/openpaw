//! CICD Initiator — WASM module for the AlertCycle.HealComplete (begin_cicd) integration.
//!
//! Checks whether a PR URL was provided with the HealComplete action.
//! If a PR exists, triggers BeginMerge to start the CI/CD closure flow.
//! If no PR, returns a no-op (the cycle stays in Fixed state).
//!
//! Build: `cargo build --target wasm32-unknown-unknown --release`

use temper_wasm_sdk::prelude::*;

/// Entry point.
#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        ctx.log("info", "cicd_initiator: starting");

        // Read pr_url from trigger params (set by the HealComplete action)
        let pr_url = ctx
            .trigger_params
            .get("pr_url")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if pr_url.is_empty() {
            // No PR — nothing to merge. Succeed without dispatching a follow-up action.
            ctx.log("info", "cicd_initiator: no pr_url, skipping CI/CD");
            set_success_result("", &json!({ "status": "skipped" }));
            return Ok(());
        }

        ctx.log(
            "info",
            &format!("cicd_initiator: PR detected, initiating merge: {pr_url}"),
        );

        // Trigger the BeginMerge action to start CI/CD closure
        set_success_result("BeginMerge", &json!({ "pr_url": pr_url }));

        Ok(())
    })();

    if let Err(e) = result {
        set_error_result(&e);
    }
    0
}
