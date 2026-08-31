//! computer_copy_start — INITIATE an async live-copy of a source computer's sandbox
//! for a CHILD Computer row (ARN-443 C; C5 structural: initiate only).
//!
//! Fires on the child's ProvisionFromCopy. The child's `machine_id` field holds the
//! SOURCE's machine (carried by the Copy spawn's copy_fields) — the machine to copy
//! from. This module has ONE concern: fire the copy POST. Discovery of the created
//! sandbox and readiness both happen later in the Copying poll (computer_copy_poll)
//! across invocations — the tensorlake copy API is synchronous and a real copy takes
//! minutes, far past the ~120s WASM cap, so nothing here waits for the copy.
//!
//! On success it reports CopyStarted (recording only the poll deadline; machine_id
//! stays the source's until CopyDiscovered). A 4xx from the POST — including the
//! provider's 409 when a `<source>-copy` already exists — is a definitive failure and
//! the trigger's on_failure routes to CopyFailed (which does NOT terminate; the
//! machine_id is still the source's here). That 409-on-duplicate is also what makes
//! discovery unambiguous: a successful initiate proves no `<source>-copy` existed
//! before, so the one the poll discovers is ours.
//!
//! Build: `cargo build --target wasm32-wasip1 --release`.

use temper_wasm_sdk::prelude::*;
use wasm_helpers::entity_field_str;
use wasm_helpers::sandbox::{self, normalize_sandbox_provider};

// The tensorlake copy API is SYNCHRONOUS (blocks until the copy is fully ready).
// This short wait just needs the copy created server-side; the Copying poll then
// discovers it and drives readiness — so the initiate fits well under the ~120s
// WASM cap (a full synchronous wait was the R1 bug, a too-short wait that failed on
// the real API was the C5 bug; initiate-then-poll is the fix).
const COPY_START_WAIT_SECS: u64 = 5;
/// Wall-clock budget (ms) for the copy to be discovered AND become ready, stamped on
/// the row so the poll reads a single deadline. A live-copy of a real box takes
/// minutes.
const COPY_READY_BUDGET_MS: i64 = 300_000;

#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));

        let provider = provider_from_fields(&fields);
        let source_machine_id = entity_field_str(&fields, &["machine_id", "MachineId"])
            .filter(|s| !s.is_empty())
            .ok_or("computer_copy_start: no source machine_id to copy from")?
            .to_string();

        ctx.log(
            "info",
            &format!(
                "computer_copy_start: initiating copy of {source_machine_id} for child {}",
                ctx.entity_id
            ),
        );

        // Fire the copy POST. A 4xx (incl. the provider's 409 on a duplicate
        // <source>-copy) is a definitive failure -> CopyFailed via on_failure.
        sandbox::sandbox_copy_initiate(
            &ctx,
            &provider,
            &source_machine_id,
            COPY_START_WAIT_SECS,
        )?;

        let deadline_at_ms = Context::get_time_millis() + COPY_READY_BUDGET_MS;
        set_success_result(
            "CopyStarted",
            &json!({ "copy_deadline_at_ms": deadline_at_ms.to_string() }),
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
    fn provider_defaults_and_normalizes() {
        assert_eq!(provider_from_fields(&json!({})), "tensorlake");
        assert_eq!(provider_from_fields(&json!({"provider": "tl"})), "tensorlake");
        assert_eq!(provider_from_fields(&json!({"provider": "modal"})), "modal");
    }
}
