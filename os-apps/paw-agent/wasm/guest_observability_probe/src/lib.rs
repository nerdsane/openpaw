//! Proof-only WASM module for the guest observability host API.
//!
//! The module is dispatched by the live proof IOA spec. It exercises the
//! structured guest span lifecycle and then hands the same entity to the
//! migrated `monty_repl` integration so the proof covers both the direct API
//! and a key TemperPaw module path.

use temper_wasm_sdk::prelude::*;

#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;
        let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));
        let run_id = ctx
            .trigger_params
            .get("run_id")
            .and_then(Value::as_str)
            .or_else(|| fields.get("run_id").and_then(Value::as_str))
            .unwrap_or("wasmobs-proof-unknown");
        let temper_api_url = ctx
            .config
            .get("temper_api_url")
            .cloned()
            .or_else(|| {
                fields
                    .get("temper_api_url")
                    .and_then(Value::as_str)
                    .map(ToString::to_string)
            })
            .unwrap_or_else(|| "http://127.0.0.1:3467".to_string());

        let root = ctx.start_span(
            "proof.guest_observability",
            &json!({
                "run_id": run_id,
                "proof.name": "wasm_guest_observability_host_api",
                "component": "guest_observability_probe",
                "temperpaw.migrated_path.next": "monty_repl",
            }),
        )?;
        root.add_event(
            "proof.started",
            &json!({
                "run_id": run_id,
                "module": ctx.wasm_module,
                "entity_type": ctx.entity_type,
                "trigger_action": ctx.trigger_action,
            }),
        )?;

        ctx.log_structured(
            "info",
            "wasm guest observability proof structured log",
            &json!({
                "run_id": run_id,
                "proof_phase": "direct_guest_api",
                "component": "guest_observability_probe",
            }),
        )?;
        ctx.emit_progress(&json!({
            "kind": "wasm_guest_observability_proof",
            "run_id": run_id,
            "phase": "direct_guest_api",
            "progress": 0.5,
        }))?;
        ctx.emit_metric(
            "temperpaw.wasm_guest_observability.proof",
            1.0,
            &json!({
                "component": "wasm_guest_observability",
                "proof_phase": "direct_guest_api",
            }),
            Some("counter"),
        )?;

        let nested = ctx.start_span(
            "proof.nested_guest_span",
            &json!({
                "run_id": run_id,
                "proof.phase": "nested",
            }),
        )?;
        nested.add_event(
            "proof.nested_event",
            &json!({
                "run_id": run_id,
                "event.detail": "nested span event from WASM",
            }),
        )?;

        let health_url = format!("{temper_api_url}/healthz");
        match ctx.http_call("GET", &health_url, &[], "") {
            Ok(resp) => {
                nested.set_attributes(&json!({
                    "http.healthz.status_code": resp.status,
                    "http.healthz.ok": resp.status < 400,
                }))?;
            }
            Err(error) => {
                nested.set_attributes(&json!({
                    "http.healthz.error": error,
                }))?;
            }
        }
        nested.end_ok(&json!({
            "proof.nested.completed": true,
        }))?;

        root.set_attributes(&json!({
            "proof.direct_api.completed": true,
            "proof.next_action": "RunMigratedToolPath",
        }))?;
        root.end_ok(&json!({
            "proof.completed": true,
        }))?;

        let tool_code = format!(
            "print('wasm guest observability migrated Monty path run_id={run_id}')\n\
             temper.specs()"
        );
        let pending_tool_calls = serde_json::to_string(&json!([
            {
                "type": "tool_use",
                "id": format!("toolu_{run_id}"),
                "name": "python",
                "input": {
                    "code": tool_code,
                }
            }
        ]))
        .map_err(|e| format!("failed to build proof tool call: {e}"))?;

        set_success_result(
            "RunMigratedToolPath",
            &json!({
                "run_id": run_id,
                "pending_tool_calls": pending_tool_calls,
                "conversation": "[]",
                "temper_api_url": temper_api_url,
                "workdir": "/workspace",
                "tools_enabled": "temper_specs",
                "normal_repl_state_max_bytes": "0",
                "persist_tool_spans_file": "false",
            }),
        );
        Ok(())
    })();

    if let Err(error) = result {
        set_error_result(&error);
        return 1;
    }
    0
}
