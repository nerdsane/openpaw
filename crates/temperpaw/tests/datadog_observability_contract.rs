use serde_json::Value;
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

fn repo_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn load_json(relative_path: &str) -> Value {
    let path = repo_root().join(relative_path);
    serde_json::from_str(
        &std::fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display())),
    )
    .unwrap_or_else(|err| panic!("failed to parse {}: {err}", path.display()))
}

fn dashboard_text() -> String {
    load_json("dd-dashboards/temperpaw-overview.json").to_string()
}

fn load_text(relative_path: &str) -> String {
    let path = repo_root().join(relative_path);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()))
}

fn collect_cargo_manifests(root: &Path, relative_dir: &Path, files: &mut Vec<PathBuf>) {
    let dir = root.join(relative_dir);
    let entries =
        fs::read_dir(&dir).unwrap_or_else(|err| panic!("failed to read {}: {err}", dir.display()));

    for entry in entries {
        let entry = entry.unwrap_or_else(|err| panic!("failed to read dir entry: {err}"));
        let relative_path = relative_dir.join(entry.file_name());
        let file_type = entry
            .file_type()
            .unwrap_or_else(|err| panic!("failed to stat {}: {err}", relative_path.display()));

        if file_type.is_dir() {
            let name = entry.file_name();
            if name != "target" && name != "node_modules" {
                collect_cargo_manifests(root, &relative_path, files);
            }
        } else if file_type.is_file()
            && relative_path
                .file_name()
                .is_some_and(|name| name == "Cargo.toml")
        {
            files.push(relative_path);
        }
    }
}

#[test]
fn temper_dependency_pin_uses_runtime_llmobs_identity_parent_and_hierarchy_fix() {
    let manifest = load_text("crates/temperpaw/Cargo.toml");
    let lockfile = load_text("Cargo.lock");
    let expected_rev = "01288d7bbbbf30f68d5353d3e8982aad5be49ee0";
    let host_boundary_rev = "7b170cf71246e01c337e81062b54ea8c597b9293";
    let parent_only_rev = "4fbfcb971c7c9513ad6605cb8376a8c492c21482";
    let parentless_rev = "ffa0a15212966dbada3db8da6e652f081e5f261b";
    let legacy_rev = "5a19c5f4406e95533896a860b5da15a7a68a70ee";

    for temper_crate in [
        "temper-platform",
        "temper-observe",
        "temper-server",
        "temper-runtime",
        "temper-jit",
        "temper-authz",
        "temper-store-postgres",
        "temper-store-turso",
    ] {
        let manifest_clause = format!(
            "{temper_crate} = {{ git = \"https://github.com/nerdsane/temper.git\", rev = \"{expected_rev}\""
        );
        assert!(
            manifest.contains(&manifest_clause),
            "{temper_crate} must pin the Temper rev with runtime-derived LLMObs service identity, parent stitching, agent/workflow hierarchy, DBM attribution, profiling envelope, Datadog-visible WASM span hints, host-boundary spans, and guest progress/log correlation"
        );
    }

    assert!(
        !manifest.contains(legacy_rev)
            && !lockfile.contains(legacy_rev)
            && !manifest.contains(parent_only_rev)
            && !lockfile.contains(parent_only_rev)
            && !manifest.contains(parentless_rev)
            && !lockfile.contains(parentless_rev)
            && !manifest.contains(host_boundary_rev)
            && !lockfile.contains(host_boundary_rev),
        "TemperPaw must not pin Temper revs without complete WASM host-boundary observability, hard-coded LLMObs identity, parentless direct LLMObs spans, or one-span LLMObs traces"
    );
    assert!(
        lockfile.contains(expected_rev),
        "Cargo.lock must resolve Temper dependencies to the runtime-derived LLMObs service identity, parent-stitching, and agent/workflow hierarchy fix"
    );
}

#[test]
fn wasm_sdk_dependencies_pin_same_temper_observability_revision_as_server() {
    let root = repo_root();
    let expected_rev = "01288d7bbbbf30f68d5353d3e8982aad5be49ee0";
    let expected_dependency = format!(
        "temper-wasm-sdk = {{ git = \"https://github.com/nerdsane/temper.git\", rev = \"{expected_rev}\""
    );
    let forbidden_dependency =
        "temper-wasm-sdk = { git = \"https://github.com/nerdsane/temper.git\", branch = \"main\"";
    let mut manifests = Vec::new();
    collect_cargo_manifests(&root, Path::new("os-apps"), &mut manifests);

    let mut sdk_manifests = 0usize;
    let mut sdk_lockfiles = 0usize;
    for manifest_path in manifests {
        let manifest = fs::read_to_string(root.join(&manifest_path))
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", manifest_path.display()));
        if !manifest.contains("temper-wasm-sdk") {
            continue;
        }
        sdk_manifests += 1;
        assert!(
            manifest.contains(&expected_dependency),
            "{} must pin temper-wasm-sdk to the same Temper rev as the server so guest modules do not drift away from WASM host-boundary observability contracts",
            manifest_path.display()
        );
        assert!(
            !manifest.contains(forbidden_dependency),
            "{} must not build temper-wasm-sdk from moving main; production images need one coherent SDK/runtime observability contract",
            manifest_path.display()
        );

        let lock_path = manifest_path.with_file_name("Cargo.lock");
        let absolute_lock_path = root.join(&lock_path);
        if absolute_lock_path.exists() {
            let lockfile = fs::read_to_string(&absolute_lock_path)
                .unwrap_or_else(|err| panic!("failed to read {}: {err}", lock_path.display()));
            if lockfile.contains("name = \"temper-wasm-sdk\"") {
                sdk_lockfiles += 1;
                assert!(
                    lockfile.contains(expected_rev),
                    "{} must resolve temper-wasm-sdk to the same Temper observability rev as the server",
                    lock_path.display()
                );
                assert!(
                    !lockfile.contains("?branch=main#"),
                    "{} must not keep a moving-main temper-wasm-sdk lock source",
                    lock_path.display()
                );
            }
        }
    }

    assert!(
        sdk_manifests > 20,
        "contract must inspect the packaged WASM module dependency surface"
    );
    assert!(
        sdk_lockfiles > 20,
        "contract must inspect checked-in WASM Cargo.lock resolution"
    );
}

#[test]
fn dockerfile_pins_cloned_katagami_wasm_sdk_to_temper_observability_revision() {
    let dockerfile = load_text("Dockerfile");
    let expected_rev = "01288d7bbbbf30f68d5353d3e8982aad5be49ee0";

    for required in [
        &format!("TEMPER_OBSERVABILITY_REV={expected_rev}"),
        "os-apps/katagami-curation/wasm",
        "temper-wasm-sdk",
        "branch = \\\"main\\\"",
        "rev = \\\"${TEMPER_OBSERVABILITY_REV}\\\"",
        "Cargo.lock",
    ] {
        assert!(
            dockerfile.contains(required),
            "Dockerfile must pin cloned Katagami WASM SDK dependencies with `{required}`"
        );
    }
}

fn monitor_search_text() -> String {
    load_json("dd-monitors/temperpaw-monitors.json")
        .as_array()
        .expect("monitors must be an array")
        .iter()
        .map(|monitor| {
            let tags = monitor["tags"]
                .as_array()
                .map(|tags| {
                    tags.iter()
                        .filter_map(|tag| tag.as_str())
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .unwrap_or_default();
            format!(
                "{}\n{}\n{}\n{}\n{}",
                monitor["name"].as_str().unwrap_or_default(),
                monitor["type"].as_str().unwrap_or_default(),
                monitor["query"].as_str().unwrap_or_default(),
                monitor["message"].as_str().unwrap_or_default(),
                tags
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn agent_operating_guidance_teaches_complete_datadog_diagnostics() {
    let sre_manual = load_text("os-apps/paw-agent/agents/sre/AGENT.md");
    let paw_skill = load_text("os-apps/paw-agent/agents/paw/skills/temperpaw-agent/SKILL.md");
    let combined = format!("{sre_manual}\n{paw_skill}");

    for required in [
        "temperpaw.agent.session",
        "managed_session_id",
        "inner_session_id",
        "dd.trace_id",
        "dd.span_id",
        "LLM Observability",
        "gen_ai.operation.name",
        "Postgres DBM",
        "profiling",
        "Database Monitoring",
        "get_llmobs_agent_loop",
        "wasm_module",
        "workflow_step",
        "wasm_guest.progress",
        "wasm.host.get_secret",
        "non-redundant",
        "chronological",
    ] {
        assert!(
            combined.contains(required),
            "agent operating guidance must teach Datadog diagnostic concept `{required}`"
        );
    }
}

#[test]
fn datadog_facets_include_agent_session_diagnostic_fields() {
    let facets = load_json("dd-pipelines/facets.json");
    let paths: BTreeSet<&str> = facets["facets"]
        .as_array()
        .expect("facets must be an array")
        .iter()
        .filter_map(|facet| facet["path"].as_str())
        .collect();

    for required in [
        "tenant",
        "entity_type",
        "entity_id",
        "action_name",
        "state",
        "from_status",
        "to_status",
        "observability_event",
        "trigger_action",
        "session_id",
        "managed_session_id",
        "inner_session_id",
        "parent_session_id",
        "turn_id",
        "agent_id",
        "inner_agent_id",
        "managed_agent_id",
        "environment_id",
        "tool.name",
        "tool.call_id",
        "wasm_module",
        "workflow_step",
        "progress.kind",
        "gen_ai.operation.name",
        "gen_ai.provider.name",
        "gen_ai.system",
        "gen_ai.conversation.id",
        "gen_ai.request.model",
        "gen_ai.usage.input_tokens",
        "gen_ai.usage.output_tokens",
        "workflow.cycle_id",
        "workflow_root_entity_type",
        "workflow_root_entity_id",
        "workflow_run_id",
        "deployment.id",
        "dd.trace_id",
        "dd.span_id",
        "error.kind",
    ] {
        assert!(
            paths.contains(required),
            "Datadog log facets must make `{required}` searchable for humans and agents"
        );
    }
}

#[test]
fn guide_teaches_wasm_host_boundary_observability() {
    let guide = load_text("docs/temperpaw-datadog-observability-guide.md");
    let success_contract =
        load_text("docs/temperpaw-identity-and-observability-success-contract.md");
    let combined = format!("{guide}\n{success_contract}");

    for required in [
        "WASM Host Boundary Visibility",
        "wasm.host.get_secret",
        "wasm.host.evaluate_spec",
        "wasm.host.connect_call",
        "wasm.host.cache_contains",
        "wasm.host.cache_to_stream",
        "wasm.host.cache_from_stream",
        "wasm.host.read_field",
        "wasm.host.hash_stream",
        "wasm_guest.progress",
        "wasm_module",
        "workflow_step",
        "progress.kind",
        "not inside-WASM APM spans",
        "ADR-0086",
    ] {
        assert!(
            combined.contains(required),
            "observability docs must teach WASM host-boundary diagnostic concept `{required}`"
        );
    }
}

#[test]
fn dashboard_exposes_session_llm_database_logs_and_trace_surfaces() {
    let dashboard = dashboard_text();

    for required in [
        "Agent Session Trace",
        "temperpaw.agent.session",
        "Session ID",
        "managed_session_id",
        "inner_session_id",
        "LLM Observability",
        "gen_ai.system",
        "gen_ai.request.model",
        "gen_ai.usage.input_tokens",
        "Postgres DBM",
        "dbm",
        "Profiling",
        "datadog.profiling.rust.profiles_uploaded",
        "Logs by Trace",
        "dd.trace_id",
    ] {
        assert!(
            dashboard.contains(required),
            "TemperPaw dashboard must expose `{required}`"
        );
    }
}

#[test]
fn monitors_cover_session_trace_llmobs_and_postgres_dbm_health() {
    let monitors = monitor_search_text();
    let monitor_defs = load_json("dd-monitors/temperpaw-monitors.json");

    for required in [
        "[TemperPaw] Agent Session Trace Correlation Missing",
        "[TemperPaw] LLM Error Rate Spike",
        "[TemperPaw] LLM Latency Regression",
        "[TemperPaw] Postgres DBM Query Latency Regression",
        "[TemperPaw] Postgres DBM Activity Missing",
        "[Temper] Profiler Upload Failures",
        "trace-analytics alert",
        "@observability_event:temperpaw.agent.session -trace_id:*",
        "@module_name:provider_caller",
        "datadog.dbm.activity_rows",
        "type:sql",
        "@db.system:postgresql",
        "@peer.service:temperpaw-postgres",
        "gen_ai.system",
        "gen_ai.request.model",
        "postgres",
        "dbm",
    ] {
        assert!(
            monitors.contains(required),
            "Datadog monitors must include `{required}` coverage"
        );
    }

    assert!(
        !monitors.contains(
            "service:temperpaw operation_name:postgresql.query @peer.service:temperpaw-postgres"
        ),
        "Postgres DBM correlation monitor must use live indexed DB span attributes instead of the stale operation_name clause"
    );
    assert!(
        !monitors.contains("[TemperPaw] Postgres DBM Missing APM Correlation"),
        "Datadog trace-analytics absence monitors must not be used for DBM/APM child SQL spans because live Trace Explorer can match them while monitor evaluation reports zero"
    );

    assert!(
        !monitors.contains("trace.temperpaw.agent.session.hits")
            && !monitors.contains("trace.tool.llm_call.duration"),
        "Datadog monitors must use live-validated trace analytics queries, not generated trace metrics that are absent in production"
    );
    assert!(
        !monitors.contains("@entity_type:ManagedSession @action_name:(StartSession OR ResumeSession)\").rollup(\"count\").last(\"30m\") < 1"),
        "Agent session trace monitors must not alert on idle managed-session traffic; alert only when actual agent-session events lack trace correlation"
    );
    assert!(
        !monitors.contains("[Temper] Profiler Uploads Stalled"),
        "Profiler uploads are on-demand in Railway; monitor upload failures and proof uploads, not continuous background upload absence"
    );

    let dbm_activity_monitor = monitor_defs
        .as_array()
        .expect("monitors must be an array")
        .iter()
        .find(|monitor| {
            monitor["name"].as_str() == Some("[TemperPaw] Postgres DBM Activity Missing")
        })
        .expect("Postgres DBM activity monitor must exist");
    assert_eq!(
        dbm_activity_monitor["type"].as_str(),
        Some("metric alert"),
        "Postgres DBM health monitor must use DBM metrics for alerting; APM SQL child-span correlation remains in the runbook/query text for diagnostics"
    );
    assert!(
        dbm_activity_monitor["query"]
            .as_str()
            .is_some_and(|query| query.contains("< 0.1")),
        "Postgres DBM activity rows can be fractional after Datadog rollup; the missing-activity threshold must be below one row so sparse-but-valid DBM samples do not false-alert"
    );
    assert_eq!(
        dbm_activity_monitor["options"]["thresholds"]["critical"].as_f64(),
        Some(0.1),
        "Postgres DBM activity monitor critical threshold must match the fractional missing-activity query"
    );

    let dbm_latency_monitor = monitor_defs
        .as_array()
        .expect("monitors must be an array")
        .iter()
        .find(|monitor| {
            monitor["name"].as_str() == Some("[TemperPaw] Postgres DBM Query Latency Regression")
        })
        .expect("Postgres DBM query latency monitor must exist");
    assert!(
        dbm_latency_monitor["query"]
            .as_str()
            .is_some_and(|query| query.contains("> 30000000")),
        "Postgres DBM query latency metric is reported in nanoseconds, so the critical threshold must be 30ms/30,000,000ns rather than `> 1`"
    );

    let state_timeout_reset_monitor = monitor_defs
        .as_array()
        .expect("monitors must be an array")
        .iter()
        .find(|monitor| monitor["name"].as_str() == Some("[Temper] State Timeout Reset Rate Drop"))
        .expect("state timeout reset monitor must exist");
    assert!(
        state_timeout_reset_monitor["query"]
            .as_str()
            .is_some_and(|query| !query.contains("default_zero")),
        "State timeout reset-drop monitor must not convert idle/no-data periods into alerts"
    );
    assert_eq!(
        state_timeout_reset_monitor["options"]["on_missing_data"].as_str(),
        Some("resolve"),
        "State timeout reset-drop monitor must resolve no-data periods unless active Executing workload gating exists"
    );
}

#[test]
fn datadog_covers_temperfs_blob_and_document_services() {
    let dashboard = dashboard_text();
    let monitors = monitor_search_text();
    let pipeline = load_json("dd-pipelines/temper-temperpaw.json").to_string();
    let guide = load_text("docs/temperpaw-datadog-observability-guide.md");
    let blob_adapter = load_text("os-apps/paw-fs/wasm/blob_adapter/src/lib.rs");
    let workspace_fs = load_text("os-apps/paw-fs/wasm/workspace_fs/src/lib.rs");
    let facets = load_json("dd-pipelines/facets.json");
    let paths: BTreeSet<&str> = facets["facets"]
        .as_array()
        .expect("facets must be an array")
        .iter()
        .filter_map(|facet| facet["path"].as_str())
        .collect();

    for required in [
        "TemperFS Blob & Document Services",
        "temper_blob_io_wait_duration_ms",
        "temper_blob_local_fast_path_requests_total",
        "temper_blob_transport_wait_duration_ms",
        "temper_blob_transport_requests_total",
        "temperpaw.fs",
        "fs.operation",
        "Prepared Context Content Files Loaded",
    ] {
        assert!(
            dashboard.contains(required),
            "dashboard must expose TemperFS/blob/document coverage `{required}`"
        );
    }

    for required in [
        "[Temper] Blob Transport Wait Spike",
        "[TemperPaw] TemperFS Metadata Operation Errors",
        "[Temper] Session Memory Externalization Spike",
        "observability_event:temperpaw.fs",
        "fs.outcome:error",
        "temper_blob_transport_wait_duration_ms",
        "temper_session_large_content_externalized_total",
    ] {
        assert!(
            monitors.contains(required),
            "monitors must cover TemperFS/blob symptom `{required}`"
        );
    }

    for required in [
        "fields_json",
        "Parse WASM structured log fields",
        "blob.operation",
        "workspace_id",
    ] {
        assert!(
            pipeline.contains(required),
            "log pipeline must parse blob/document structured fields `{required}`"
        );
    }

    for required in [
        "workspace_id",
        "file_id",
        "content_hash",
        "stream_id",
        "content_type",
        "blob.operation",
        "blob.backend",
        "blob.cache_hit",
        "blob.status_code",
        "blob.size_bytes",
        "fs.operation",
        "fs.path",
        "fs.outcome",
        "fs.backend",
    ] {
        assert!(
            paths.contains(required),
            "Datadog facets must make TemperFS/blob field `{required}` searchable"
        );
    }

    for required in [
        "host_log_structured",
        "temperpaw.blob",
        "\"workspace_id\"",
        "\"file_id\"",
        "\"content_hash\"",
        "\"stream_id\"",
        "\"content_type\"",
        "\"blob\"",
        "\"operation\"",
        "\"cache_hit\"",
        "\"status_code\"",
    ] {
        assert!(
            blob_adapter.contains(required),
            "blob_adapter must emit structured observability field `{required}`"
        );
    }

    for required in [
        "log_structured",
        "temperpaw.fs",
        "\"workspace_id\"",
        "\"fs\"",
        "\"operation\"",
        "\"outcome\"",
        "\"backend\"",
        "\"path\"",
    ] {
        assert!(
            workspace_fs.contains(required),
            "workspace_fs must emit structured observability field `{required}`"
        );
    }

    for required in [
        "TemperFS Blob & Document Services",
        "@workspace_id:<workspace id>",
        "@file_id:<file id>",
        "@fs.operation:create_file",
        "@content_hash:<sha256>",
        "temper_blob_transport_wait_duration_ms",
        "temper_blob_local_fast_path_requests_total",
    ] {
        assert!(
            guide.contains(required),
            "observability guide must teach TemperFS/blob diagnostic path `{required}`"
        );
    }
}

#[test]
fn datadog_covers_modal_bridge_and_sandbox_operations() {
    let dashboard = dashboard_text();
    let monitors = monitor_search_text();
    let guide = load_text("docs/temperpaw-datadog-observability-guide.md");
    let sre_manual = load_text("os-apps/paw-agent/agents/sre/AGENT.md");
    let paw_skill = load_text("os-apps/paw-agent/agents/paw/skills/temperpaw-agent/SKILL.md");
    let sandbox_helper = load_text("os-apps/paw-agent/wasm/wasm-helpers/src/sandbox.rs");
    let modal_bridge = load_text("os-apps/paw-agent/modal-bridge/modal_bridge.py");
    let facets = load_json("dd-pipelines/facets.json");
    let paths: BTreeSet<&str> = facets["facets"]
        .as_array()
        .expect("facets must be an array")
        .iter()
        .filter_map(|facet| facet["path"].as_str())
        .collect();

    for required in [
        "Sandbox & Modal Bridge",
        "temper_wasm_host_http_duration_ms",
        "temper_wasm_host_http_requests_total",
        "call_kind:text",
        "@observability_event:temperpaw.sandbox",
        "sandbox_provider",
        "modal_bridge_url",
    ] {
        assert!(
            dashboard.contains(required),
            "dashboard must expose Modal/sandbox bridge coverage `{required}`"
        );
    }

    for required in [
        "[TemperPaw] Sandbox Host HTTP Error Spike",
        "temper_wasm_host_http_requests_total",
        "call_kind:text",
        "status_code_class:5xx",
    ] {
        assert!(
            monitors.contains(required),
            "monitors must cover Modal/sandbox bridge symptom `{required}`"
        );
    }

    for required in [
        "sandbox_provider",
        "sandbox_id",
        "sandbox.operation",
        "sandbox.backend",
        "sandbox.exit_code",
        "sandbox.status_code",
        "sandbox.workdir",
        "modal_bridge.operation",
        "modal_bridge.endpoint",
        "modal_bridge.duration_ms",
    ] {
        assert!(
            paths.contains(required),
            "Datadog facets must make sandbox field `{required}` searchable"
        );
    }

    for required in [
        "log_structured",
        "temperpaw.sandbox",
        "\"sandbox_provider\"",
        "\"sandbox_id\"",
        "\"sandbox\"",
        "\"operation\"",
        "\"backend\"",
        "\"exit_code\"",
        "\"status_code\"",
        "\"workdir\"",
    ] {
        assert!(
            sandbox_helper.contains(required),
            "sandbox helper must emit structured observability field `{required}`"
        );
    }

    for required in [
        "_log_bridge_event",
        "temperpaw.sandbox",
        "\"sandbox_provider\"",
        "\"sandbox_id\"",
        "\"modal_bridge\"",
        "\"duration_ms\"",
        "\"endpoint\"",
        "\"status_code\"",
    ] {
        assert!(
            modal_bridge.contains(required),
            "Modal bridge must emit structured observability field `{required}`"
        );
    }

    let combined_guidance = format!("{guide}\n{sre_manual}\n{paw_skill}");
    for required in [
        "Sandbox & Modal Bridge",
        "@sandbox_provider:modal",
        "@sandbox_id:<sandbox id>",
        "@sandbox.operation:bash",
        "temper_wasm_host_http_duration_ms",
        "modal_bridge_url",
    ] {
        assert!(
            combined_guidance.contains(required),
            "human/agent guidance must teach sandbox bridge diagnostic path `{required}`"
        );
    }
}

#[test]
fn datadog_covers_channel_transport_observability() {
    let dashboard = dashboard_text();
    let monitors = monitor_search_text();
    let guide = load_text("docs/temperpaw-datadog-observability-guide.md");
    let slack_transport = load_text("crates/paw-transport/src/slack/transport.rs");
    let slack_socket = load_text("crates/paw-transport/src/slack/socket.rs");
    let discord_transport = load_text("crates/paw-transport/src/discord/transport.rs");
    let facets = load_json("dd-pipelines/facets.json");
    let paths: BTreeSet<&str> = facets["facets"]
        .as_array()
        .expect("facets must be an array")
        .iter()
        .filter_map(|facet| facet["path"].as_str())
        .collect();

    for required in [
        "Channel Transports",
        "temperpaw.transport",
        "transport.operation",
        "@transport.name:slack",
        "@transport.name:discord",
    ] {
        assert!(
            dashboard.contains(required),
            "dashboard must expose channel transport coverage `{required}`"
        );
    }

    for required in [
        "[TemperPaw] Channel Transport Dispatch Failures",
        "observability_event:temperpaw.transport",
        "transport.operation:receive_message",
        "transport.outcome:error",
    ] {
        assert!(
            monitors.contains(required),
            "monitors must cover channel transport symptom `{required}`"
        );
    }

    for required in [
        "transport.name",
        "transport.operation",
        "transport.outcome",
        "transport.channel_id",
        "transport.message_id",
        "transport.command",
        "transport.webhook_port",
        "slack.envelope_id",
        "slack.envelope_type",
    ] {
        assert!(
            paths.contains(required),
            "Datadog facets must make channel transport field `{required}` searchable"
        );
    }

    for required in [
        "observability_event = \"temperpaw.transport\"",
        "transport.name = \"slack\"",
        "transport.operation",
        "transport.outcome",
        "transport.channel_id",
        "transport.message_id",
        "message.length",
    ] {
        assert!(
            slack_transport.contains(required),
            "Slack transport must emit structured tracing field `{required}`"
        );
    }

    for required in [
        "observability_event = \"temperpaw.transport\"",
        "transport.name = \"slack\"",
        "transport.operation = \"socket_mode\"",
        "slack.envelope_id",
        "slack.envelope_type",
    ] {
        assert!(
            slack_socket.contains(required),
            "Slack Socket Mode must emit structured tracing field `{required}`"
        );
    }

    for required in [
        "tracing::info_span!",
        "discord.receive",
        "discord.channel_id",
        "discord.message_id",
    ] {
        assert!(
            discord_transport.contains(required),
            "Discord transport must retain structured tracing field `{required}`"
        );
    }

    for required in [
        "Channel Transports",
        "@observability_event:temperpaw.transport",
        "@transport.name:slack",
        "@transport.operation:receive_message",
        "@transport.outcome:error",
    ] {
        assert!(
            guide.contains(required),
            "observability guide must teach channel transport diagnostic path `{required}`"
        );
    }
}

#[test]
fn datadog_covers_governance_approval_observability() {
    let dashboard = dashboard_text();
    let monitors = monitor_search_text();
    let guide = load_text("docs/temperpaw-datadog-observability-guide.md");
    let request_approval = load_text("os-apps/paw-agent/wasm/request_approval/src/lib.rs");
    let facets = load_json("dd-pipelines/facets.json");
    let paths: BTreeSet<&str> = facets["facets"]
        .as_array()
        .expect("facets must be an array")
        .iter()
        .filter_map(|facet| facet["path"].as_str())
        .collect();

    for required in [
        "Governance Approvals",
        "temperpaw.approval",
        "approval.operation",
        "@decision_id:*",
    ] {
        assert!(
            dashboard.contains(required),
            "dashboard must expose approval coverage `{required}`"
        );
    }

    for required in [
        "[TemperPaw] Approval Notification Failures",
        "observability_event:temperpaw.approval",
        "approval.outcome:error",
        "approval.operation:notify_human",
    ] {
        assert!(
            monitors.contains(required),
            "monitors must cover approval symptom `{required}`"
        );
    }

    for required in [
        "decision_id",
        "approval.operation",
        "approval.outcome",
        "approval.delivery",
        "approval.reason",
        "approval.action",
        "approval.http_status",
    ] {
        assert!(
            paths.contains(required),
            "Datadog facets must make approval field `{required}` searchable"
        );
    }

    for required in [
        "log_structured",
        "temperpaw.approval",
        "\"decision_id\"",
        "\"approval\"",
        "\"operation\"",
        "\"outcome\"",
        "\"delivery\"",
        "\"action\"",
        "\"http_status\"",
    ] {
        assert!(
            request_approval.contains(required),
            "request_approval must emit structured observability field `{required}`"
        );
    }

    for required in [
        "Governance Approvals",
        "@observability_event:temperpaw.approval",
        "@decision_id:<decision id>",
        "@approval.operation:notify_human",
        "@approval.outcome:error",
    ] {
        assert!(
            guide.contains(required),
            "observability guide must teach approval diagnostic path `{required}`"
        );
    }
}

#[test]
fn datadog_covers_webhook_trigger_observability() {
    let dashboard = dashboard_text();
    let monitors = monitor_search_text();
    let guide = load_text("docs/temperpaw-datadog-observability-guide.md");
    let trigger = load_text("crates/paw-transport/src/webhook/trigger.rs");
    let facets = load_json("dd-pipelines/facets.json");
    let paths: BTreeSet<&str> = facets["facets"]
        .as_array()
        .expect("facets must be an array")
        .iter()
        .filter_map(|facet| facet["path"].as_str())
        .collect();

    for required in [
        "Webhook Triggers",
        "temperpaw.webhook",
        "webhook.route_key",
        "@webhook.outcome:error",
    ] {
        assert!(
            dashboard.contains(required),
            "dashboard must expose webhook trigger coverage `{required}`"
        );
    }

    for required in [
        "[TemperPaw] Webhook Receive Errors",
        "observability_event:temperpaw.webhook",
        "webhook.outcome:error",
    ] {
        assert!(
            monitors.contains(required),
            "monitors must cover webhook trigger symptom `{required}`"
        );
    }

    for required in [
        "webhook.route_key",
        "webhook.event_id",
        "webhook.operation",
        "webhook.outcome",
        "webhook.status",
        "webhook.payload_bytes",
    ] {
        assert!(
            paths.contains(required),
            "Datadog facets must make webhook field `{required}` searchable"
        );
    }

    for required in [
        "observability_event = \"temperpaw.webhook\"",
        "webhook.route_key",
        "webhook.event_id",
        "webhook.operation",
        "webhook.outcome",
        "webhook.status",
        "webhook.payload_bytes",
    ] {
        assert!(
            trigger.contains(required),
            "webhook trigger must emit structured tracing field `{required}`"
        );
    }

    for required in [
        "Webhook Triggers",
        "@observability_event:temperpaw.webhook",
        "@webhook.route_key:<route key>",
        "@webhook.outcome:error",
    ] {
        assert!(
            guide.contains(required),
            "observability guide must teach webhook diagnostic path `{required}`"
        );
    }
}

#[test]
fn otel_collector_routes_llmobs_without_dropping_apm_coverage() {
    let collector =
        std::fs::read_to_string(repo_root().join("scripts/otel-collector-datadog.yaml"))
            .expect("collector config should be readable");

    for required in [
        "traces/llmobs",
        "traces/apm",
        "dd-otlp-source: llmobs",
        "transform/dbm",
        "set(attributes[\"span.type\"], \"sql\") where attributes[\"db.system\"] != nil and attributes[\"span.type\"] == nil",
        "attributes[\"gen_ai.operation.name\"] == nil and attributes[\"gen_ai.system\"] == nil",
        "attributes[\"gen_ai.operation.name\"] != nil or attributes[\"gen_ai.system\"] != nil",
        "exporters: [clickhouse, otlphttp/llmobs]",
        "exporters: [clickhouse, datadog]",
    ] {
        assert!(
            collector.contains(required),
            "collector must preserve LLMObs/APM routing clause `{required}`"
        );
    }
}

#[test]
fn railway_otel_collectors_preserve_llmobs_routing() {
    let railway_collector =
        std::fs::read_to_string(repo_root().join("scripts/otel-collector-railway.yaml"))
            .expect("railway collector config should be readable");
    let deploy_source =
        std::fs::read_to_string(repo_root().join("crates/temperpaw-cli/src/deploy.rs"))
            .expect("deploy source should be readable");

    for (name, source) in [
        ("scripts/otel-collector-railway.yaml", railway_collector),
        ("crates/temperpaw-cli/src/deploy.rs", deploy_source),
    ] {
        for required in [
            "traces/llmobs",
            "traces/apm",
            "otlphttp/llmobs",
            "dd-otlp-source: llmobs",
            "service.namespace",
            "team",
            "transform/dbm",
            "set(attributes[\"span.type\"], \"sql\") where attributes[\"db.system\"] != nil and attributes[\"span.type\"] == nil",
            "attributes[\"gen_ai.operation.name\"] == nil and attributes[\"gen_ai.system\"] == nil",
            "attributes[\"gen_ai.operation.name\"] != nil or attributes[\"gen_ai.system\"] != nil",
        ] {
            assert!(
                source.contains(required),
                "{name} must preserve LLMObs/APM routing clause `{required}`"
            );
        }
    }
}

#[test]
fn railway_otel_collector_has_deployable_checked_in_source() {
    let collector_dir = repo_root().join("deploy/otel-collector");
    let dockerfile = std::fs::read_to_string(collector_dir.join("Dockerfile"))
        .expect("checked-in OTEL collector Dockerfile should be readable");
    let entrypoint = std::fs::read_to_string(collector_dir.join("entrypoint.sh"))
        .expect("checked-in OTEL collector entrypoint should be readable");
    let railway = std::fs::read_to_string(collector_dir.join("railway.toml"))
        .expect("checked-in OTEL collector railway.toml should be readable");
    let datadog = std::fs::read_to_string(collector_dir.join("otel-datadog.yaml"))
        .expect("checked-in OTEL collector Datadog config should be readable");
    let debug = std::fs::read_to_string(collector_dir.join("otel-debug.yaml"))
        .expect("checked-in OTEL collector debug config should be readable");

    for required in [
        "FROM otel/opentelemetry-collector-contrib:latest AS collector",
        "FROM alpine:",
        "COPY --from=collector /otelcol-contrib /otelcol-contrib",
        "COPY otel-datadog.yaml /etc/otelcol-contrib/otel-datadog.yaml",
        "COPY --chmod=755 entrypoint.sh /entrypoint.sh",
        "ENTRYPOINT [\"/entrypoint.sh\"]",
    ] {
        assert!(
            dockerfile.contains(required),
            "checked-in collector Dockerfile must include `{required}`"
        );
    }
    assert!(
        !dockerfile.contains("USER 0"),
        "checked-in collector Dockerfile must not rely on numeric USER 0"
    );

    for required in [
        "DD_API_KEY detected - exporting to Datadog",
        "--feature-gates=datadog.EnableOperationAndResourceNameV2",
        "--config /etc/otelcol-contrib/otel-datadog.yaml",
    ] {
        assert!(
            entrypoint.contains(required),
            "checked-in collector entrypoint must include `{required}`"
        );
    }

    assert!(
        railway.contains("dockerfilePath = \"Dockerfile\""),
        "checked-in collector Railway manifest must deploy the collector Dockerfile"
    );

    for required in [
        "value: temperpaw",
        "action: upsert",
        "traces/llmobs",
        "traces/apm",
        "transform/dbm",
        "dd-otlp-source: llmobs",
    ] {
        assert!(
            datadog.contains(required),
            "checked-in collector Datadog config must include `{required}`"
        );
    }

    assert!(
        debug.contains("exporters:") && debug.contains("debug:"),
        "checked-in collector debug config must preserve no-key debug mode"
    );
}

#[test]
fn datadog_pipeline_deploy_reconciles_legacy_log_metrics() {
    let script = std::fs::read_to_string(repo_root().join("scripts/deploy_pipelines.py"))
        .expect("deploy_pipelines.py should be readable");

    for required in [
        "--reconcile",
        "LEGACY_LOG_METRIC_PREFIXES",
        "openpaw.",
        "logs/config/metrics/{metric_id}",
        "requests.delete",
    ] {
        assert!(
            script.contains(required),
            "Datadog pipeline deploy must include legacy log-metric reconciliation clause `{required}`"
        );
    }
}

#[test]
fn sensitive_data_scanner_covers_observability_and_agent_secret_shapes() {
    let scanner = load_json("dd-pipelines/sensitive-data-scanner.json").to_string();
    let deploy_script = std::fs::read_to_string(repo_root().join("scripts/deploy_pipelines.py"))
        .expect("deploy_pipelines.py should be readable");

    for required in [
        "Datadog API/application keys",
        "DD_(?:API|APP|APPLICATION)_KEY",
        "sk-(?:ant-)?",
        "gh[posu]_",
        "xox[pbaors]-",
        "[REDACTED:DATADOG]",
        "[REDACTED:OPENAI]",
        "[REDACTED:GITHUB]",
        "[REDACTED:SLACK]",
    ] {
        assert!(
            scanner.contains(required),
            "Sensitive-data scanner must cover `{required}`"
        );
    }

    assert!(
        deploy_script.contains("deploy_sds"),
        "Datadog pipeline deploy must read the sensitive-data scanner source of truth"
    );
}

#[test]
fn managed_session_events_expose_queryable_bridge_context() {
    let session_event_spec = std::fs::read_to_string(
        repo_root().join("os-apps/paw-managed-agents/specs/session_event.ioa.toml"),
    )
    .expect("session_event spec should be readable");
    let managed_agents_model = std::fs::read_to_string(
        repo_root().join("os-apps/paw-managed-agents/specs/model.csdl.xml"),
    )
    .expect("managed-agents CSDL should be readable");
    let session_orchestrator = std::fs::read_to_string(
        repo_root().join("os-apps/paw-managed-agents/wasm/session_orchestrator/src/lib.rs"),
    )
    .expect("session_orchestrator source should be readable");
    let event_emitter = std::fs::read_to_string(
        repo_root().join("os-apps/paw-managed-agents/wasm/event_emitter/src/lib.rs"),
    )
    .expect("event_emitter source should be readable");
    let session_terminator = std::fs::read_to_string(
        repo_root().join("os-apps/paw-managed-agents/wasm/session_terminator/src/lib.rs"),
    )
    .expect("session_terminator source should be readable");
    let managed_common =
        std::fs::read_to_string(repo_root().join("os-apps/paw-managed-agents/wasm/common.rs"))
            .expect("paw-managed-agents common source should be readable");
    let facets = load_json("dd-pipelines/facets.json");
    let paths: BTreeSet<&str> = facets["facets"]
        .as_array()
        .expect("facets must be an array")
        .iter()
        .filter_map(|facet| facet["path"].as_str())
        .collect();

    for required in [
        "name = \"observability_event\"",
        "name = \"managed_session_id\"",
        "name = \"inner_session_id\"",
        "name = \"inner_agent_id\"",
        "name = \"managed_agent_id\"",
        "name = \"parent_session_id\"",
        "name = \"environment_id\"",
        "name = \"action_name\"",
    ] {
        assert!(
            session_event_spec.contains(required),
            "SessionEvent IOA must expose queryable bridge field `{required}`"
        );
    }

    for required in [
        "<Property Name=\"ObservabilityEvent\" Type=\"Edm.String\"/>",
        "<Property Name=\"ManagedSessionId\" Type=\"Edm.String\"/>",
        "<Property Name=\"InnerSessionId\" Type=\"Edm.String\"/>",
        "<Property Name=\"InnerAgentId\" Type=\"Edm.String\"/>",
        "<Property Name=\"ManagedAgentId\" Type=\"Edm.String\"/>",
        "<Property Name=\"ParentSessionId\" Type=\"Edm.String\"/>",
        "<Property Name=\"EnvironmentId\" Type=\"Edm.String\"/>",
        "<Property Name=\"ActionName\" Type=\"Edm.String\"/>",
    ] {
        assert!(
            managed_agents_model.contains(required),
            "ManagedAgents CSDL must expose queryable bridge property `{required}`"
        );
    }

    for required in [
        "session_event.kind",
        "session_event.sequence",
        "session_event.stop_reason",
        "session_event.termination_reason",
    ] {
        assert!(
            paths.contains(required),
            "Datadog facets must make managed SessionEvent field `{required}` searchable"
        );
    }

    for required in [
        "log_managed_session_event",
        "managed_session_observability_log_fields",
        "temperpaw.agent.session event",
        "\"session_event\"",
        "\"kind\"",
        "\"sequence\"",
    ] {
        assert!(
            managed_common.contains(required),
            "managed agent common helpers must emit structured Datadog log field `{required}`"
        );
    }

    for required in [
        "\"ObservabilityEvent\"",
        "\"ManagedSessionId\"",
        "\"InnerSessionId\"",
        "\"InnerAgentId\"",
        "\"ManagedAgentId\"",
        "\"ParentSessionId\"",
        "\"EnvironmentId\"",
        "\"ActionName\"",
    ] {
        assert!(
            session_orchestrator.contains(required),
            "session_orchestrator must write queryable bridge field `{required}`"
        );
    }

    for required in [
        "managed_session_event_context",
        "with_session_event_context",
        "log_managed_session_event",
        "\"session.status_idle\"",
        "\"agent.message\"",
        "\"agent.thinking\"",
        "\"agent.tool_use\"",
        "\"agent.tool_result\"",
    ] {
        assert!(
            event_emitter.contains(required),
            "event_emitter must write bridge context on chronological event `{required}`"
        );
    }

    for required in [
        "managed_session_event_context",
        "with_session_event_context",
        "log_managed_session_event",
        "\"session.status_terminated\"",
        "\"TerminationReason\"",
    ] {
        assert!(
            session_terminator.contains(required),
            "session_terminator must write bridge context on terminal event `{required}`"
        );
    }
}

#[test]
fn datadog_monitor_deploy_reconciles_untagged_legacy_monitors() {
    let script = std::fs::read_to_string(repo_root().join("scripts/deploy_monitors.py"))
        .expect("deploy_monitors.py should be readable");

    for required in [
        "legacy_openpaw_monitor",
        "team:temperpaw",
        "slack-openpaw-alerts",
        "service:openpaw",
        "is_temperpaw_owned_monitor",
        "requests.delete",
    ] {
        assert!(
            script.contains(required),
            "Datadog monitor deploy must include untagged legacy reconciliation clause `{required}`"
        );
    }
}

#[test]
fn datadog_dashboard_deploy_reconciles_legacy_dashboards() {
    let script = std::fs::read_to_string(repo_root().join("scripts/deploy_dashboard.py"))
        .expect("deploy_dashboard.py should be readable");

    for required in [
        "--reconcile",
        "LEGACY_DASHBOARD_TERMS",
        "legacy_openpaw_dashboard",
        "is_temperpaw_owned_dashboard",
        "requests.delete",
    ] {
        assert!(
            script.contains(required),
            "Datadog dashboard deploy must include legacy dashboard reconciliation clause `{required}`"
        );
    }
}

#[test]
fn deploy_configures_postgres_dbm_agent_when_datadog_is_enabled() {
    let deploy_source =
        std::fs::read_to_string(repo_root().join("crates/temperpaw-cli/src/deploy.rs"))
            .expect("deploy source should be readable");
    let entrypoint = std::fs::read_to_string(repo_root().join("scripts/temperpaw-entrypoint.sh"))
        .expect("entrypoint should be readable");

    for required in [
        "datadog-postgres-agent",
        "datadog/agent:7",
        "dbm: true",
        "PGHOST=${{Postgres.PGHOST}}",
        "PGPASSWORD=${{Postgres.PGPASSWORD}}",
        "DD_APM_ENABLED=true",
        "DD_APM_NON_LOCAL_TRAFFIC=true",
        "DD_APM_FEATURES=enable_operation_and_resource_name_logic_v2",
        "TEMPER_PROFILING_ENABLED=true",
        "TEMPER_PROFILING_AUTO_UPLOAD=true",
        "datadog-runtime-agent",
        "DD_AGENT_HOST=datadog-runtime-agent.railway.internal",
        "DD_TRACE_AGENT_URL=http://datadog-runtime-agent.railway.internal:8126",
        "service:temperpaw",
        "team:temperpaw",
    ] {
        assert!(
            deploy_source.contains(required),
            "TemperPaw deploy must configure Postgres DBM agent clause `{required}`"
        );
    }

    assert!(
        entrypoint.contains("TEMPER_DDPROF_ENABLED"),
        "ddprof must be explicitly opt-in because Railway denies perf_event_open"
    );
    assert!(
        !entrypoint.contains("[ \"${DD_PROFILING_ENABLED:-false}\" = \"true\" ]"),
        "DD_PROFILING_ENABLED must not implicitly start ddprof on Railway"
    );
}

#[test]
fn railway_datadog_runtime_agent_product_coverage_is_documented_and_deployable() {
    let deploy_source =
        std::fs::read_to_string(repo_root().join("crates/temperpaw-cli/src/deploy.rs"))
            .expect("deploy source should be readable");
    let adr = load_text("docs/adrs/0049-railway-datadog-product-coverage.md");
    let guide = load_text("docs/temperpaw-datadog-observability-guide.md");

    for required in [
        "datadog-runtime-agent",
        "DD_OTLP_CONFIG_RECEIVER_PROTOCOLS_HTTP_ENDPOINT=0.0.0.0:4318",
        "DD_OTLP_CONFIG_RECEIVER_PROTOCOLS_GRPC_ENDPOINT=0.0.0.0:4317",
        "DD_LOGS_ENABLED=true",
        "DD_PROCESS_AGENT_ENABLED=true",
        "DD_AGENT_HOST=datadog-runtime-agent.railway.internal",
        "DD_TRACE_AGENT_URL=http://datadog-runtime-agent.railway.internal:8126",
        "DD_LLMOBS_API_ENABLED=true",
        "TEMPER_DATADOG_RAILWAY_PROFILE=datadog-enhanced-railway",
        "OTEL_EXPORTER_OTLP_ENDPOINT=http://datadog-runtime-agent.railway.internal:4318",
    ] {
        assert!(
            deploy_source.contains(required),
            "Railway Datadog runtime-agent deploy must include `{required}`"
        );
    }

    for required in [
        "APM | supported",
        "Logs correlation | supported",
        "Error Tracking | supported",
        "LLM Observability | supported",
        "On-demand Profiling | supported",
        "Continuous Profiling | best-effort",
        "USM | blocked-on-Railway-capability",
        "No Linux Compose",
        "No Kubernetes",
        "No database migration",
    ] {
        assert!(
            adr.contains(required),
            "Railway Datadog coverage ADR must classify `{required}`"
        );
    }

    for required in [
        "datadog-enhanced-railway",
        "datadog-runtime-agent",
        "blocked-on-Railway-system-probe",
        "blocked-on-Railway-perf-permissions",
        "on-demand profiling remains supported",
    ] {
        assert!(
            guide.contains(required),
            "observability guide must explain Railway Datadog product boundary `{required}`"
        );
    }
}

#[test]
fn railway_datadog_capability_check_reports_usm_and_continuous_profiler_boundaries() {
    let script = load_text("scripts/datadog_railway_capability_check.sh");
    let dockerfile = load_text("Dockerfile");
    let guide = load_text("docs/temperpaw-datadog-observability-guide.md");
    let setup_api = load_text("crates/temperpaw/src/setup_api.rs");

    for required in [
        "blocked-on-Railway-system-probe",
        "blocked-on-Railway-perf-permissions",
        "DD_SYSTEM_PROBE_SERVICE_MONITORING_ENABLED",
        "CAP_SYS_ADMIN",
        "CAP_SYS_PTRACE",
        "/sys/kernel/debug",
        "/host/proc",
        "TEMPER_DDPROF_ENABLED",
        "perf_event_paranoid",
    ] {
        assert!(
            script.contains(required),
            "Railway Datadog capability check must inspect/report `{required}`"
        );
    }

    assert!(
        dockerfile.contains("datadog_railway_capability_check.sh"),
        "production image must include the Railway Datadog capability check"
    );
    assert!(
        guide.contains("datadog_railway_capability_check.sh"),
        "operator guide must document how to run the Railway capability check"
    );

    for required in [
        "/paw/infra/railway/datadog-capability-check",
        "DatadogRailwayCapabilityReport",
        "best-effort-system-probe-not-enabled",
        "blocked-on-Railway-system-probe",
        "blocked-on-Railway-perf-permissions",
        "CAP_PERFMON",
        "perf_event_paranoid",
        "ddprof_present",
    ] {
        assert!(
            setup_api.contains(required),
            "setup API must expose a live Railway Datadog capability proof field `{required}`"
        );
    }

    assert!(
        guide.contains("/paw/infra/railway/datadog-capability-check"),
        "operator guide must document the authenticated capability endpoint for live Railway proof"
    );
}

#[test]
fn railway_runtime_agent_service_id_is_persisted_for_dashboard_status() {
    let config = load_text("crates/temperpaw/src/config.rs");
    let startup = load_text("crates/temperpaw/src/startup.rs");
    let setup_api = load_text("crates/temperpaw/src/setup_api.rs");
    let dashboard_api = load_text("dashboard/src/lib/api.ts");

    assert!(
        config.contains("RAILWAY_DATADOG_RUNTIME_AGENT_SERVICE_ID"),
        "Config must load the Runtime Agent service id env var"
    );

    for (source_name, source) in [
        ("Config", config.as_str()),
        ("startup", startup.as_str()),
        ("setup API", setup_api.as_str()),
    ] {
        assert!(
            source.contains("railway_datadog_runtime_agent_service_id"),
            "{source_name} must carry the normalized Runtime Agent service id field"
        );
    }
    assert!(
        dashboard_api.contains("datadog_runtime_agent_service_id"),
        "dashboard API type must expose the Runtime Agent service id response field"
    );
}

#[test]
fn setup_api_can_ensure_railway_datadog_runtime_agent_without_exposing_tokens() {
    let setup_api = load_text("crates/temperpaw/src/setup_api.rs");

    for required in [
        "/paw/infra/railway/datadog-runtime-agent/ensure",
        "ensure_datadog_runtime_agent",
        "serviceCreate",
        "ServiceSourceInput",
        "datadog/agent:7",
        "datadog-runtime-agent",
        "railway_datadog_runtime_agent_service_id",
        "DD_OTLP_CONFIG_RECEIVER_PROTOCOLS_HTTP_ENDPOINT",
        "DD_APM_NON_LOCAL_TRAFFIC",
        "DD_LOGS_ENABLED",
        "DD_PROCESS_AGENT_ENABLED",
        "TEMPER_DATADOG_RAILWAY_PROFILE",
        "datadog-enhanced-railway",
        "OTEL_EXPORTER_OTLP_ENDPOINT",
        "DD_TRACE_AGENT_URL",
        "DD_LLMOBS_API_ENABLED",
    ] {
        assert!(
            setup_api.contains(required),
            "Railway setup API must be able to ensure Runtime Agent contract `{required}`"
        );
    }

    assert!(
        setup_api.contains("\"DD_API_KEY\"") && setup_api.contains("\"dd_api_key\""),
        "ensure endpoint must read Datadog credentials from the vault and set Agent vars internally"
    );
    assert!(
        !setup_api.contains("railway_token\" })"),
        "ensure endpoint responses must not serialize the Railway token"
    );
}

#[test]
fn setup_api_can_run_a_temporary_continuous_profiler_canary() {
    let setup_api = load_text("crates/temperpaw/src/setup_api.rs");
    let guide = load_text("docs/temperpaw-datadog-observability-guide.md");

    for required in [
        "/paw/infra/railway/datadog-continuous-profiler-canary",
        "set_datadog_continuous_profiler_canary",
        "SetDatadogContinuousProfilerCanaryRequest",
        "TEMPER_DDPROF_ENABLED",
        "DD_PROFILING_ENABLED",
        "railway_redeploy_service",
        "railway_service_id",
    ] {
        assert!(
            setup_api.contains(required),
            "setup API must support a narrowly scoped continuous profiler canary contract `{required}`"
        );
    }

    assert!(
        !setup_api.contains("railway_token\" })"),
        "continuous profiler canary endpoint responses must not serialize Railway tokens"
    );
    assert!(
        guide.contains("/paw/infra/railway/datadog-continuous-profiler-canary")
            && guide.contains("TEMPER_DDPROF_ENABLED=true")
            && guide.contains("TEMPER_DDPROF_ENABLED=false"),
        "operator guide must document how to enable and disable the temporary ddprof canary"
    );
}

#[test]
fn temperpaw_span_hints_expose_session_tool_and_llmobs_semconv() {
    let provider_caller = std::fs::read_to_string(
        repo_root().join("os-apps/paw-agent/wasm/provider_caller/src/lib.rs"),
    )
    .expect("provider_caller source should be readable");
    let monty_dispatch = std::fs::read_to_string(
        repo_root().join("os-apps/paw-agent/wasm/monty_repl/src/dispatch.rs"),
    )
    .expect("monty_repl dispatch source should be readable");
    let monty_repl =
        std::fs::read_to_string(repo_root().join("os-apps/paw-agent/wasm/monty_repl/src/lib.rs"))
            .expect("monty_repl source should be readable");
    let managed_common =
        std::fs::read_to_string(repo_root().join("os-apps/paw-managed-agents/wasm/common.rs"))
            .expect("paw-managed-agents common source should be readable");
    let session_orchestrator = std::fs::read_to_string(
        repo_root().join("os-apps/paw-managed-agents/wasm/session_orchestrator/src/lib.rs"),
    )
    .expect("session_orchestrator source should be readable");

    for required in [
        "agent_session_span_hint_headers",
        "temperpaw.agent.session",
        "X-Temper-Span-Attr-gen_ai.operation.name",
        "\"invoke_agent\"",
        "X-Temper-Span-Attr-session_id",
        "X-Temper-Span-Attr-managed_session_id",
        "X-Temper-Span-Attr-inner_session_id",
        "X-Temper-Span-Attr-parent_session_id",
        "X-Temper-Span-Attr-agent_id",
        "X-Temper-Span-Attr-environment_id",
        "X-Temper-Span-Attr-entity_type",
        "X-Temper-Span-Attr-action_name",
    ] {
        assert!(
            managed_common.contains(required),
            "managed agent session span hints must include `{required}`"
        );
    }

    for required in [
        "agent_session_span_hint_headers",
        "TemperPaw.Configure",
        "TemperPaw.Steer",
    ] {
        assert!(
            session_orchestrator.contains(required),
            "session_orchestrator must apply managed session span hints around `{required}`"
        );
    }

    for required in [
        "X-Temper-Span-Attr-gen_ai.operation.name",
        "\"chat\"",
        "X-Temper-Span-Attr-gen_ai.provider.name",
        "X-Temper-Span-Attr-gen_ai.conversation.id",
        "X-Temper-Span-Attr-session_id",
        "X-Temper-Span-Attr-tool.name",
        "X-Temper-Span-Capture-Response-gen_ai.completion",
    ] {
        assert!(
            provider_caller.contains(required),
            "provider LLM span hints must include `{required}`"
        );
    }

    for required in [
        "X-Temper-Span-Attr-gen_ai.operation.name",
        "\"execute_tool\"",
        "X-Temper-Span-Attr-tool.name",
        "X-Temper-Span-Attr-tool.call_id",
    ] {
        assert!(
            monty_dispatch.contains(required),
            "tool-call span hints must include `{required}`"
        );
    }

    for required in [
        "attach_llmobs_tool_spans",
        "_dd_llmobs_tool_spans",
        "tool_span_events",
    ] {
        assert!(
            monty_repl.contains(required),
            "monty_repl must forward tool events to Temper's LLMObs tool-span ingestion path via `{required}`"
        );
    }
}
