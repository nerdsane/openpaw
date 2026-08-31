//! computer_copy_start — START an async live-copy of a source computer's sandbox
//! for a CHILD Computer row (ARN-443 C, async copy).
//!
//! Fires on the child's ProvisionFromCopy. The child's `machine_id` field holds
//! the SOURCE's machine (carried by the Copy spawn's copy_fields) — the machine to
//! copy from. A real live-copy takes MINUTES, far past the ~120s WASM invocation
//! cap, so this module only KICKS OFF the copy (a short-timeout POST that returns
//! the new sandbox id promptly) and reports CopyStarted; computer_copy_poll then
//! drives readiness from the Copying state across invocations. On failure the
//! trigger's on_failure routes to CopyFailed (which does NOT terminate — the
//! machine_id is still the source's at that point).
//!
//! Build: `cargo build --target wasm32-wasip1 --release`.

use temper_wasm_sdk::prelude::*;
use wasm_helpers::entity_field_str;
use wasm_helpers::sandbox::{self, normalize_sandbox_provider};

/// Wall-clock budget (ms) for the copy to become ready, stamped on the row so the
/// poll reads a single deadline. A live-copy of a real box takes minutes.
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
                "computer_copy_start: starting copy of {source_machine_id} for child {}",
                ctx.entity_id
            ),
        );

        // Kick off the copy; returns the new sandbox id promptly (may still boot).
        let handle =
            sandbox::sandbox_copy(
                &ctx,
                &provider,
                &source_machine_id,
                sandbox::COPY_START_MAX_WAIT_SECS,
            )?;

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

/// A distinct name for the child copy — NEVER the source's name (which is an
/// attach/resolution key). Derived from the child's own entity id, so it is unique
/// per copy and clearly marked.
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
        // never equals a plausible source name
        assert_ne!(copy_name("arni-big"), "arni-big");
    }
}
