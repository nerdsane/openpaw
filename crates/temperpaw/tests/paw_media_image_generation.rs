use std::{fs, path::PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("crate should live under repo/crates/temperpaw")
        .to_path_buf()
}

fn read(path: impl Into<PathBuf>) -> String {
    let path = path.into();
    fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()))
}

#[test]
fn paw_media_declares_codex_subscription_image_generation_app() {
    let root = repo_root();
    let app = read(root.join("os-apps/paw-media/app.toml"));
    let spec = read(root.join("os-apps/paw-media/specs/media_generation.ioa.toml"));
    let model = read(root.join("os-apps/paw-media/specs/model.csdl.xml"));
    let policy = read(root.join("os-apps/paw-media/policies/media_generation.cedar"));
    let wasm = read(root.join("os-apps/paw-media/wasm/openai_codex_image_generate/src/lib.rs"));

    for needle in [
        "name = \"paw-media\"",
        "startup_install = \"core\"",
        "dependencies = [\"paw-agent\", \"paw-fs\"]",
        "name = \"openai_codex_image_generate\"",
    ] {
        assert!(
            app.contains(needle),
            "paw-media app.toml should contain {needle}"
        );
    }

    for needle in [
        "name = \"MediaGeneration\"",
        "states = [\"Created\", \"Authorizing\", \"Generating\", \"Storing\", \"Complete\", \"Failed\"]",
        "media_type",
        "provider",
        "Generate",
        "RecordAuthReady",
        "RecordResult",
        "RecordError",
        "module = \"openai_codex_image_generate\"",
        "module = \"provider_auth_gate\"",
        "auth_action = \"EnsureFresh\"",
    ] {
        assert!(
            spec.contains(needle),
            "MediaGeneration spec should contain {needle}"
        );
    }

    for needle in [
        "<EntityType Name=\"MediaGeneration\">",
        "<EntitySet Name=\"MediaGenerations\"",
        "<Action Name=\"Generate\"",
        "<Action Name=\"RecordResult\"",
        "<Property Name=\"ResultFileId\"",
        "<Property Name=\"ProviderResponseId\"",
    ] {
        assert!(
            model.contains(needle),
            "paw-media CSDL should contain {needle}"
        );
    }

    for needle in [
        "resource is MediaGeneration",
        "Action::\"Generate\"",
        "Action::\"RecordResult\"",
        "Action::\"RecordError\"",
        "context.module == \"openai_codex_image_generate\"",
        "Action::\"http_call\"",
        "Action::\"access_secret\"",
    ] {
        assert!(
            policy.contains(needle),
            "paw-media Cedar should contain {needle}"
        );
    }

    for needle in [
        "chatgpt.com/backend-api/codex/responses",
        "\"type\": \"image_generation\"",
        "\"action\": \"generate\"",
        "image_generation_call",
        "RecordResult",
        "Files",
    ] {
        assert!(
            wasm.contains(needle),
            "Codex image WASM should contain {needle}"
        );
    }
}

#[test]
fn image_generation_tool_is_exposed_through_default_agent_tools() {
    let root = repo_root();
    let tool_catalog = read(root.join("os-apps/paw-agent/wasm/tool-catalog/src/lib.rs"));
    let dispatch = read(root.join("os-apps/paw-agent/wasm/monty_repl/src/dispatch.rs"));
    let paw_agent_app = read(root.join("os-apps/paw-agent/app.toml"));
    let startup = read(root.join("crates/temperpaw/src/startup.rs"));
    let setup_api = read(root.join("crates/temperpaw/src/setup_api.rs"));
    let session_spec = read(root.join("os-apps/paw-agent/specs/session.ioa.toml"));
    let cron_spec = read(root.join("os-apps/paw-agent/specs/cron_job.ioa.toml"));
    let route_message = read(root.join("os-apps/paw-channels/wasm/route_message/src/lib.rs"));

    for (label, source) in [
        ("tool catalog", tool_catalog.as_str()),
        ("startup defaults", startup.as_str()),
        ("setup API defaults", setup_api.as_str()),
        ("Session tools default", session_spec.as_str()),
        ("CronJob tools default", cron_spec.as_str()),
        ("channel route defaults", route_message.as_str()),
    ] {
        assert!(
            source.contains("temper_image_generate"),
            "{label} should include temper_image_generate in default tools"
        );
    }

    for needle in [
        "method: \"image_generate\"",
        "token: Some(\"temper_image_generate\")",
        "generate an image through the paw-media app",
    ] {
        assert!(
            tool_catalog.contains(needle),
            "tool catalog should expose {needle}"
        );
    }

    for needle in [
        "\"image_generate\" => Some(\"temper_image_generate\")",
        "\"image_generate\" => temper_image_generate",
        "/tdata/MediaGenerations",
        "Temper.Generate?await_integration=true",
        "__temperpaw_image",
        "unwrap_or_else(|| \"low\".to_string())",
    ] {
        assert!(
            dispatch.contains(needle),
            "Monty dispatch should contain {needle}"
        );
    }

    assert!(
        paw_agent_app.contains("name = \"openai_codex_auth\""),
        "paw-agent app.toml must declare openai_codex_auth so provider_auth_gate can run Codex subscription auth"
    );
}

#[test]
fn paw_media_wasm_is_built_into_ci_and_production_images() {
    let root = repo_root();
    let dockerfile = read(root.join("Dockerfile"));
    let ci = read(root.join(".github/workflows/ci.yml"));
    let identity_contract =
        read(root.join("crates/temperpaw/tests/temperpaw_identity_contract.rs"));
    let build_script = read(root.join("os-apps/paw-media/wasm/build.sh"));

    assert!(
        dockerfile.contains("cd /app/os-apps/paw-media/wasm && bash build.sh"),
        "Dockerfile must build paw-media WASM before copying os-apps into the runtime image"
    );
    assert!(
        ci.contains("os-apps/paw-media/wasm/build.sh"),
        "CI must build paw-media WASM so missing renderer packaging fails before deploy"
    );
    assert!(
        identity_contract.contains("\"os-apps/paw-media/wasm/build.sh\""),
        "identity contract should keep paw-media in the audited WASM build-script set"
    );
    assert!(
        build_script.contains("openai_codex_image_generate.wasm"),
        "paw-media build.sh must publish openai_codex_image_generate.wasm outside target/"
    );
}

#[test]
fn paw_media_policy_limits_result_callbacks_to_runtime_modules() {
    let root = repo_root();
    let policy = read(root.join("os-apps/paw-media/policies/media_generation.cedar"));

    let user_actions = r#"action in [
    Action::"create",
    Action::"read",
    Action::"list",
    Action::"Generate"
  ]"#;
    assert!(
        policy.contains(user_actions),
        "user-facing MediaGeneration policy should only expose create/read/list/Generate"
    );
    for forbidden in [
        "Action::\"RecordAuthReady\"",
        "Action::\"RecordStoring\"",
        "Action::\"RecordResult\"",
        "Action::\"RecordError\"",
    ] {
        assert!(
            !policy
                .split("resource is MediaGeneration")
                .next()
                .unwrap_or_default()
                .contains(forbidden),
            "callback action {forbidden} must not be in the broad user-facing permit"
        );
    }
    for needle in [
        "context.module == \"provider_auth_gate\"",
        "context.module == \"openai_codex_image_generate\"",
        "Action::\"RecordAuthReady\"",
        "Action::\"RecordStoring\"",
        "Action::\"RecordResult\"",
        "Action::\"RecordError\"",
    ] {
        assert!(
            policy.contains(needle),
            "paw-media callback policy should contain {needle}"
        );
    }
}

#[test]
fn image_generation_tool_rejects_empty_success_results() {
    let root = repo_root();
    let dispatch = read(root.join("os-apps/paw-agent/wasm/monty_repl/src/dispatch.rs"));

    for needle in [
        "image_generate: generation completed without an image artifact",
        "file_id.is_empty()",
        "base64_data.is_empty()",
        "path.is_empty()",
    ] {
        assert!(
            dispatch.contains(needle),
            "Monty image result renderer should contain {needle}"
        );
    }
}

#[test]
fn paw_media_is_a_core_startup_app() {
    let root = repo_root();
    temper_platform::os_apps::set_os_apps_dir(root.join("os-apps"));
    let startup_apps = temper_platform::os_apps::list_startup_os_apps();

    assert!(
        startup_apps.iter().any(|app| app == "paw-media"),
        "paw-media should be installed as a core startup app"
    );
}
