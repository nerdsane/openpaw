use std::fs;
use std::path::Path;

fn repo_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn session_spec_defines_bounded_turn_pipeline() {
    let spec = fs::read_to_string(repo_root().join("os-apps/paw-agent/specs/session.ioa.toml"))
        .expect("session.ioa.toml should exist");

    for needle in [
        "PreparingContext",
        "CallingProvider",
        "ApplyingProviderResponse",
        "name = \"ContextReady\"",
        "name = \"ProviderResponseReady\"",
        "name = \"prepare_context\"",
        "module = \"context_preparer\"",
        "name = \"call_provider\"",
        "module = \"provider_caller\"",
        "name = \"apply_provider_response\"",
        "module = \"provider_response_applier\"",
        "name = \"prepared_context_file_id\"",
        "name = \"provider_response_file_id\"",
    ] {
        assert!(
            spec.contains(needle),
            "session spec should contain {needle}"
        );
    }

    assert!(
        spec.contains("effect = [{ type = \"trigger\", name = \"prepare_context\" }]"),
        "session spec should route turn entry points through prepare_context"
    );
}

#[test]
fn active_context_preparer_owns_delta_batch_read_contract() {
    let root = repo_root();
    let preparer =
        fs::read_to_string(root.join("os-apps/paw-agent/wasm/context_preparer/src/lib.rs"))
            .expect("context_preparer source should exist");
    let spec = fs::read_to_string(root.join("os-apps/paw-agent/specs/session.ioa.toml"))
        .expect("session.ioa.toml should exist");

    for needle in [
        "try_reuse_prepared_context",
        "build_context_refs_since",
        "read_text_file_versions_batch",
        "read_text_files_batch",
        "read_content_file_version_raw",
        "context_preparer: reused prepared context",
    ] {
        assert!(
            preparer.contains(needle),
            "active context_preparer should contain {needle}"
        );
    }

    assert!(
        spec.contains("reset_on = [\"ProgressMade\", \"ResumeContext\"]"),
        "PreparingContext state_timeout should reset on real ProgressMade"
    );
}

#[test]
fn route_message_carries_context_cache_fields_to_continuations() {
    let source =
        fs::read_to_string(repo_root().join("os-apps/paw-channels/wasm/route_message/src/lib.rs"))
            .expect("route_message source should exist");

    for needle in [
        "\"prepared_context_file_id\": str_field(fields, &[\"prepared_context_file_id\", \"PreparedContextFileId\"]).unwrap_or(\"\")",
        "\"system_prompt_hash\": str_field(fields, &[\"system_prompt_hash\", \"SystemPromptHash\"]).unwrap_or(\"\")",
        "\"system_prompt_file_id\": str_field(fields, &[\"system_prompt_file_id\", \"SystemPromptFileId\"]).unwrap_or(\"\")",
    ] {
        assert!(
            source.contains(needle),
            "continuation Configure body should carry {needle}"
        );
    }
}

#[test]
fn session_policy_authorizes_new_pipeline_callbacks_and_modules() {
    let policy = fs::read_to_string(repo_root().join("os-apps/paw-agent/policies/session.cedar"))
        .expect("session.cedar should exist");

    for needle in [
        "Action::\"ContextReady\"",
        "Action::\"ProviderResponseReady\"",
        "\"context_preparer\"",
        "\"provider_caller\"",
        "\"provider_response_applier\"",
    ] {
        assert!(
            policy.contains(needle),
            "session policy should contain {needle}"
        );
    }
}

#[test]
fn dashboard_and_monitors_cover_session_context_metrics() {
    let dashboard = fs::read_to_string(repo_root().join("dd-dashboards/temperpaw-overview.json"))
        .expect("dashboard json should exist");
    let monitors = fs::read_to_string(repo_root().join("dd-monitors/temperpaw-monitors.json"))
        .expect("monitor json should exist");

    for needle in [
        "temper_session_context_tokens",
        "temper_session_context_bytes",
        "temper_session_context_prepare_duration_ms",
        "temper_session_provider_request_bytes",
        "temper_session_provider_response_bytes",
        "temper_session_memory_limit_exceeded_total",
    ] {
        assert!(
            dashboard.contains(needle),
            "dashboard should contain {needle}"
        );
    }

    assert!(
        monitors.contains("temper_session_memory_limit_exceeded_total"),
        "monitors should alert on session memory-limit failures"
    );
}

#[test]
fn session_spec_passes_modal_bridge_url_to_modal_integrations() {
    let spec = fs::read_to_string(repo_root().join("os-apps/paw-agent/specs/session.ioa.toml"))
        .expect("session.ioa.toml should exist");

    let count = spec
        .matches("modal_bridge_url = \"{secret:modal_bridge_url}\"")
        .count();

    assert!(
        count >= 3,
        "session spec should pass modal_bridge_url into sandbox-related integrations"
    );
}

#[test]
fn record_result_clears_pending_tool_state_on_terminal_completion() {
    let root = repo_root();
    let spec = fs::read_to_string(root.join("os-apps/paw-agent/specs/session.ioa.toml"))
        .expect("session.ioa.toml should exist");
    let csdl = fs::read_to_string(root.join("os-apps/paw-agent/specs/model.csdl.xml"))
        .expect("model.csdl.xml should exist");
    let monty = fs::read_to_string(root.join("os-apps/paw-agent/wasm/monty_repl/src/lib.rs"))
        .expect("monty_repl source should exist");

    assert!(
        spec.contains(
            "params = [\"result\", \"conversation\", \"input_tokens\", \"output_tokens\", \"session_leaf_id\", \"repl_file_id\", \"tool_spans_file_id\", \"system_prompt_hash\", \"system_prompt_file_id\", \"pending_tool_calls\", \"pending_tool_context\", \"pending_decision_id\"]"
        ),
        "RecordResult should be able to clear pending tool and approval fields on completion"
    );

    for needle in [
        "<Parameter Name=\"pending_tool_calls\" Type=\"Edm.String\" Nullable=\"true\"/>",
        "<Parameter Name=\"pending_tool_context\" Type=\"Edm.String\" Nullable=\"true\"/>",
        "<Parameter Name=\"pending_decision_id\" Type=\"Edm.String\" Nullable=\"true\"/>",
    ] {
        assert!(
            csdl.contains(needle),
            "RecordResult CSDL should contain {needle}"
        );
    }

    for needle in [
        "done_params[\"pending_tool_calls\"] = json!(\"\")",
        "done_params[\"pending_tool_context\"] = json!(\"\")",
        "done_params[\"pending_decision_id\"] = json!(\"\")",
    ] {
        assert!(
            monty.contains(needle),
            "temper.done completion path should contain {needle}"
        );
    }
}

#[test]
fn finalize_result_clears_pending_tool_state_on_terminal_completion() {
    let root = repo_root();
    let spec = fs::read_to_string(root.join("os-apps/paw-agent/specs/session.ioa.toml"))
        .expect("session.ioa.toml should exist");
    let csdl = fs::read_to_string(root.join("os-apps/paw-agent/specs/model.csdl.xml"))
        .expect("model.csdl.xml should exist");
    let steering_checker =
        fs::read_to_string(root.join("os-apps/paw-agent/wasm/steering_checker/src/lib.rs"))
            .expect("steering_checker source should exist");

    assert!(
        spec.contains(
            "params = [\"result\", \"conversation\", \"session_leaf_id\", \"pending_tool_calls\", \"pending_tool_context\", \"pending_decision_id\"]"
        ),
        "FinalizeResult should be able to clear pending tool and approval fields on completion"
    );

    for needle in [
        "<Action Name=\"FinalizeResult\" IsBound=\"true\">",
        "<Parameter Name=\"pending_tool_calls\" Type=\"Edm.String\" Nullable=\"true\"/>",
        "<Parameter Name=\"pending_tool_context\" Type=\"Edm.String\" Nullable=\"true\"/>",
        "<Parameter Name=\"pending_decision_id\" Type=\"Edm.String\" Nullable=\"true\"/>",
    ] {
        assert!(
            csdl.contains(needle),
            "FinalizeResult CSDL should contain {needle}"
        );
    }

    for needle in [
        "\"pending_tool_calls\": \"\"",
        "\"pending_tool_context\": \"\"",
        "\"pending_decision_id\": \"\"",
    ] {
        assert!(
            steering_checker.contains(needle),
            "steering_checker finalize path should contain {needle}"
        );
    }
}
