//! computer_copy_lock_poll — release the SOURCE's per-source copy lock (ARN-443 C5).
//!
//! Fires on Computer.CopyLockPoll (driven by the CopyInFlight state_timeout on a
//! SOURCE). The source took the lock (Ready -> CopyInFlight) when it spawned a copy
//! child, storing the child's id in `inflight_copy_id`. This module READS that child
//! and decides when the lock may release — the source sequences its OWN unlock from
//! a read; it never dispatches a transition on another entity (CLAUDE.md: sequencing
//! belongs to the state machine).
//!
//! Release (CopyUnlock -> Ready) ONLY on a definitive signal: the child OWNS its own
//! copy machine (owned_machine — discovery done, the shared "<source>-copy" name is no
//! longer ambiguous), the child is terminal/vanished, or the bounded liveness cap
//! (copy_lock_polls >= max_lock_polls) is hit. Everything else — still in flight, or a
//! transient child-read error — reports an empty callback so the CopyInFlight timeout
//! re-arms and the source STAYS LOCKED.
//!
//! FAIL-CLOSED (R2 #3): a transient read error must NOT release, because releasing
//! while the child is still discovering by name would reopen the ambiguity the lock
//! exists to prevent. The module never set_errors; the trigger's on_failure is
//! CopyLockContinue (re-arm, stay locked), and the liveness cap is the only bounded
//! escape. Every release zeroes copy_lock_polls (reset_polls) so the next copy from
//! this source starts the cap fresh (R2 #1).
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
    // source unlocks regardless so the lock can never wedge it out of Ready — this is
    // the bounded-deadline release, and it holds even if the child read keeps failing
    // (the count is incremented by the spec each tick, checked here every run).
    let polls = num_field(&fields, &["copy_lock_polls", "CopyLockPolls"]);
    let max_polls = num_field(&fields, &["max_lock_polls", "MaxLockPolls"]).max(1);
    if polls >= max_polls {
        ctx.log("info", "computer_copy_lock_poll: liveness cap reached — releasing lock");
        unlock();
        return 0;
    }

    let child_id = entity_field_str(&fields, &["inflight_copy_id", "InflightCopyId"])
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let child_id = match child_id {
        Some(id) => id,
        None => {
            // No child recorded — nothing to wait for; release (definitive).
            ctx.log("info", "computer_copy_lock_poll: no inflight_copy_id — releasing lock");
            unlock();
            return 0;
        }
    };

    // Read the child. A transient read error is NOT a release signal (that would
    // reopen the ambiguity the lock prevents): re-arm and let the liveness cap be the
    // only bounded escape (R2 #3). We never set_error here.
    let child = match read_child(&ctx, &fields, &child_id) {
        Ok(c) => c,
        Err(e) => {
            ctx.log("info", &format!("computer_copy_lock_poll: child {child_id} read failed, staying locked: {e}"));
            set_success_result("", &json!({})); // transient — re-arm, stay locked
            return 0;
        }
    };
    let cf = child.get("fields").unwrap_or(&child);
    let status = entity_field_str(cf, &["status", "Status"]).unwrap_or("");
    let child_owns = bool_field(cf, &["owned_machine", "OwnedMachine"]);

    // Release ONLY on a definitive child state: it owns its own machine (discovery
    // done — the shared name is no longer ambiguous) or it is terminal.
    if child_owns || matches!(status, "Leased" | "Destroyed" | "Terminating") {
        ctx.log(
            "info",
            &format!("computer_copy_lock_poll: child {child_id} settled (status={status}, owned={child_owns}) — releasing lock"),
        );
        unlock();
    } else {
        set_success_result("", &json!({})); // still in flight — re-arm
    }
    0
}

/// Release the lock (CopyUnlock) and zero the poll counter so the NEXT copy from this
/// source starts the liveness cap fresh (R2 #1).
fn unlock() {
    set_success_result("CopyUnlock", &json!({ "reset_polls": "0" }));
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
