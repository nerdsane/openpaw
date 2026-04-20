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
