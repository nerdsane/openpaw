//! computer_copy_poll — discover, then poll, an async live-copy (ARN-443 C; C5
//! structural: discovery moved here from computer_copy_start).
//!
//! Fires on Copy.CopyPoll (driven by the Copying state_timeout). Two phases, keyed
//! off the row's `owned_machine` provenance flag:
//!
//! DISCOVERY (owned_machine == false — machine_id is still the SOURCE's):
//!   - find the created "<source>-copy" sandbox by name, excluding any machine
//!     already claimed by a live Computer row (fail-CLOSED: if that claim read
//!     fails we do NOT discover this tick — we never adopt a sandbox we could not
//!     check) → CopyDiscovered records the copy's OWN machine + marks owned_machine;
//!   - not listed yet, in budget → empty callback (the Copying timeout re-arms);
//!   - past the deadline → set_failure → on_failure = CopyExpired.
//!
//! READINESS (owned_machine == true — machine_id is the copy's own):
//!   - ready               → CopyComplete (Copying → Leased);
//!   - not ready, in budget → empty callback (re-arm);
//!   - past the deadline    → set_failure → CopyExpired (terminate the leaked copy).
//!     A transient error before the deadline is treated as "not ready yet".
//!
//! Build: `cargo build --target wasm32-wasip1 --release`.

use temper_wasm_sdk::prelude::*;
use wasm_helpers::sandbox::{self, SandboxHandle, normalize_sandbox_provider};
use wasm_helpers::{bounded_reads, entity_field_str, odata_headers, resolve_temper_api_url};

/// Max /tdata pages to walk when collecting claimed machine_ids — a hard bound so a
/// pathological nextLink chain can never spin the invocation.
const MAX_CLAIMED_PAGES: usize = 25;

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

    let now = Context::get_time_millis();
    let deadline_at = entity_field_str(&fields, &["copy_deadline_at_ms", "CopyDeadlineAtMs"])
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(0);
    let past_deadline = deadline_at != 0 && now > deadline_at;

    let provider = provider_from_fields(&fields);

    if bool_field(&fields, &["owned_machine", "OwnedMachine"]) {
        poll_readiness(&ctx, &fields, &provider, past_deadline);
    } else {
        discover(&ctx, &fields, &provider, past_deadline);
    }
    0
}

/// DISCOVERY phase: find the copy sandbox and record it (CopyDiscovered).
fn discover(ctx: &Context, fields: &Value, provider: &str, past_deadline: bool) {
    // Before discovery, machine_id is the SOURCE's machine — the copy source.
    let source_machine_id = match entity_field_str(fields, &["machine_id", "MachineId"])
        .filter(|s| !s.is_empty())
    {
        Some(s) => s.to_string(),
        None => {
            set_error_result("computer_copy_poll: no source machine_id to discover a copy of");
            return;
        }
    };

    // FAIL-CLOSED: never adopt a sandbox without the claimed-id check. If we cannot
    // read the live Computer rows, we do NOT discover this tick — re-arm (or expire
    // past the deadline), rather than risk adopting a machine owned by another row.
    let claimed_ids = match live_claimed_machine_ids(ctx, fields) {
        Ok(v) => v,
        Err(e) => {
            if past_deadline {
                set_error_result(&format!("copy claim-check unavailable at deadline: {e}"));
            } else {
                ctx.log("info", &format!("computer_copy_poll: claim read failed, retrying: {e}"));
                set_success_result("", &json!({}));
            }
            return;
        }
    };

    match sandbox::sandbox_copy_discover(ctx, provider, &source_machine_id, &claimed_ids) {
        Ok(Some(handle)) => {
            ctx.log(
                "info",
                &format!("computer_copy_poll: discovered copy {}", handle.sandbox_id),
            );
            set_success_result(
                "CopyDiscovered",
                &json!({
                    "machine_id": handle.sandbox_id,
                    "sandbox_url": handle.sandbox_url,
                    "source_machine_id": source_machine_id,
                    "name": copy_name(&ctx.entity_id),
                }),
            );
        }
        Ok(None) => {
            if past_deadline {
                set_error_result("copy sandbox never appeared before its deadline");
            } else {
                set_success_result("", &json!({})); // not listed yet — re-arm
            }
        }
        Err(e) => {
            if past_deadline {
                set_error_result(&format!("copy discovery failed at deadline: {e}"));
            } else {
                set_success_result("", &json!({})); // transient list error — re-arm
            }
        }
    }
}

/// READINESS phase: the copy's own machine is recorded; health-check it.
fn poll_readiness(ctx: &Context, fields: &Value, _provider: &str, past_deadline: bool) {
    let handle = match handle_from_fields(fields) {
        Ok(h) => h,
        Err(e) => {
            set_error_result(&format!("computer_copy_poll: {e}"));
            return;
        }
    };
    match sandbox::sandbox_health_check(ctx, &handle) {
        Ok(true) => {
            ctx.log("info", &format!("computer_copy_poll: copy {} is ready", handle.sandbox_id));
            set_success_result("CopyComplete", &json!({}));
        }
        Ok(false) => {
            if past_deadline {
                set_error_result("copy never became ready before its deadline");
            } else {
                set_success_result("", &json!({})); // still booting — re-arm
            }
        }
        Err(e) => {
            if past_deadline {
                set_error_result(&format!("copy readiness unknown at deadline: {e}"));
            } else {
                set_success_result("", &json!({}));
            }
        }
    }
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

/// machine_ids referenced by LIVE Computer rows (any non-terminal state), read via
/// the Temper loopback, following @odata.nextLink so a large fleet is not truncated.
/// Returns Err on ANY read/parse failure so the caller can fail CLOSED (never adopt
/// a sandbox it could not check against the live rows).
fn live_claimed_machine_ids(ctx: &Context, fields: &Value) -> Result<Vec<String>, String> {
    let api = resolve_temper_api_url(ctx, fields);
    let headers = odata_headers(ctx, &ctx.tenant, fields);
    let mut path = "/tdata/Computers?$select=machine_id,status&$top=200".to_string();
    let mut out = Vec::new();
    for _ in 0..MAX_CLAIMED_PAGES {
        let body = bounded_reads::get_json(ctx, &api, &path, &headers, "computer_copy_poll.claimed")?;
        if let Some(arr) = body.get("value").and_then(|v| v.as_array()) {
            for row in arr {
                let f = row.get("fields").unwrap_or(row);
                let status = entity_field_str(f, &["status", "Status"]).unwrap_or("");
                if matches!(status, "Destroyed" | "Created" | "") {
                    continue; // not holding a live sandbox
                }
                if let Some(mid) =
                    entity_field_str(f, &["machine_id", "MachineId"]).filter(|m| !m.is_empty())
                {
                    out.push(mid.to_string());
                }
            }
        }
        match body.get("@odata.nextLink").and_then(|v| v.as_str()) {
            Some(next) if !next.is_empty() => path = next_link_path(next),
            _ => return Ok(out),
        }
    }
    Ok(out)
}

/// Reduce an @odata.nextLink (absolute URL or relative path) to the path+query the
/// loopback GET expects (get_json prepends the api base).
fn next_link_path(next: &str) -> String {
    if let Some(idx) = next.find("/tdata/") {
        next[idx..].to_string()
    } else if next.starts_with('/') {
        next.to_string()
    } else {
        format!("/{next}")
    }
}

/// Build the copy's sandbox handle from the row (machine_id + sandbox_url are the
/// COPY's, set at CopyDiscovered).
fn handle_from_fields(fields: &Value) -> Result<SandboxHandle, String> {
    let sandbox_url = entity_field_str(fields, &["sandbox_url", "SandboxUrl"])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or("no sandbox_url on the copy")?
        .to_string();
    let sandbox_id = entity_field_str(fields, &["machine_id", "MachineId"])
        .filter(|s| !s.is_empty())
        .ok_or("no machine_id on the copy")?
        .to_string();
    let provider = entity_field_str(fields, &["provider", "Provider"])
        .filter(|s| !s.is_empty())
        .map(normalize_sandbox_provider)
        .unwrap_or_else(|| "tensorlake".to_string());
    Ok(SandboxHandle {
        sandbox_url,
        sandbox_id,
        provider,
    })
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
    fn handle_needs_url_and_id() {
        assert!(handle_from_fields(&json!({"sandbox_url": "https://x", "machine_id": "m"})).is_ok());
        assert!(handle_from_fields(&json!({"machine_id": "m"})).is_err());
        assert!(handle_from_fields(&json!({"sandbox_url": "https://x"})).is_err());
    }

    #[test]
    fn copy_name_is_distinct_and_never_empty() {
        assert_eq!(copy_name("abc123def456ghi"), "copy-abc123def456");
        assert_eq!(copy_name(""), "copy");
        assert_ne!(copy_name("arni-big"), "arni-big");
    }

    #[test]
    fn bool_field_reads_json_bool_and_string() {
        assert!(bool_field(&json!({"owned_machine": true}), &["owned_machine"]));
        assert!(bool_field(&json!({"owned_machine": "true"}), &["owned_machine"]));
        assert!(!bool_field(&json!({"owned_machine": "false"}), &["owned_machine"]));
        assert!(!bool_field(&json!({}), &["owned_machine"]));
    }

    #[test]
    fn next_link_path_normalizes_absolute_and_relative() {
        assert_eq!(
            next_link_path("https://host/tdata/Computers?$skiptoken=abc"),
            "/tdata/Computers?$skiptoken=abc"
        );
        assert_eq!(next_link_path("/tdata/Computers?x=1"), "/tdata/Computers?x=1");
        assert_eq!(next_link_path("tdata/Computers"), "/tdata/Computers");
    }
}
