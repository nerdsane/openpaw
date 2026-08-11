//! Contract tests for OTS trajectory emission (ARN-109).
//!
//! These assert the wiring that decides whether a stored trajectory is usable
//! training data: that tool spans are actually persisted, that the emitter is
//! told which spec governed the run, and that the emitted JSON uses the exact
//! field names the kernel's `temper-ots` structs deserialize.

use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn session_spec() -> String {
    fs::read_to_string(repo_root().join("os-apps/paw-agent/specs/session.ioa.toml"))
        .expect("session.ioa.toml should exist")
}

fn emitter_source() -> String {
    fs::read_to_string(
        repo_root().join("os-apps/paw-agent/wasm/emit_ots_trajectory/src/ots_build.rs"),
    )
    .expect("ots_build.rs should exist")
}

/// The production defect this track fixes: with span persistence off, every
/// stored trajectory carried an empty `decisions` array.
#[test]
fn run_tools_persists_tool_spans() {
    let spec = session_spec();
    assert!(
        spec.contains("persist_tool_spans_file = \"true\""),
        "run_tools must persist tool spans; without them OTS decisions lose \
         tool wall-clock time and externalized turns lose their only evidence"
    );
    assert!(
        !spec.contains("persist_tool_spans_file = \"false\""),
        "no trigger may disable tool-span persistence"
    );
}

/// `metadata.spec_version` has to name the spec that actually ran, and the WASM
/// guest context exposes no spec hash — so the literal in the spec is the
/// identity, and it must track the app manifest.
#[test]
fn emitter_spec_version_matches_the_app_manifest() {
    let manifest = fs::read_to_string(repo_root().join("os-apps/paw-agent/app.toml"))
        .expect("paw-agent app.toml should exist");
    let version = manifest
        .lines()
        .find_map(|line| line.trim().strip_prefix("version = "))
        .map(|value| value.trim().trim_matches('"').to_string())
        .expect("app.toml should declare a version");
    let expected = format!("spec_version = \"paw-agent@{version}\"");

    assert!(
        session_spec().contains(&expected),
        "emit_ots_trajectory config must declare {expected}; the emitter reports \
         it as OTSMetadata.spec_version and it may not drift from app.toml"
    );
}

/// Every terminal path that finishes a session has to emit a trajectory —
/// otherwise the training set silently loses whole classes of run.
#[test]
fn every_terminal_action_emits_a_trajectory() {
    let spec = session_spec();
    let emit = "{ type = \"trigger\", name = \"emit_ots_trajectory\" }";
    for action in [
        "FinalizeResult",
        "FinalizeResultNoReply",
        "RecordResult",
        "RecordResultNoReply",
        "RecordResultInlineReply",
        "Fail",
        "Cancel",
        "TimeoutFail",
    ] {
        let marker = format!("name = \"{action}\"\n");
        let start = spec
            .find(&marker)
            .unwrap_or_else(|| panic!("{action} must exist in session.ioa.toml"));
        let block = &spec[start..];
        let block = &block[..block.find("\n[[action]]").unwrap_or(block.len())];
        assert!(
            block.contains(emit),
            "{action} must trigger emit_ots_trajectory"
        );
    }
}

/// Field names are the wire contract with the kernel's `temper-ots` structs.
/// A rename on either side silently drops the data at deserialization.
#[test]
fn emitter_uses_the_kernel_ots_field_names() {
    let source = emitter_source();
    for field in [
        // OTSTrajectory / OTSMetadata
        "\"trajectory_id\"",
        "\"version\"",
        "\"metadata\"",
        "\"turns\"",
        "\"task_description\"",
        "\"timestamp_start\"",
        "\"timestamp_end\"",
        "\"agent_id\"",
        "\"outcome\"",
        // ARN-109 additive metadata
        "\"harness\"",
        "\"spec_version\"",
        // OTSTurn
        "\"turn_id\"",
        "\"span_id\"",
        "\"timestamp\"",
        "\"messages\"",
        "\"decisions\"",
        // ARN-109 additive turn fields
        "\"prompt_token_ids\"",
        "\"completion_token_ids\"",
        "\"response_mask\"",
        "\"logprobs\"",
        // OTSMessage
        "\"message_id\"",
        "\"role\"",
        "\"content\"",
        "\"reasoning\"",
        // OTSDecision
        "\"decision_id\"",
        "\"decision_type\"",
        "\"cause_id\"",
        "\"choice\"",
        "\"consequence\"",
        "\"result_summary\"",
        "\"error_type\"",
    ] {
        assert!(
            source.contains(field),
            "emitter must produce the OTS field {field}"
        );
    }

    assert!(
        source.contains("\"tool_selection\""),
        "decision_type serializes snake_case per temper-ots enums"
    );
    assert!(
        source.contains("pub const HARNESS: &str = \"temperpaw\""),
        "metadata.harness identifies the runtime that produced the run"
    );
}

/// Inlining message bodies once cost ~300MB of a 491MB database (.proofs/061).
/// The budget constants are the guard, and the file-reference path is what
/// keeps large bodies out of the document entirely.
#[test]
fn emitter_bounds_inline_payloads() {
    let source = emitter_source();
    assert!(
        source.contains("MAX_MESSAGE_INLINE_CHARS"),
        "per-message inline ceiling must exist"
    );
    assert!(
        source.contains("MAX_TRAJECTORY_INLINE_CHARS"),
        "whole-document inline ceiling must exist"
    );
    assert!(
        source.contains("content_file_id"),
        "externalized bodies must be referenced by file id, never inlined"
    );
}

/// Retry idempotency: the Turso row is keyed on trajectory_id, so the id has to
/// be derived from the session and repeated inside metadata for the POST handler.
#[test]
fn emitter_keeps_trajectory_id_idempotency() {
    let lib = fs::read_to_string(
        repo_root().join("os-apps/paw-agent/wasm/emit_ots_trajectory/src/lib.rs"),
    )
    .expect("emit_ots_trajectory lib.rs should exist");
    assert!(
        lib.contains("format!(\"trj-{session_id}\")"),
        "trajectory_id must stay derived from the session id"
    );
    assert!(
        lib.contains("MarkTrajectoryEmitted") && lib.contains("TrajectoryEmissionFailed"),
        "emission status actions must stay wired"
    );
    assert!(
        emitter_source().contains("\"trajectory_id\": trajectory_id"),
        "metadata must repeat trajectory_id for the server-side POST handler"
    );

    let spec = session_spec();
    for action in [
        "MarkTrajectoryEmitted",
        "TrajectoryEmissionFailed",
        "RetryTrajectoryEmission",
    ] {
        assert!(
            spec.contains(&format!("name = \"{action}\"")),
            "{action} must stay declared on the Session automaton"
        );
    }
}

/// Per-turn timestamps and token counts come from the entry itself, because the
/// entity event log is a hot tail that drops older events at snapshot boundaries.
#[test]
fn session_entries_carry_their_own_wall_clock() {
    let helpers =
        fs::read_to_string(repo_root().join("os-apps/paw-agent/wasm/wasm-helpers/src/lib.rs"))
            .expect("wasm-helpers lib.rs should exist");
    assert!(
        helpers.contains("fn stamp_recorded_at"),
        "every SessionEntry must be stamped with its own creation time"
    );
    assert!(
        helpers.contains("\"ts_ms\""),
        "the stamp field is ts_ms; the emitter reads it back"
    );

    let applier = fs::read_to_string(
        repo_root().join("os-apps/paw-agent/wasm/provider_response_applier/src/lib.rs"),
    )
    .expect("provider_response_applier lib.rs should exist");
    assert!(
        applier.contains("fn assistant_turn_extra"),
        "assistant entries must record per-turn provider, model and usage facts"
    );
    for field in ["input_tokens", "output_tokens", "stop_reason"] {
        assert!(
            applier.contains(&format!("\"{field}\"")),
            "assistant turn extras must record {field}"
        );
    }
}
