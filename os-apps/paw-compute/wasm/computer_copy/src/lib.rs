//! computer_copy — WASM module that makes a live copy of a source computer's
//! sandbox for a CHILD Computer row (ARN-443 C).
//!
//! Fires on the child's ProvisionFromCopy action. At that point the child's
//! `machine_id` field holds the SOURCE's machine (carried in by the Copy spawn's
//! copy_fields) — the machine to copy from. This module copies it via the shared
//! provider abstraction and reports CopyComplete(machine_id, sandbox_url,
//! source_machine_id), where machine_id is now the NEW copy's machine and
//! source_machine_id records what it was copied from. On failure the trigger's
//! on_failure routes to CopyFailed.
//!
//! Build: `cargo build --target wasm32-wasip1 --release`.

use temper_wasm_sdk::prelude::*;
use wasm_helpers::entity_field_str;
use wasm_helpers::sandbox::{self, normalize_sandbox_provider};

/// Max seconds to wait for the copy to become ready (under the caller's budget).
const COPY_READY_TIMEOUT_SECS: u64 = 240;

#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));

        let provider = provider_from_fields(&fields);
        let source_machine_id = entity_field_str(&fields, &["machine_id", "MachineId"])
            .filter(|s| !s.is_empty())
            .ok_or("computer_copy: no source machine_id to copy from")?
            .to_string();

        ctx.log(
            "info",
            &format!(
                "computer_copy: copying {source_machine_id} for child {}",
                ctx.entity_id
            ),
        );

        let handle = sandbox::sandbox_copy(&ctx, &provider, &source_machine_id, COPY_READY_TIMEOUT_SECS)?;

        set_success_result(
            "CopyComplete",
            &json!({
                "machine_id": handle.sandbox_id,
                "sandbox_url": handle.sandbox_url,
                "source_machine_id": source_machine_id,
            }),
        );
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_defaults_to_tensorlake() {
        assert_eq!(provider_from_fields(&json!({})), "tensorlake");
        assert_eq!(provider_from_fields(&json!({"provider": ""})), "tensorlake");
    }

    #[test]
    fn provider_normalizes_and_reads() {
        assert_eq!(provider_from_fields(&json!({"provider": "tl"})), "tensorlake");
        assert_eq!(provider_from_fields(&json!({"provider": "modal"})), "modal");
        assert_eq!(provider_from_fields(&json!({"Provider": "TensorLake"})), "tensorlake");
    }
}
