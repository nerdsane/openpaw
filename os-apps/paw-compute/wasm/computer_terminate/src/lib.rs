//! computer_terminate — WASM module that tears down a computer's sandbox
//! (ARN-443 C).
//!
//! Fires on Destroy (Computer → Terminating). Terminates the row's OWN sandbox
//! (`machine_id`) via the shared provider abstraction, then reports
//! TerminateComplete → Destroyed. Best-effort and idempotent: an empty machine_id
//! (e.g. a never-provisioned row) is a no-op, and a provider error is logged but
//! still reports TerminateComplete so the row always reaches Destroyed. A leaked
//! sandbox is caught by the panel's stale-copy reaper.
//!
//! Safety (ARN-443 C5, provenance): this module calls the provider terminate ONLY
//! when the row OWNS its machine (`owned_machine` — set at ProvisionComplete or
//! CopyDiscovered). A child copy carries the inherited SOURCE machine_id until it
//! discovers its own (owned_machine stays false through Created→Provisioning→
//! Copying-before-discovery), so even if a teardown reached it, the source's sandbox
//! can NEVER be terminated through the child. This is defense in depth behind the
//! spec guard that keeps Destroy out of Provisioning; it does not rely on the
//! mutable machine_id field alone.
//!
//! Build: `cargo build --target wasm32-wasip1 --release`.

use temper_wasm_sdk::prelude::*;
use wasm_helpers::entity_field_str;
use wasm_helpers::sandbox::{self, normalize_sandbox_provider};

#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));

        let provider = provider_from_fields(&fields);
        let machine_id = entity_field_str(&fields, &["machine_id", "MachineId"])
            .unwrap_or("")
            .to_string();
        let owns_machine = bool_field(&fields, &["owned_machine", "OwnedMachine"]);

        if machine_id.is_empty() {
            ctx.log(
                "info",
                &format!(
                    "computer_terminate: {} has no machine_id — nothing to terminate",
                    ctx.entity_id
                ),
            );
        } else if !owns_machine {
            // The row does not own this machine (e.g. a copy child still carrying the
            // inherited SOURCE machine_id before discovery). NEVER call the provider —
            // terminating here could tear down a source's sandbox.
            ctx.log(
                "info",
                &format!(
                    "computer_terminate: {} does not own machine {machine_id} (owned_machine=false) — refusing provider terminate",
                    ctx.entity_id
                ),
            );
        } else if let Err(e) = sandbox::sandbox_terminate(&ctx, &provider, &machine_id) {
            // Best-effort: log, but never block Destroyed on a terminate error.
            ctx.log(
                "error",
                &format!("computer_terminate: terminate {machine_id} failed: {e}"),
            );
        } else {
            ctx.log(
                "info",
                &format!("computer_terminate: terminated {machine_id}"),
            );
        }

        set_success_result("TerminateComplete", &json!({}));
        Ok(())
    })();

    if let Err(e) = result {
        set_error_result(&e);
    }
    0
}

/// The provider recorded on the row, normalized; defaults to tensorlake.
fn provider_from_fields(fields: &Value) -> String {
    entity_field_str(fields, &["provider", "Provider"])
        .filter(|s| !s.is_empty())
        .map(normalize_sandbox_provider)
        .unwrap_or_else(|| "tensorlake".to_string())
}

/// Read a bool field that may be stored as a JSON bool or a "true"/"false" string.
fn bool_field(fields: &Value, keys: &[&str]) -> bool {
    for k in keys {
        match fields.get(k) {
            Some(Value::Bool(b)) => return *b,
            Some(Value::String(s)) => return s == "true",
            _ => {}
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_defaults_to_tensorlake() {
        assert_eq!(provider_from_fields(&json!({})), "tensorlake");
        assert_eq!(provider_from_fields(&json!({"provider": "tl"})), "tensorlake");
        assert_eq!(provider_from_fields(&json!({"provider": "modal"})), "modal");
    }

    #[test]
    fn owned_machine_gate_reads_bool_and_string() {
        // A copy child pre-discovery (owned_machine false / absent) must NOT be
        // treated as owning its machine.
        assert!(!bool_field(&json!({"machine_id": "src-123"}), &["owned_machine"]));
        assert!(!bool_field(&json!({"owned_machine": "false"}), &["owned_machine"]));
        // A provisioned or discovered row owns its machine.
        assert!(bool_field(&json!({"owned_machine": "true"}), &["owned_machine"]));
        assert!(bool_field(&json!({"owned_machine": true}), &["owned_machine"]));
    }
}
