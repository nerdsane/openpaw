use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(path: impl AsRef<Path>) -> String {
    fs::read_to_string(path.as_ref())
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.as_ref().display()))
}

#[test]
fn session_link_is_a_reusable_temperpaw_child_session_monitor() {
    let root = repo_root();
    let spec_path = root.join("os-apps/paw-agent/specs/session_link.ioa.toml");
    let spec = read(&spec_path);
    let wiki_builder = read(root.join("os-apps/paw-wiki/wasm/build_session_message/src/lib.rs"));

    for needle in [
        "name = \"SessionLink\"",
        "name = \"Configure\"",
        "name = \"CheckChild\"",
        "name = \"ChildPending\"",
        "name = \"ParentNotified\"",
        "name = \"NotifyFailed\"",
        "module = \"session_link_monitor\"",
        "max_checks",
        "[[state_timeout]]",
        "state = \"Created\"",
        "state = \"Watching\"",
        "from = [\"Created\", \"Watching\"]",
    ] {
        assert!(
            spec.contains(needle),
            "SessionLink spec should contain {needle}"
        );
    }

    assert!(
        !spec.contains("allow_indefinite_states = [\"Watching\"]"),
        "SessionLink.Watching must be bounded by max_checks, not indefinite"
    );

    assert!(
        wiki_builder.contains("/tdata/SessionLinks")
            && wiki_builder.contains("ParentEntitySet")
            && wiki_builder.contains("ChildSessionId")
            && wiki_builder.contains("OnFailureAction"),
        "WikiJob should use the reusable SessionLink monitor instead of bespoke child-session polling"
    );

    assert!(
        wiki_builder.contains("dispatch_wiki_job_failure")
            && wiki_builder.contains("SessionLink setup failed"),
        "WikiJob should fail visibly if child-session monitoring cannot be established"
    );
}

#[test]
fn runtime_model_provider_selection_has_no_hardcoded_llm_fallbacks() {
    let root = repo_root();
    let checked_files = [
        "os-apps/paw-agent/specs/session.ioa.toml",
        "os-apps/paw-agent/specs/agent.ioa.toml",
        "os-apps/paw-agent/specs/cron_job.ioa.toml",
        "os-apps/paw-wiki/specs/wiki_job.ioa.toml",
        "os-apps/paw-managed-agents/specs/managed_agent.ioa.toml",
        "os-apps/paw-agent/wasm/llm_caller/src/lib.rs",
        "os-apps/paw-agent/wasm/context_compactor/src/lib.rs",
        "os-apps/paw-agent/wasm/monty_repl/src/entity_ops.rs",
        "os-apps/paw-wiki/wasm/build_session_message/src/lib.rs",
        "os-apps/paw-channels/wasm/route_message/src/lib.rs",
        "os-apps/paw-managed-agents/wasm/common.rs",
        "os-apps/paw-managed-agents/wasm/session_orchestrator/src/lib.rs",
    ];
    let prohibited = [
        "initial = \"claude-sonnet",
        "initial = \"anthropic\"",
        "unwrap_or(\"claude-sonnet",
        "unwrap_or(\"anthropic\")",
        "unwrap_or_else(|| \"claude-sonnet",
        "unwrap_or_else(|| \"anthropic\"",
        "\"provider\": \"anthropic\"",
        "falling back to {alt}",
        "let alternatives = [\"anthropic\"",
        "infer_provider(",
    ];

    for relative in checked_files {
        let contents = read(root.join(relative));
        for needle in prohibited {
            assert!(
                !contents.contains(needle),
                "{relative} should not contain hardcoded runtime model/provider fallback `{needle}`"
            );
        }
    }
}
