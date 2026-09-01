//! computer_copy_start — INITIATE a live-copy and record the created copy's id
//! (ARN-443 C; C5 R3: use only a provably-ours id, no name discovery).
//!
//! Fires on the child's ProvisionFromCopy. The child's `machine_id` field holds the
//! SOURCE's machine (carried by the Copy spawn's copy_fields) — the machine to copy
//! from. This fires the copy POST and, ONLY on a clean 2xx that returns a copy id,
//! reports CopyStarted with the COPY's own id/url and owned_machine=true; the Copying
//! poll then drives readiness on that id. Any non-clean result — 4xx (incl. the
//! provider's 409 on a duplicate `<source>-copy`), 5xx, a gateway/read timeout, a
//! transport error, or a 2xx without an id — is UNPROVABLE and routes to CopyFailed
//! (retry-later) via the trigger's on_failure. Rationale in `sandbox_copy_initiate`:
//! adopting a copy we cannot prove we created could make our lease reaper terminate a
//! sandbox another live review depends on, so we fail toward a (reaper-collected) leak
//! and never toward adoption.
//!
//! Build: `cargo build --target wasm32-wasip1 --release`.

use temper_wasm_sdk::prelude::*;
use wasm_helpers::entity_field_str;
use wasm_helpers::sandbox::{self, normalize_sandbox_provider};

// How long the copy POST waits for the (synchronous) copy to complete and return its
// id. Must be under the ~120s WASM invocation cap. A copy that does not return a clean
// 2xx-with-id within this window fails closed (CopyFailed, retry-later) rather than
// being adopted by name — the C5 real-provider drive confirms whether a real copy
// (e.g. arni-big, ~1 min) returns its id in time; if the provider gateway times out
// below the copy duration regardless, the true fix is a provider async-create that
// returns the id immediately (the standing upstream ask).
const COPY_START_WAIT_SECS: u64 = 90;
/// Wall-clock budget (ms) for the recorded copy to become READY, stamped on the row so
/// the readiness poll reads a single deadline.
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

        // Clean-2xx-with-id or bust. Err -> CopyFailed via on_failure (retry-later).
        let handle =
            sandbox::sandbox_copy_initiate(&ctx, &provider, &source_machine_id, COPY_START_WAIT_SECS)?;

        let deadline_at_ms = Context::get_time_millis() + COPY_READY_BUDGET_MS;
        set_success_result(
            "CopyStarted",
            &json!({
                "machine_id": handle.sandbox_id,
                "sandbox_url": handle.sandbox_url,
                "source_machine_id": source_machine_id,
                "name": copy_name(&ctx.entity_id),
                "copy_deadline_at_ms": deadline_at_ms.to_string(),
            }),
        );
        Ok(())
    })();

    if let Err(e) = result {
        set_error_result(&e);
    }
    0
}

/// A distinct name for the child copy — NEVER the source's name (an attach key).
/// Derived from the child's own entity id, so it is unique per copy and marked.
fn copy_name(entity_id: &str) -> String {
    let short: String = entity_id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .take(12)
        .collect();
    if short.is_empty() {
        "copy".to_string()
    } else {
        format!("copy-{short}")
    }
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

    #[test]
    fn copy_name_is_distinct_and_never_empty() {
        assert_eq!(copy_name("abc123def456ghi"), "copy-abc123def456");
        assert_eq!(copy_name(""), "copy");
        assert_ne!(copy_name("arni-big"), "arni-big");
    }
}
