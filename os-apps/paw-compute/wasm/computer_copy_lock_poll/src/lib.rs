//! computer_copy_lock_poll — release the SOURCE's per-source copy lock (ARN-443 C5).
//!
//! Fires on Computer.CopyLockPoll (driven by the CopyInFlight state_timeout on a
//! SOURCE). The source took the lock (Ready -> CopyInFlight) when it spawned a copy
//! child, storing the child's id in `inflight_copy_id`. This module READS that child
//! and decides when the lock may release — the source sequences its OWN unlock from
//! a read; it never dispatches a transition on another entity (CLAUDE.md: sequencing
//! belongs to the state machine).
//!
//! Release (CopyUnlock -> Ready) when the copy is no longer using the shared
//! "<source>-copy" name ambiguously — i.e. the child OWNS its own copy machine
//! (owned_machine) — or the child has failed / vanished, or the liveness cap is hit.
//! Otherwise report an empty callback so the CopyInFlight timeout re-arms.
//!
//! Premature release is safe: a second Copy's INITIATE would hit the provider's 409
//! on the still-present "<source>-copy" name, so a source is never double-copied.
//! Hence the trigger's on_failure is CopyUnlock (fail OPEN) — a flaky child read
//! never wedges a source out of Ready.
//!
//! Build: `cargo build --target wasm32-wasip1 --release`.

use temper_wasm_sdk::prelude::*;
use wasm_helpers::{bounded_reads, entity_field_str, odata_headers, resolve_temper_api_url};

#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let ctx = match Context::from_host() {
        Ok(c) => c,
        Err(e) => {
            set_error_result(&e);
            return 0;
        }
    };
    let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));

    // Liveness cap (session_link idiom: the count lives in the spec). Past it, the
    // source unlocks regardless so the lock can never wedge it out of Ready.
    let polls = num_field(&fields, &["copy_lock_polls", "CopyLockPolls"]);
    let max_polls = num_field(&fields, &["max_lock_polls", "MaxLockPolls"]).max(1);
    if polls >= max_polls {
        ctx.log("info", "computer_copy_lock_poll: liveness cap reached — releasing lock");
        set_success_result("CopyUnlock", &json!({}));
        return 0;
    }

    let child_id = entity_field_str(&fields, &["inflight_copy_id", "InflightCopyId"])
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let child_id = match child_id {
        Some(id) => id,
        None => {
            // No child recorded — nothing to wait for; release.
            ctx.log("info", "computer_copy_lock_poll: no inflight_copy_id — releasing lock");
            set_success_result("CopyUnlock", &json!({}));
            return 0;
        }
    };

    // Read the child. A missing / unreadable child surfaces as Err and the trigger's
    // on_failure (CopyUnlock) fails open — safe, per the module doc.
    let child = match read_child(&ctx, &fields, &child_id) {
        Ok(c) => c,
        Err(e) => {
            set_error_result(&format!("computer_copy_lock_poll: read child {child_id}: {e}"));
            return 0;
        }
    };
    let cf = child.get("fields").unwrap_or(&child);
    let status = entity_field_str(cf, &["status", "Status"]).unwrap_or("");
    let child_owns = bool_field(cf, &["owned_machine", "OwnedMachine"]);

    // Release once the child owns its own machine (discovery done — the shared name
    // is no longer ambiguous) or the child is terminal.
    if child_owns || matches!(status, "Leased" | "Destroyed" | "Terminating") {
        ctx.log(
            "info",
            &format!("computer_copy_lock_poll: child {child_id} settled (status={status}, owned={child_owns}) — releasing lock"),
        );
        set_success_result("CopyUnlock", &json!({}));
    } else {
        set_success_result("", &json!({})); // still in flight — re-arm
    }
    0
}

/// GET a single Computer row via the Temper loopback.
fn read_child(ctx: &Context, fields: &Value, child_id: &str) -> Result<Value, String> {
    let api = resolve_temper_api_url(ctx, fields);
    let headers = odata_headers(ctx, &ctx.tenant, fields);
    let path = format!("/tdata/Computers('{child_id}')");
    bounded_reads::get_json(ctx, &api, &path, &headers, "computer_copy_lock_poll.child")
}

/// Read a numeric field stored as a JSON number or a decimal string; 0 if absent.
fn num_field(fields: &Value, keys: &[&str]) -> i64 {
    for k in keys {
        match fields.get(k) {
            Some(Value::Number(n)) => return n.as_i64().unwrap_or(0),
            Some(Value::String(s)) => {
                if let Ok(i) = s.parse::<i64>() {
                    return i;
                }
            }
            _ => {}
        }
    }
    0
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
    fn num_field_reads_number_and_string() {
        assert_eq!(num_field(&json!({"copy_lock_polls": 5}), &["copy_lock_polls"]), 5);
        assert_eq!(num_field(&json!({"copy_lock_polls": "7"}), &["copy_lock_polls"]), 7);
        assert_eq!(num_field(&json!({}), &["copy_lock_polls"]), 0);
    }

    #[test]
    fn bool_field_reads_bool_and_string() {
        assert!(bool_field(&json!({"owned_machine": true}), &["owned_machine"]));
        assert!(bool_field(&json!({"owned_machine": "true"}), &["owned_machine"]));
        assert!(!bool_field(&json!({"owned_machine": "false"}), &["owned_machine"]));
    }
}
