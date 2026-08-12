//! `emit_ots_trajectory` — WASM integration that POSTs an OTS trajectory to
//! Temper's `/api/ots/trajectories` endpoint whenever a `paw-agent` Session
//! enters a terminal state (`Completed`, `Failed`, or `Cancelled`).
//!
//! See `docs/adrs/0035-ots-trajectory-emission.md` for the architectural
//! rationale and field mapping.

use serde_json::json;
use temper_wasm_sdk::prelude::*;
use wasm_helpers::{
    TranscriptPresence, entity_field_str, read_session_transcript, resolve_temper_api_url,
    runtime_headers,
};

mod ots_build;

use ots_build::TrajectoryInputs;

#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        let fields = ctx
            .entity_state
            .get("fields")
            .cloned()
            .unwrap_or_else(|| json!({}));
        let temper_api_url = resolve_temper_api_url(&ctx, &fields);
        let tenant = ctx.tenant.clone();
        let session_id = ctx.entity_id.clone();
        let status = entity_field_str(&ctx.entity_state, &["Status", "status"])
            .unwrap_or("")
            .to_string();

        if !matches!(status.as_str(), "Completed" | "Failed" | "Cancelled") {
            return Err(format!(
                "emit_ots_trajectory: session {session_id} not in terminal state (status={status})"
            ));
        }

        let agent_id = fields
            .get("agent_id")
            .and_then(|v| v.as_str())
            .unwrap_or(session_id.as_str())
            .to_string();

        // Stable trajectory_id across retries — enables INSERT OR REPLACE idempotency.
        let existing_trajectory_id = fields
            .get("trajectory_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let trajectory_id = if existing_trajectory_id.is_empty() {
            format!("trj-{session_id}")
        } else {
            existing_trajectory_id.to_string()
        };

        let tool_spans_file_id = fields
            .get("tool_spans_file_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        // A declared span file that 404s is missing evidence, not an absence of
        // tool calls, and the trajectory has to say so.
        let tool_spans_read =
            read_temperfs_file_safe(&ctx, &temper_api_url, &tenant, tool_spans_file_id)?;
        let tool_spans_missing = !tool_spans_file_id.is_empty() && tool_spans_read.is_none();
        let tool_spans_jsonl = tool_spans_read.unwrap_or_default();

        // The transcript is the source of real turn boundaries. A read failure
        // is not a reason to store a spans-only row: the trajectory would be
        // permanently incomplete and, being marked emitted, never repaired. It
        // is recorded as a failed emission instead, which leaves the row absent
        // and the retry path (`RetryTrajectoryEmission`, plus the Evolution
        // Engine sweep) able to produce a complete one. An absent transcript is
        // a different thing from an unreadable one and still emits — a
        // first-turn session has no materialized entries yet — but the document
        // is spans-only, and it says so rather than passing as complete.
        let session_file_id = fields
            .get("session_file_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let (session_jsonl, transcript) = if session_file_id.is_empty() {
            (String::new(), TranscriptPresence::Undeclared)
        } else {
            match read_session_transcript(&ctx, &temper_api_url, &tenant, &fields, session_file_id)
            {
                Ok(read) => (read.jsonl, read.presence),
                Err(error) => {
                    let msg = format!(
                        "session transcript read failed for {session_id}; no trajectory emitted so a retry can produce a complete one: {error}"
                    );
                    ctx.log("warn", &format!("emit_ots_trajectory: {msg}"));
                    set_success_result(
                        "TrajectoryEmissionFailed",
                        &json!({
                            "trajectory_emission_error": msg,
                            "trajectory_emission_status": "failed",
                        }),
                    );
                    return Ok(());
                }
            }
        };

        let spec_version = resolve_spec_version(&ctx);

        let trajectory = ots_build::build_trajectory(&TrajectoryInputs {
            trajectory_id: &trajectory_id,
            session_id: &session_id,
            agent_id: &agent_id,
            status: &status,
            fields: &fields,
            session_jsonl: &session_jsonl,
            tool_spans_jsonl: &tool_spans_jsonl,
            entity_state: &ctx.entity_state,
            spec_version: &spec_version,
            transcript,
            tool_spans_missing,
        });

        // Degradations are decided from the same inputs the document was built
        // from, so the entity and the row cannot disagree about them.
        let degradations = ots_build::degradations(&trajectory);
        if !degradations.is_empty() {
            ctx.log(
                "warn",
                &format!(
                    "emit_ots_trajectory: session {session_id} produced a degraded trajectory ({})",
                    degradations.join(", ")
                ),
            );
        }

        let body = trajectory.to_string();
        let url = format!("{temper_api_url}/api/ots/trajectories");
        let mut headers = runtime_headers(
            &ctx,
            &tenant,
            &fields,
            Some("application/json"),
            Some("application/json"),
        );
        headers.push(("X-Agent-Id".to_string(), agent_id.clone()));
        headers.push(("X-Session-Id".to_string(), session_id.clone()));
        headers.push(("X-Tenant-Id".to_string(), tenant.clone()));
        headers.push(("X-Trajectory-Id".to_string(), trajectory_id.clone()));

        let resp = ctx.http_call("POST", &url, &headers, &body)?;
        if !(200..300).contains(&resp.status) {
            let msg = format!(
                "POST /api/ots/trajectories failed (HTTP {}): {}",
                resp.status,
                truncate_body(&resp.body)
            );
            ctx.log(
                "warn",
                &format!("emit_ots_trajectory: {msg}"),
            );
            set_success_result(
                "TrajectoryEmissionFailed",
                &json!({
                    "trajectory_emission_error": msg,
                    "trajectory_emission_status": "failed",
                }),
            );
            return Ok(());
        }

        ctx.log(
            "info",
            &format!(
                "emit_ots_trajectory: emitted trajectory {trajectory_id} for session {session_id} (status={status})"
            ),
        );

        // A degraded row is still a row — retrying cannot restore a transcript
        // that is not there — so it is marked emitted, with what it is missing
        // recorded on the entity as well as inside the document.
        let (emission_status, emission_error) = if degradations.is_empty() {
            ("emitted", String::new())
        } else {
            ("emitted_degraded", degradations.join(","))
        };

        set_success_result(
            "MarkTrajectoryEmitted",
            &json!({
                "trajectory_id": trajectory_id,
                "trajectory_emission_status": emission_status,
                "trajectory_emission_error": emission_error,
            }),
        );
        Ok(())
    })();

    if let Err(error) = result {
        // Top-level errors still go through set_error_result so the platform's
        // on_failure hook fires. These are panics / preflight failures, not
        // remote HTTP errors (those route through set_success_result above).
        set_error_result(&error);
    }
    0
}

/// Identity of the actor spec this run executed under.
///
/// The WASM guest context carries no spec hash (`temper-wasm-sdk::Context`
/// exposes config, trigger params, entity state and ids only — see ADR-0035
/// decision section 9), so the governing identity is declared in the spec's own
/// trigger config as `<app>@<version>` and travels with the spec that declares
/// it. A repo contract test keeps that literal pinned to `app.toml`.
fn resolve_spec_version(ctx: &Context) -> String {
    ctx.config
        .get("spec_version")
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_default()
}

/// Read a TemperFS file, reporting a missing one as `None` rather than as an
/// empty body — the caller has to be able to tell "no tool calls" from "the
/// record of the tool calls is gone".
fn read_temperfs_file_safe(
    ctx: &Context,
    temper_api_url: &str,
    tenant: &str,
    file_id: &str,
) -> Result<Option<String>, String> {
    if file_id.is_empty() {
        return Ok(Some(String::new()));
    }
    let fields = ctx
        .entity_state
        .get("fields")
        .cloned()
        .unwrap_or_else(|| json!({}));
    let url = format!("{temper_api_url}/tdata/Files('{file_id}')/$value");
    let headers = runtime_headers(
        ctx,
        tenant,
        &fields,
        Some("application/json"),
        Some("application/octet-stream"),
    );
    let resp = ctx.http_call("GET", &url, &headers, "")?;
    match resp.status {
        200 => Ok(Some(resp.body)),
        404 => Ok(None),
        other => Err(format!(
            "emit_ots_trajectory: TemperFS read failed (HTTP {other})"
        )),
    }
}

fn truncate_body(body: &str) -> String {
    const LIMIT: usize = 240;
    if body.len() <= LIMIT {
        body.to_string()
    } else {
        format!("{}...", &body[..LIMIT])
    }
}
